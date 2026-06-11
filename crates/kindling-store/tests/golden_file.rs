//! Golden-file test: a database created by the existing TypeScript store
//! (`packages/kindling-store-sqlite`) must be readable — and writable — by
//! the Rust store.
//!
//! The fixture is committed at `tests/fixtures/ts-golden.db`; regenerate it
//! with `node crates/kindling-store/tests/fixtures/generate-golden-db.mjs`
//! after a `pnpm run build` whenever the fixture script changes.

use std::path::PathBuf;

use kindling_types::{CapsuleStatus, CapsuleType, ObservationKind, PinTargetType, ScopeIds};

use kindling_store::SqliteKindlingStore;

/// Copy the committed fixture into a temp dir so the test never mutates it.
fn open_golden() -> (tempfile::TempDir, SqliteKindlingStore) {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ts-golden.db");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ts-golden.db");
    std::fs::copy(&fixture, &path).unwrap();
    let store = SqliteKindlingStore::open(&path).unwrap();
    (dir, store)
}

#[test]
fn reads_observations_written_by_typescript() {
    let (_dir, store) = open_golden();

    let obs = store.get_observation_by_id("obs-1").unwrap().unwrap();
    assert_eq!(obs.kind, ObservationKind::ToolCall);
    assert_eq!(obs.content, "ran the database migration against staging");
    assert_eq!(obs.ts, 1700000000001);
    assert_eq!(obs.scope_ids.session_id.as_deref(), Some("sess-golden"));
    assert_eq!(obs.scope_ids.repo_id.as_deref(), Some("repo-golden"));
    assert_eq!(obs.provenance["tool"], serde_json::json!("Bash"));
    assert_eq!(obs.provenance["exitCode"], serde_json::json!(0));
    assert!(!obs.redacted);

    let redacted = store
        .get_observation_by_id("obs-redacted")
        .unwrap()
        .unwrap();
    assert!(redacted.redacted);
    assert_eq!(redacted.content, "[redacted]");
}

#[test]
fn reads_capsules_written_by_typescript() {
    let (_dir, store) = open_golden();

    let closed = store.get_capsule("cap-1").unwrap().unwrap();
    assert_eq!(closed.kind, CapsuleType::Session);
    assert_eq!(closed.status, CapsuleStatus::Closed);
    assert_eq!(closed.opened_at, 1700000000000);
    assert_eq!(closed.closed_at, Some(1700000001000));
    assert_eq!(closed.observation_ids, vec!["obs-1", "obs-2"]);

    let open = store
        .get_open_capsule_for_session("sess-other")
        .unwrap()
        .unwrap();
    assert_eq!(open.id, "cap-2");
    assert_eq!(open.kind, CapsuleType::PocketflowNode);
}

#[test]
fn reads_summaries_written_by_typescript() {
    let (_dir, store) = open_golden();

    let summary = store
        .get_latest_summary_for_capsule("cap-1")
        .unwrap()
        .unwrap();
    assert_eq!(summary.id, "sum-1");
    assert_eq!(
        summary.content,
        "migrated the database and hit a deploy timeout"
    );
    assert!((summary.confidence - 0.95).abs() < f64::EPSILON);
    assert_eq!(summary.evidence_refs, vec!["obs-1", "obs-2"]);
}

#[test]
fn reads_pins_written_by_typescript_with_ttl() {
    let (_dir, store) = open_golden();

    // pin-expired (expires 1700000000900) is filtered out at this `now`.
    let pins = store.list_active_pins(None, Some(1700000001000)).unwrap();
    let ids: Vec<_> = pins.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["pin-2", "pin-1"]); // newest first
    assert_eq!(pins[0].target_type, PinTargetType::Summary);
    assert_eq!(pins[1].target_type, PinTargetType::Observation);
    assert_eq!(pins[1].reason.as_deref(), Some("migration reference"));
}

#[test]
fn fts_index_built_by_typescript_is_queryable() {
    let (_dir, store) = open_golden();

    // Porter stemming: 'migration' in content matches the query 'migrating'.
    let hits: i64 = store
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM observations_fts WHERE observations_fts MATCH 'migrating'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(hits, 1);

    // The redacted observation's content never reaches the index.
    let secret_hits: i64 = store
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM observations_fts WHERE observations_fts MATCH 'secret'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(secret_hits, 0);
}

#[test]
fn scoped_queries_against_typescript_data() {
    let (_dir, store) = open_golden();

    let scoped = store
        .query_observations(
            Some(&ScopeIds {
                session_id: Some("sess-golden".to_string()),
                ..Default::default()
            }),
            None,
            None,
            100,
        )
        .unwrap();
    // obs-1 and obs-2; obs-redacted is excluded, obs-3 is another session.
    let ids: Vec<_> = scoped.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, vec!["obs-2", "obs-1"]);
}

#[test]
fn rust_writes_into_typescript_database() {
    let (_dir, store) = open_golden();

    store
        .insert_observation(&kindling_types::Observation {
            id: "obs-rust".to_string(),
            kind: ObservationKind::Command,
            content: "written by the rust store".to_string(),
            provenance: serde_json::Map::new(),
            ts: 1700000003000,
            scope_ids: ScopeIds {
                session_id: Some("sess-golden".to_string()),
                ..Default::default()
            },
            redacted: false,
        })
        .unwrap();
    store
        .attach_observation_to_capsule("cap-2", "obs-rust")
        .unwrap();

    let obs = store.get_observation_by_id("obs-rust").unwrap().unwrap();
    assert_eq!(obs.content, "written by the rust store");

    // The FTS insert trigger defined by the TS migrations fires for Rust
    // writes too — same triggers, same database.
    let hits: i64 = store
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM observations_fts WHERE observations_fts MATCH 'rust'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(hits, 1);
}
