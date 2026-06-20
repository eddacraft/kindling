//! Snapshot/parity tests for the export-bundle JSON shape.
//!
//! # Reference provenance
//!
//! These goldens are **hand-derived** from the TypeScript source, NOT generated
//! by running the built TS CLI. The TS `better-sqlite3` native module does not
//! build under the Node v26 toolchain available in this environment (a known
//! caveat recorded in the project memory), so the TS CLI cannot be executed
//! here to emit a reference. The expected JSON below is derived field-by-field
//! from:
//!
//! * `packages/kindling-core/src/export/bundle.ts` — `createExportBundle`
//!   (top-level `{ bundleVersion, exportedAt, dataset, metadata? }` order;
//!   `metadata` appended after `dataset`).
//! * `packages/kindling-store-sqlite/src/store/export.ts` — `exportDatabase`
//!   (`dataset = { version, exportedAt, scope?, observations, capsules,
//!   summaries, pins }`; entity orderings).
//! * `packages/kindling-core/src/types/*.ts` — entity field order/casing.
//!
//! The CLI builds the bundle deterministically (fixed ids/timestamps via the
//! service `export` API, which takes an explicit `exported_at`), so the
//! serialized JSON is asserted byte-for-byte against the hand-derived string.
//! When the TS toolchain can run again, swap the constant for a TS-generated
//! reference and keep the assertion.

use kindling_service::{ExportBundleOptions, KindlingService};
use kindling_types::{
    Capsule, CapsuleStatus, CapsuleType, Observation, ObservationKind, Pin, PinTargetType,
    ScopeIds, Summary,
};

/// Build a service seeded with one of each entity, all with fixed ids and
/// timestamps so the export is fully deterministic.
fn seeded_service() -> KindlingService {
    let service = KindlingService::open_in_memory().unwrap();
    let store = service.store();

    let scope = ScopeIds {
        session_id: Some("sess-1".to_string()),
        ..Default::default()
    };

    let mut provenance = serde_json::Map::new();
    provenance.insert(
        "source".to_string(),
        serde_json::Value::String("cli".to_string()),
    );

    store
        .insert_observation(&Observation {
            id: "obs-1".to_string(),
            kind: ObservationKind::Message,
            content: "hello".to_string(),
            provenance,
            ts: 1000,
            scope_ids: scope.clone(),
            redacted: false,
        })
        .unwrap();

    store
        .create_capsule(&Capsule {
            id: "cap-1".to_string(),
            kind: CapsuleType::Session,
            intent: "do work".to_string(),
            status: CapsuleStatus::Closed,
            opened_at: 900,
            closed_at: Some(2000),
            scope_ids: scope.clone(),
            observation_ids: vec![],
            summary_id: None,
        })
        .unwrap();
    store
        .attach_observation_to_capsule("cap-1", "obs-1")
        .unwrap();

    store
        .insert_summary(&Summary {
            id: "sum-1".to_string(),
            capsule_id: "cap-1".to_string(),
            content: "a summary".to_string(),
            confidence: 1.0,
            created_at: 1500,
            evidence_refs: vec![],
        })
        .unwrap();

    store
        .insert_pin(&Pin {
            id: "pin-1".to_string(),
            target_type: PinTargetType::Observation,
            target_id: "obs-1".to_string(),
            reason: Some("keep".to_string()),
            created_at: 1200,
            expires_at: None,
            scope_ids: scope,
        })
        .unwrap();

    service
}

#[test]
fn export_bundle_json_matches_ts_shape() {
    let service = seeded_service();

    let bundle = service
        .export(ExportBundleOptions {
            scope: None,
            include_redacted: false,
            limit: None,
            metadata: None,
            exported_at: 1_700_000_000_000,
        })
        .unwrap();

    let json = bundle.to_json(false).unwrap();

    // Hand-derived reference (compact, key order per the TS sources above).
    let expected = concat!(
        r#"{"bundleVersion":"1.0","exportedAt":1700000000000,"dataset":{"#,
        r#""version":"1.0","exportedAt":1700000000000,"#,
        r#""observations":[{"id":"obs-1","kind":"message","content":"hello","#,
        r#""provenance":{"source":"cli"},"ts":1000,"#,
        r#""scopeIds":{"sessionId":"sess-1"},"redacted":false}],"#,
        r#""capsules":[{"id":"cap-1","type":"session","intent":"do work","#,
        r#""status":"closed","openedAt":900,"closedAt":2000,"#,
        r#""scopeIds":{"sessionId":"sess-1"},"observationIds":["obs-1"]}],"#,
        r#""summaries":[{"id":"sum-1","capsuleId":"cap-1","content":"a summary","#,
        r#""confidence":1.0,"createdAt":1500,"evidenceRefs":[]}],"#,
        r#""pins":[{"id":"pin-1","targetType":"observation","targetId":"obs-1","#,
        r#""reason":"keep","createdAt":1200,"scopeIds":{"sessionId":"sess-1"}}]}}"#,
    );

    assert_eq!(json, expected);
}

#[test]
fn export_with_metadata_places_it_after_dataset() {
    let service = seeded_service();
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "description".to_string(),
        serde_json::Value::String("Kindling memory export".to_string()),
    );

    let bundle = service
        .export(ExportBundleOptions {
            scope: None,
            include_redacted: false,
            limit: None,
            metadata: Some(metadata),
            exported_at: 42,
        })
        .unwrap();

    let json = bundle.to_json(false).unwrap();
    // `metadata` must appear AFTER `dataset` (TS sets it last).
    let dataset_pos = json.find("\"dataset\"").unwrap();
    let metadata_pos = json.find("\"metadata\"").unwrap();
    assert!(
        metadata_pos > dataset_pos,
        "metadata must serialize after dataset to match TS field order"
    );
    assert!(json.ends_with(r#""metadata":{"description":"Kindling memory export"}}"#));
}

#[test]
fn bundle_round_trips_through_from_json() {
    let service = seeded_service();
    let bundle = service
        .export(ExportBundleOptions {
            scope: None,
            include_redacted: false,
            limit: None,
            metadata: None,
            exported_at: 7,
        })
        .unwrap();

    let json = bundle.to_json(false).unwrap();
    let parsed = kindling_service::ExportBundle::from_json(&json).unwrap();
    assert_eq!(parsed, bundle);
    // Re-serialization is stable (deterministic key order).
    assert_eq!(parsed.to_json(false).unwrap(), json);
}
