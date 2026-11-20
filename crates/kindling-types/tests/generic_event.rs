//! Wire-shape, grammar-boundary, and strict base64 tests for `generic-event@1` (KINTEG-015a).

use kindling_types::{
    build_capability, validate_event_key, validate_generic_event_envelope,
    validate_generic_event_scope_ids, validate_schema_name, Capability, CapabilityFeature,
    CommitFailure, ConflictKind, GenericEvent, GenericEventForget, GenericEventLimits,
    GenericEventLookup, GenericEventReceipt, GenericEventScope, RejectReason, ScopeIds,
    GENERIC_EVENT_ENVELOPE_VERSION, GENERIC_EVENT_MAX_KEY_BYTES, GENERIC_EVENT_MAX_PAYLOAD_BYTES,
    GENERIC_EVENT_MAX_SCHEMA_NAME_BYTES, GENERIC_EVENT_MAX_SCOPE_VALUE_BYTES,
};
use serde_json::json;

fn sample_receipt() -> GenericEventReceipt {
    GenericEventReceipt {
        event_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        envelope_version: 1,
        schema_name: "example.audit".to_string(),
        schema_version: 3,
        key: "kfit/run-42/evt-7".to_string(),
        scope: GenericEventScope {
            project_id: "0123456789ab".to_string(),
            session_id: Some("s".to_string()),
            repo_id: None,
            agent_id: None,
            user_id: None,
            task_id: None,
        },
        digest: "sha256:d4391e955ce4cf16ae13fccb7bacb97f4263415f956f599c4e4beef35de9e57d"
            .to_string(),
        committed_at: 1_763_452_800_000,
    }
}

#[test]
fn generic_event_round_trips_with_base64_payload() {
    let event = GenericEvent {
        envelope_version: 1,
        schema_name: "example.audit".to_string(),
        schema_version: 3,
        key: "kfit/run-42/evt-7".to_string(),
        scope_ids: ScopeIds {
            session_id: Some("s".to_string()),
            ..Default::default()
        },
        payload: br#"{"k":"v"}"#.to_vec(),
    };

    let v = serde_json::to_value(&event).unwrap();
    assert_eq!(
        v,
        json!({
            "envelopeVersion": 1,
            "schemaName": "example.audit",
            "schemaVersion": 3,
            "key": "kfit/run-42/evt-7",
            "scopeIds": { "sessionId": "s" },
            "payload": "eyJrIjoidiJ9"
        })
    );

    let decoded: GenericEvent = serde_json::from_value(v).unwrap();
    assert_eq!(decoded, event);
}

#[test]
fn strict_base64_rejects_unpadded_url_alphabet_and_whitespace() {
    let base = |payload_str: &str| {
        json!({
            "envelopeVersion": 1,
            "schemaName": "a",
            "schemaVersion": 1,
            "key": "k",
            "scopeIds": {},
            "payload": payload_str
        })
    };

    // Valid padded standard base64 for [0xFB, 0xFF] is "+/8="
    let ok: GenericEvent = serde_json::from_value(base("+/8=")).unwrap();
    assert_eq!(ok.payload, vec![0xFB, 0xFF]);

    // Unpadded input ("+/8" instead of "+/8=") must fail.
    assert!(serde_json::from_value::<GenericEvent>(base("+/8")).is_err());

    // URL-safe alphabet ("-_8=" instead of "+/8=") must fail.
    assert!(serde_json::from_value::<GenericEvent>(base("-_8=")).is_err());

    // Whitespace (space, newline, tab) must fail.
    assert!(serde_json::from_value::<GenericEvent>(base(" +/8=")).is_err());
    assert!(serde_json::from_value::<GenericEvent>(base("+/8=\n")).is_err());
    assert!(serde_json::from_value::<GenericEvent>(base("+/ 8=")).is_err());
    assert!(serde_json::from_value::<GenericEvent>(base("eyJr\nIjoidiJ9")).is_err());
}

#[test]
fn receipt_lookup_and_forget_wire_shapes() {
    let receipt = sample_receipt();
    let v = serde_json::to_value(&receipt).unwrap();
    assert_eq!(
        v,
        json!({
            "eventId": "550e8400-e29b-41d4-a716-446655440000",
            "envelopeVersion": 1,
            "schemaName": "example.audit",
            "schemaVersion": 3,
            "key": "kfit/run-42/evt-7",
            "scope": { "projectId": "0123456789ab", "sessionId": "s" },
            "digest": "sha256:d4391e955ce4cf16ae13fccb7bacb97f4263415f956f599c4e4beef35de9e57d",
            "committedAt": 1763452800000i64
        })
    );
    assert_eq!(
        serde_json::from_value::<GenericEventReceipt>(v).unwrap(),
        receipt
    );

    let found = GenericEventLookup::Found {
        receipt: receipt.clone(),
        payload: br#"{"k":"v"}"#.to_vec(),
    };
    let found_json = serde_json::to_value(&found).unwrap();
    assert_eq!(found_json["state"], "found");
    assert_eq!(found_json["payload"], "eyJrIjoidiJ9");
    assert_eq!(
        serde_json::from_value::<GenericEventLookup>(found_json).unwrap(),
        found
    );

    let redacted = GenericEventLookup::Redacted {
        receipt: receipt.clone(),
        redacted_at: 1_763_456_400_000,
    };
    let redacted_json = serde_json::to_value(&redacted).unwrap();
    assert_eq!(redacted_json["state"], "redacted");
    assert_eq!(redacted_json["redactedAt"], 1_763_456_400_000i64);
    assert_eq!(
        serde_json::from_value::<GenericEventLookup>(redacted_json).unwrap(),
        redacted
    );

    let not_found_json = serde_json::to_value(&GenericEventLookup::NotFound).unwrap();
    assert_eq!(not_found_json, json!({ "state": "notFound" }));
    assert_eq!(
        serde_json::from_value::<GenericEventLookup>(not_found_json).unwrap(),
        GenericEventLookup::NotFound
    );

    let forget_redacted = GenericEventForget::Redacted {
        receipt: receipt.clone(),
        redacted_at: 1_763_456_400_000,
    };
    let forget_json = serde_json::to_value(&forget_redacted).unwrap();
    assert_eq!(forget_json["state"], "redacted");
    assert_eq!(forget_json["redactedAt"], 1_763_456_400_000i64);
    assert_eq!(
        serde_json::from_value::<GenericEventForget>(forget_json).unwrap(),
        forget_redacted
    );

    let forget_not_found = serde_json::to_value(&GenericEventForget::NotFound).unwrap();
    assert_eq!(forget_not_found, json!({ "state": "notFound" }));
}

#[test]
fn limits_omits_unset_quota_and_round_trips() {
    let without_quota = GenericEventLimits {
        max_payload_bytes: GENERIC_EVENT_MAX_PAYLOAD_BYTES,
        max_key_bytes: GENERIC_EVENT_MAX_KEY_BYTES,
        max_schema_name_bytes: GENERIC_EVENT_MAX_SCHEMA_NAME_BYTES,
        project_quota_bytes: None,
        secret_detector_version: 1,
    };
    let v = serde_json::to_value(&without_quota).unwrap();
    assert!(v.get("projectQuotaBytes").is_none());
    assert_eq!(
        serde_json::from_value::<GenericEventLimits>(v).unwrap(),
        without_quota
    );

    let with_quota = GenericEventLimits {
        project_quota_bytes: Some(1_073_741_824),
        ..without_quota
    };
    let v = serde_json::to_value(&with_quota).unwrap();
    assert_eq!(v["projectQuotaBytes"], 1_073_741_824u64);
    assert_eq!(
        serde_json::from_value::<GenericEventLimits>(v).unwrap(),
        with_quota
    );
}

#[test]
fn detail_enums_round_trip() {
    let reasons = [
        RejectReason::InvalidEnvelope {
            field: "key".to_string(),
            message: "bad key".to_string(),
        },
        RejectReason::TooLarge {
            limit: 262_144,
            actual: 262_145,
        },
        RejectReason::SecretDetected {
            field: "payload".to_string(),
            classes: vec!["openaiKey".to_string()],
            count: 1,
            detector_version: 1,
        },
        RejectReason::Unrecognized {
            code: "future_code".to_string(),
            message: "unknown".to_string(),
        },
    ];
    for r in reasons {
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(serde_json::from_value::<RejectReason>(v).unwrap(), r);
    }

    let conflicts = [
        ConflictKind::Mismatch {
            existing: sample_receipt(),
        },
        ConflictKind::Redacted {
            existing: sample_receipt(),
            redacted_at: 1_763_456_400_000,
        },
    ];
    for c in conflicts {
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(serde_json::from_value::<ConflictKind>(v).unwrap(), c);
    }

    for f in [CommitFailure::Busy, CommitFailure::Storage] {
        let v = serde_json::to_value(f).unwrap();
        assert_eq!(serde_json::from_value::<CommitFailure>(v).unwrap(), f);
    }
}

#[test]
fn schema_name_grammar_boundaries() {
    // 0 bytes -> invalid
    assert!(validate_schema_name("").is_err());
    // 1 byte -> valid
    assert!(validate_schema_name("a").is_ok());
    assert!(validate_schema_name("0").is_ok());
    // Leading punctuation or uppercase -> invalid
    assert!(validate_schema_name(".a").is_err());
    assert!(validate_schema_name("-a").is_err());
    assert!(validate_schema_name("_a").is_err());
    assert!(validate_schema_name("A").is_err());
    assert!(validate_schema_name("aA").is_err());
    assert!(validate_schema_name("a/b").is_err());
    // max (128 bytes) -> valid
    let max_name = "x".repeat(GENERIC_EVENT_MAX_SCHEMA_NAME_BYTES as usize);
    assert!(validate_schema_name(&max_name).is_ok());
    // max + 1 (129 bytes) -> invalid
    let over_name = "x".repeat(GENERIC_EVENT_MAX_SCHEMA_NAME_BYTES as usize + 1);
    assert!(validate_schema_name(&over_name).is_err());
}

#[test]
fn key_grammar_boundaries() {
    // 0 bytes -> invalid
    assert!(validate_event_key("").is_err());
    // 1 byte -> valid
    assert!(validate_event_key("!").is_ok());
    assert!(validate_event_key("~").is_ok());
    // Space (0x20), DEL (0x7F), control chars, non-ASCII -> invalid
    assert!(validate_event_key("a b").is_err());
    assert!(validate_event_key("a\tb").is_err());
    assert!(validate_event_key("a\nb").is_err());
    assert!(validate_event_key("a\x7fb").is_err());
    assert!(validate_event_key("café").is_err());
    // max (256 bytes) -> valid
    let max_key = "k".repeat(GENERIC_EVENT_MAX_KEY_BYTES as usize);
    assert!(validate_event_key(&max_key).is_ok());
    // max + 1 (257 bytes) -> invalid
    let over_key = "k".repeat(GENERIC_EVENT_MAX_KEY_BYTES as usize + 1);
    assert!(validate_event_key(&over_key).is_err());
}

#[test]
fn scope_ids_grammar_boundaries() {
    // Empty ScopeIds (all None) -> valid
    assert!(validate_generic_event_scope_ids(&ScopeIds::default()).is_ok());

    // Empty string "" -> rejected for every scope field
    assert!(validate_generic_event_scope_ids(&ScopeIds {
        session_id: Some(String::new()),
        ..Default::default()
    })
    .is_err());
    assert!(validate_generic_event_scope_ids(&ScopeIds {
        task_id: Some(String::new()),
        ..Default::default()
    })
    .is_err());

    // Control characters rejected
    assert!(validate_generic_event_scope_ids(&ScopeIds {
        repo_id: Some("repo\n1".to_string()),
        ..Default::default()
    })
    .is_err());
    assert!(validate_generic_event_scope_ids(&ScopeIds {
        agent_id: Some("agent\x001".to_string()),
        ..Default::default()
    })
    .is_err());

    // 1 byte, max (256 bytes) -> valid; max + 1 (257 bytes) -> invalid
    assert!(validate_generic_event_scope_ids(&ScopeIds {
        user_id: Some("u".to_string()),
        ..Default::default()
    })
    .is_ok());
    assert!(validate_generic_event_scope_ids(&ScopeIds {
        user_id: Some("u".repeat(GENERIC_EVENT_MAX_SCOPE_VALUE_BYTES)),
        ..Default::default()
    })
    .is_ok());
    assert!(validate_generic_event_scope_ids(&ScopeIds {
        user_id: Some("u".repeat(GENERIC_EVENT_MAX_SCOPE_VALUE_BYTES + 1)),
        ..Default::default()
    })
    .is_err());
}

#[test]
fn envelope_validation_checks_versions_and_payload_bounds() {
    let valid = GenericEvent {
        envelope_version: GENERIC_EVENT_ENVELOPE_VERSION,
        schema_name: "a".to_string(),
        schema_version: 1,
        key: "k".to_string(),
        scope_ids: ScopeIds::default(),
        payload: vec![0u8],
    };
    assert!(validate_generic_event_envelope(&valid, GENERIC_EVENT_MAX_PAYLOAD_BYTES).is_ok());

    // envelope_version != 1
    let mut bad_env = valid.clone();
    bad_env.envelope_version = 2;
    assert!(matches!(
        validate_generic_event_envelope(&bad_env, GENERIC_EVENT_MAX_PAYLOAD_BYTES),
        Err(RejectReason::InvalidEnvelope { field, .. }) if field == "envelopeVersion"
    ));

    // schema_version == 0
    let mut bad_sv = valid.clone();
    bad_sv.schema_version = 0;
    assert!(matches!(
        validate_generic_event_envelope(&bad_sv, GENERIC_EVENT_MAX_PAYLOAD_BYTES),
        Err(RejectReason::InvalidEnvelope { field, .. }) if field == "schemaVersion"
    ));

    // payload 0 bytes -> InvalidEnvelope
    let mut empty = valid.clone();
    empty.payload = Vec::new();
    assert!(matches!(
        validate_generic_event_envelope(&empty, GENERIC_EVENT_MAX_PAYLOAD_BYTES),
        Err(RejectReason::InvalidEnvelope { field, .. }) if field == "payload"
    ));

    // payload == max -> Ok
    let mut max = valid.clone();
    max.payload = vec![0u8; GENERIC_EVENT_MAX_PAYLOAD_BYTES as usize];
    assert!(validate_generic_event_envelope(&max, GENERIC_EVENT_MAX_PAYLOAD_BYTES).is_ok());

    // payload == max + 1 -> TooLarge
    let mut over = valid;
    over.payload = vec![0u8; GENERIC_EVENT_MAX_PAYLOAD_BYTES as usize + 1];
    assert_eq!(
        validate_generic_event_envelope(&over, GENERIC_EVENT_MAX_PAYLOAD_BYTES),
        Err(RejectReason::TooLarge {
            limit: GENERIC_EVENT_MAX_PAYLOAD_BYTES,
            actual: GENERIC_EVENT_MAX_PAYLOAD_BYTES + 1,
        })
    );
}

#[test]
fn capability_accepts_legacy_04_json_and_unknown_features() {
    // 0.4.x /v1/health payload without schemaRevision or features still deserializes.
    let legacy = json!({
        "version": "0.4.1",
        "schemaVersion": 6,
        "supportedKinds": ["tool_call"],
        "storagePath": "/tmp/kindling",
        "kindRegistry": []
    });
    let cap: Capability = serde_json::from_value(legacy).unwrap();
    assert_eq!(cap.schema_version, 6);
    assert_eq!(cap.schema_revision, None);
    assert!(cap.features.is_empty());

    // Unknown feature entries and unknown limits shapes do not fail parsing.
    let with_features = build_capability("0.5.0", 6, "/tmp/kindling")
        .with_schema_revision(7)
        .with_features(vec![CapabilityFeature {
            name: "future-feature".to_string(),
            version: 9,
            limits: Some(json!({ "custom": [1, 2, 3] })),
        }]);
    let v = serde_json::to_value(&with_features).unwrap();
    assert_eq!(v["schemaVersion"], 6);
    assert_eq!(v["schemaRevision"], 7);
    assert_eq!(
        serde_json::from_value::<Capability>(v).unwrap(),
        with_features
    );
}
