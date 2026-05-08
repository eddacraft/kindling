//! JSON round-trip tests against fixtures shaped like the existing TypeScript
//! types in `packages/kindling-core/src/types/`.
//!
//! Each test:
//!   1. Reads a fixture written in the TS wire shape (camelCase, snake_case
//!      enum values, optional fields absent rather than null).
//!   2. Deserialises it into the Rust type.
//!   3. Re-serialises the Rust value and compares it to the fixture as a
//!      `serde_json::Value` (so cosmetic formatting doesn't break the test).
//!
//! When this fails it almost always means the Rust type drifted from the TS
//! definition — fix the Rust side, not the fixture.

use kindling_types::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn fixture(name: &str) -> Value {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    let raw = fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
}

fn round_trip<T: DeserializeOwned + Serialize>(name: &str) -> T {
    let value = fixture(name);
    let parsed: T =
        serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("deserialize {name}: {e}"));
    let reserialised: Value =
        serde_json::to_value(&parsed).unwrap_or_else(|e| panic!("serialize {name}: {e}"));
    assert_eq!(reserialised, value, "round-trip mismatch for {name}");
    parsed
}

#[test]
fn scope_ids_full() {
    let s: ScopeIds = round_trip("scope_ids_full.json");
    assert_eq!(s.session_id.as_deref(), Some("01J8XS7ABCDEF0123456789ABC"));
    assert_eq!(s.repo_id.as_deref(), Some("/home/dev/kindling"));
    assert_eq!(s.agent_id.as_deref(), Some("claude-code"));
    assert_eq!(s.user_id.as_deref(), Some("josh"));
    assert_eq!(s.task_id.as_deref(), Some("BEADS-42"));
}

#[test]
fn scope_ids_partial_omits_absent_fields() {
    let s: ScopeIds = round_trip("scope_ids_partial.json");
    assert!(s.agent_id.is_none());
    assert!(s.user_id.is_none());
    assert!(s.task_id.is_none());
}

#[test]
fn scope_ids_empty_serialises_as_empty_object() {
    let v = serde_json::to_value(ScopeIds::default()).unwrap();
    assert_eq!(
        v,
        json!({}),
        "absent optional fields must not appear in JSON"
    );
}

#[test]
fn observation_full() {
    let o: Observation = round_trip("observation_full.json");
    assert_eq!(o.kind, ObservationKind::ToolCall);
    assert_eq!(o.content, "rg --files src/");
    assert_eq!(o.ts, 1_746_662_400_000);
    assert!(!o.redacted);
    assert_eq!(
        o.provenance.get("toolName").and_then(Value::as_str),
        Some("ripgrep"),
    );
}

#[test]
fn observation_kind_serialises_as_snake_case_strings() {
    for (kind, expected) in [
        (ObservationKind::ToolCall, "tool_call"),
        (ObservationKind::Command, "command"),
        (ObservationKind::FileDiff, "file_diff"),
        (ObservationKind::Error, "error"),
        (ObservationKind::Message, "message"),
        (ObservationKind::NodeStart, "node_start"),
        (ObservationKind::NodeEnd, "node_end"),
        (ObservationKind::NodeOutput, "node_output"),
        (ObservationKind::NodeError, "node_error"),
    ] {
        let s = serde_json::to_value(kind).unwrap();
        assert_eq!(s, Value::String(expected.into()), "kind={kind:?}");
    }
}

#[test]
fn observation_kind_all_lists_every_variant_in_order() {
    assert_eq!(ObservationKind::ALL.len(), 9);
    assert_eq!(ObservationKind::ALL[0], ObservationKind::ToolCall);
    assert_eq!(ObservationKind::ALL[8], ObservationKind::NodeError);
}

#[test]
fn capsule_open_minimal() {
    let c: Capsule = round_trip("capsule_open.json");
    assert_eq!(c.kind, CapsuleType::Session);
    assert_eq!(c.status, CapsuleStatus::Open);
    assert!(c.closed_at.is_none());
    assert!(c.summary_id.is_none());
    assert_eq!(c.observation_ids.len(), 2);
}

#[test]
fn capsule_closed_full() {
    let c: Capsule = round_trip("capsule_closed.json");
    assert_eq!(c.kind, CapsuleType::PocketflowNode);
    assert_eq!(c.status, CapsuleStatus::Closed);
    assert_eq!(c.closed_at, Some(1_746_662_460_000));
    assert_eq!(
        c.summary_id.as_deref(),
        Some("01J8XS7SUMMARY00000000000001")
    );
}

#[test]
fn capsule_type_field_uses_the_keyword_name_on_the_wire() {
    let c = Capsule {
        id: "x".into(),
        kind: CapsuleType::Session,
        intent: "demo".into(),
        status: CapsuleStatus::Open,
        opened_at: 0,
        closed_at: None,
        scope_ids: ScopeIds::default(),
        observation_ids: vec![],
        summary_id: None,
    };
    let v = serde_json::to_value(&c).unwrap();
    assert!(v.get("type").is_some(), "expected 'type' key, got {v}");
    assert!(v.get("kind").is_none());
}

#[test]
fn summary_round_trip() {
    let s: Summary = round_trip("summary.json");
    assert!(is_valid_confidence(s.confidence));
    assert_eq!(s.evidence_refs.len(), 2);
}

#[test]
fn is_valid_confidence_rejects_out_of_range_and_nan() {
    assert!(is_valid_confidence(0.0));
    assert!(is_valid_confidence(0.5));
    assert!(is_valid_confidence(1.0));
    assert!(!is_valid_confidence(-0.1));
    assert!(!is_valid_confidence(1.1));
    assert!(!is_valid_confidence(f64::NAN));
}

#[test]
fn pin_round_trip() {
    let p: Pin = round_trip("pin.json");
    assert_eq!(p.target_type, PinTargetType::Observation);
    assert_eq!(p.expires_at, Some(1_746_748_800_000));
    assert_eq!(p.reason.as_deref(), Some("Critical context for auth flow"));
}

#[test]
fn pin_active_when_no_expiry_and_when_future_expiry() {
    let mut p = Pin {
        id: "x".into(),
        target_type: PinTargetType::Observation,
        target_id: "y".into(),
        reason: None,
        created_at: 0,
        expires_at: None,
        scope_ids: ScopeIds::default(),
    };
    assert!(is_pin_active(&p, 1_000_000));
    p.expires_at = Some(2_000_000);
    assert!(is_pin_active(&p, 1_000_000));
    assert!(!is_pin_active(&p, 2_000_000));
    assert!(!is_pin_active(&p, 3_000_000));
}

#[test]
fn retrieve_result_with_observation_pin_and_summary_candidate() {
    let r: RetrieveResult = round_trip("retrieve_result.json");
    assert_eq!(r.pins.len(), 1);
    match &r.pins[0].target {
        RetrievedEntity::Observation(o) => assert_eq!(o.kind, ObservationKind::ToolCall),
        RetrievedEntity::Summary(_) => panic!("expected observation in pin target"),
    }
    assert_eq!(r.candidates.len(), 1);
    match &r.candidates[0].entity {
        RetrievedEntity::Summary(s) => {
            assert!((s.confidence - 0.82).abs() < f64::EPSILON);
        }
        RetrievedEntity::Observation(_) => panic!("expected summary in candidate"),
    }
    assert_eq!(r.provenance.provider_used, "local-fts");
    assert_eq!(r.provenance.total_candidates, 12);
}

#[test]
fn validation_error_with_value() {
    let v = ValidationError {
        field: "confidence".into(),
        message: "out of range".into(),
        value: Some(json!(1.5)),
    };
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json["field"], "confidence");
    assert_eq!(json["value"], 1.5);
    let parsed: ValidationError = serde_json::from_value(json).unwrap();
    assert_eq!(parsed, v);
}

#[test]
fn validation_error_omits_value_when_none() {
    let v = ValidationError {
        field: "missing".into(),
        message: "required".into(),
        value: None,
    };
    let json = serde_json::to_value(&v).unwrap();
    assert!(
        json.get("value").is_none(),
        "value must be absent, got: {json}"
    );
}
