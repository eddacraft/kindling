//! Integration tests for the SQLite store against temporary databases.

use std::path::Path;

use kindling_types::{
    Capsule, CapsuleStatus, CapsuleType, Observation, ObservationKind, Pin, PinTargetType,
    ScopeIds, Summary,
};

use kindling_store::{schema_version, SqliteKindlingStore, StoreError, StoreOptions};

fn scope(session: &str) -> ScopeIds {
    ScopeIds {
        session_id: Some(session.to_string()),
        repo_id: Some("repo-1".to_string()),
        agent_id: None,
        user_id: None,
        task_id: None,
    }
}

fn observation(id: &str, content: &str, ts: i64, session: &str) -> Observation {
    Observation {
        id: id.to_string(),
        kind: ObservationKind::ToolCall,
        content: content.to_string(),
        provenance: serde_json::Map::new(),
        ts,
        scope_ids: scope(session),
        redacted: false,
    }
}

fn capsule(id: &str, session: &str, opened_at: i64) -> Capsule {
    Capsule {
        id: id.to_string(),
        kind: CapsuleType::Session,
        intent: "test capsule".to_string(),
        status: CapsuleStatus::Open,
        opened_at,
        closed_at: None,
        scope_ids: scope(session),
        observation_ids: Vec::new(),
        summary_id: None,
    }
}

fn count(store: &SqliteKindlingStore, sql: &str) -> i64 {
    store
        .connection()
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

#[test]
fn fresh_database_gets_canonical_schema() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteKindlingStore::open(&dir.path().join("kindling.db")).unwrap();

    let user_version: i64 = store
        .connection()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(user_version, schema_version().version);

    // Migration history is seeded so the TS runner sees a complete schema.
    assert_eq!(
        count(&store, "SELECT COUNT(*) FROM schema_migrations"),
        schema_version().version
    );

    let journal_mode: String = store
        .connection()
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal_mode, "wal");
}

#[test]
fn observation_roundtrip_and_fts_sync() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let mut obs = observation("obs-1", "deployed the flux capacitor", 1000, "sess-1");
    obs.provenance
        .insert("tool".to_string(), serde_json::json!("Bash"));
    store.insert_observation(&obs).unwrap();

    let loaded = store.get_observation_by_id("obs-1").unwrap().unwrap();
    assert_eq!(loaded, obs);

    // FTS insert trigger indexed the content (porter stemming: deployed -> deploy).
    assert_eq!(
        count(
            &store,
            "SELECT COUNT(*) FROM observations_fts WHERE observations_fts MATCH 'deploying'"
        ),
        1
    );
}

#[test]
fn redaction_masks_content_and_drops_fts_row() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation("obs-1", "secret launch codes", 1000, "s"))
        .unwrap();

    store.redact_observation("obs-1").unwrap();

    let loaded = store.get_observation_by_id("obs-1").unwrap().unwrap();
    assert!(loaded.redacted);
    assert_eq!(loaded.content, "[redacted]");
    assert_eq!(
        count(
            &store,
            "SELECT COUNT(*) FROM observations_fts WHERE observations_fts MATCH 'launch'"
        ),
        0
    );

    assert!(matches!(
        store.redact_observation("missing"),
        Err(StoreError::ObservationNotFound(_))
    ));
}

#[test]
fn capsule_lifecycle() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .create_capsule(&capsule("cap-1", "sess-1", 1000))
        .unwrap();

    let open = store
        .get_open_capsule_for_session("sess-1")
        .unwrap()
        .unwrap();
    assert_eq!(open.id, "cap-1");
    assert_eq!(open.status, CapsuleStatus::Open);

    store.close_capsule("cap-1", Some(2000), None).unwrap();
    assert!(store
        .get_open_capsule_for_session("sess-1")
        .unwrap()
        .is_none());

    let closed = store.get_capsule("cap-1").unwrap().unwrap();
    assert_eq!(closed.status, CapsuleStatus::Closed);
    assert_eq!(closed.closed_at, Some(2000));

    // Closing again (or closing a missing capsule) errors.
    assert!(matches!(
        store.close_capsule("cap-1", Some(3000), None),
        Err(StoreError::CapsuleNotOpen(_))
    ));
    assert!(matches!(
        store.close_capsule("missing", None, None),
        Err(StoreError::CapsuleNotOpen(_))
    ));
}

#[test]
fn close_capsule_validates_summary() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .create_capsule(&capsule("cap-1", "sess-1", 1000))
        .unwrap();

    assert!(matches!(
        store.close_capsule("cap-1", Some(2000), Some("missing-summary")),
        Err(StoreError::SummaryNotFound { .. })
    ));

    // The status update itself succeeded before summary validation (mirrors
    // the TS behaviour, where the UPDATE runs first).
    store
        .create_capsule(&capsule("cap-2", "sess-2", 1000))
        .unwrap();
    store
        .insert_summary(&Summary {
            id: "sum-1".to_string(),
            capsule_id: "cap-2".to_string(),
            content: "did things".to_string(),
            confidence: 0.9,
            created_at: 1500,
            evidence_refs: vec![],
        })
        .unwrap();
    store
        .close_capsule("cap-2", Some(2000), Some("sum-1"))
        .unwrap();
}

#[test]
fn attach_preserves_deterministic_order() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .create_capsule(&capsule("cap-1", "sess-1", 1000))
        .unwrap();
    for (i, id) in ["obs-b", "obs-a", "obs-c"].iter().enumerate() {
        store
            .insert_observation(&observation(id, "content", 1000 + i as i64, "sess-1"))
            .unwrap();
        store.attach_observation_to_capsule("cap-1", id).unwrap();
    }

    let loaded = store.get_capsule("cap-1").unwrap().unwrap();
    assert_eq!(loaded.observation_ids, vec!["obs-b", "obs-a", "obs-c"]);
}

#[test]
fn summaries_roundtrip_and_lookup() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .create_capsule(&capsule("cap-1", "sess-1", 1000))
        .unwrap();
    store
        .create_capsule(&capsule("cap-2", "sess-2", 1000))
        .unwrap();

    for (id, capsule_id, created_at) in [("sum-1", "cap-1", 1500_i64), ("sum-2", "cap-2", 2500_i64)]
    {
        store
            .insert_summary(&Summary {
                id: id.to_string(),
                capsule_id: capsule_id.to_string(),
                content: format!("summary {id}"),
                confidence: 0.8,
                created_at,
                evidence_refs: vec!["obs-1".to_string()],
            })
            .unwrap();
    }

    // The schema enforces one summary per capsule (capsule_id UNIQUE).
    let second_for_cap_1 = store.insert_summary(&Summary {
        id: "sum-dup".to_string(),
        capsule_id: "cap-1".to_string(),
        content: "duplicate".to_string(),
        confidence: 0.5,
        created_at: 3000,
        evidence_refs: vec![],
    });
    assert!(second_for_cap_1.is_err());

    let latest = store
        .get_latest_summary_for_capsule("cap-1")
        .unwrap()
        .unwrap();
    assert_eq!(latest.id, "sum-1");
    assert_eq!(latest.evidence_refs, vec!["obs-1"]);

    let by_id = store.get_summary_by_id("sum-1").unwrap().unwrap();
    assert_eq!(by_id.content, "summary sum-1");

    // summaries_fts trigger indexed both rows.
    assert_eq!(
        count(
            &store,
            "SELECT COUNT(*) FROM summaries_fts WHERE summaries_fts MATCH 'summary'"
        ),
        2
    );
}

#[test]
fn pins_respect_ttl_and_scope() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let pin = |id: &str, session: &str, expires_at: Option<i64>| Pin {
        id: id.to_string(),
        target_type: PinTargetType::Observation,
        target_id: "obs-1".to_string(),
        reason: Some("important".to_string()),
        created_at: 1000,
        expires_at,
        scope_ids: scope(session),
    };

    store.insert_pin(&pin("pin-live", "sess-1", None)).unwrap();
    store
        .insert_pin(&pin("pin-future", "sess-1", Some(5000)))
        .unwrap();
    store
        .insert_pin(&pin("pin-expired", "sess-1", Some(1500)))
        .unwrap();
    store.insert_pin(&pin("pin-other", "sess-2", None)).unwrap();

    let active = store.list_active_pins(None, Some(2000)).unwrap();
    let ids: Vec<_> = active.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids.len(), 3);
    assert!(!ids.contains(&"pin-expired"));

    let scoped = store
        .list_active_pins(
            Some(&ScopeIds {
                session_id: Some("sess-1".to_string()),
                ..Default::default()
            }),
            Some(2000),
        )
        .unwrap();
    let scoped_ids: Vec<_> = scoped.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(scoped_ids, vec!["pin-live", "pin-future"]);

    store.delete_pin("pin-live").unwrap();
    assert!(matches!(
        store.delete_pin("pin-live"),
        Err(StoreError::PinNotFound(_))
    ));
}

#[test]
fn query_observations_filters_scope_time_and_redacted() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation("obs-1", "first", 1000, "sess-1"))
        .unwrap();
    store
        .insert_observation(&observation("obs-2", "second", 2000, "sess-1"))
        .unwrap();
    store
        .insert_observation(&observation("obs-3", "third", 3000, "sess-2"))
        .unwrap();
    store
        .insert_observation(&observation("obs-4", "fourth", 4000, "sess-1"))
        .unwrap();
    store.redact_observation("obs-4").unwrap();

    // Newest first, redacted excluded.
    let all = store.query_observations(None, None, None, 100).unwrap();
    let ids: Vec<_> = all.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, vec!["obs-3", "obs-2", "obs-1"]);

    let scoped = store
        .query_observations(
            Some(&ScopeIds {
                session_id: Some("sess-1".to_string()),
                ..Default::default()
            }),
            None,
            None,
            100,
        )
        .unwrap();
    assert_eq!(scoped.len(), 2);

    let windowed = store
        .query_observations(None, Some(1500), Some(2500), 100)
        .unwrap();
    assert_eq!(windowed.len(), 1);
    assert_eq!(windowed[0].id, "obs-2");

    let limited = store.query_observations(None, None, None, 1).unwrap();
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].id, "obs-3");
}

#[test]
fn evidence_snippets_truncate_and_preserve_order() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store
        .insert_observation(&observation("obs-long", &"x".repeat(250), 1000, "s"))
        .unwrap();
    store
        .insert_observation(&observation("obs-short", "short", 2000, "s"))
        .unwrap();

    let snippets = store
        .get_evidence_snippets(
            &[
                "obs-short".to_string(),
                "missing".to_string(),
                "obs-long".to_string(),
            ],
            200,
        )
        .unwrap();

    assert_eq!(snippets.len(), 2);
    assert_eq!(snippets[0].observation_id, "obs-short");
    assert_eq!(snippets[0].snippet, "short");
    assert_eq!(snippets[1].observation_id, "obs-long");
    assert_eq!(snippets[1].snippet.len(), 203); // 200 chars + "..."
    assert!(snippets[1].snippet.ends_with("..."));

    assert!(store.get_evidence_snippets(&[], 200).unwrap().is_empty());
}

#[test]
fn transaction_rolls_back_on_error() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();

    let result: Result<(), _> = store.transaction(|s| {
        s.insert_observation(&observation("obs-1", "inside tx", 1000, "s"))?;
        Err(StoreError::ObservationNotFound("forced".to_string()))
    });
    assert!(result.is_err());
    assert!(store.get_observation_by_id("obs-1").unwrap().is_none());

    store
        .transaction(|s| s.insert_observation(&observation("obs-2", "committed", 2000, "s")))
        .unwrap();
    assert!(store.get_observation_by_id("obs-2").unwrap().is_some());
}

#[test]
fn rejects_newer_schema_versions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kindling.db");
    // Create a valid DB, then bump its user_version past the supported one.
    {
        let store = SqliteKindlingStore::open(&path).unwrap();
        let future = schema_version().version + 1;
        store
            .connection()
            .execute_batch(&format!("PRAGMA user_version = {future};"))
            .unwrap();
    }
    assert!(matches!(
        SqliteKindlingStore::open(&path),
        Err(StoreError::SchemaTooNew { .. })
    ));
}

#[test]
fn rejects_pre_contract_databases() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kindling.db");
    // Simulate a pre-005 TS database: tables exist but user_version is 0.
    {
        let store = SqliteKindlingStore::open(&path).unwrap();
        store
            .connection()
            .execute_batch("PRAGMA user_version = 0;")
            .unwrap();
    }
    assert!(matches!(
        SqliteKindlingStore::open(&path),
        Err(StoreError::SchemaTooOld { found: 0, .. })
    ));
}

#[test]
fn readonly_refuses_uninitialized_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kindling.db");
    // An empty file is a valid (schemaless) SQLite database.
    std::fs::write(&path, b"").unwrap();
    assert!(matches!(
        SqliteKindlingStore::open_with_options(&path, &StoreOptions { readonly: true }),
        Err(StoreError::UninitializedDatabase)
    ));
}

#[test]
fn readonly_reads_an_initialized_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kindling.db");
    {
        let store = SqliteKindlingStore::open(&path).unwrap();
        store
            .insert_observation(&observation("obs-1", "hello", 1000, "sess-1"))
            .unwrap();
    }
    let ro =
        SqliteKindlingStore::open_with_options(&path, &StoreOptions { readonly: true }).unwrap();
    assert!(ro.get_observation_by_id("obs-1").unwrap().is_some());
    assert!(ro
        .insert_observation(&observation("obs-2", "nope", 2000, "sess-1"))
        .is_err());
}

#[test]
fn concurrent_connections_share_a_wal_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kindling.db");
    let writer_a = SqliteKindlingStore::open(&path).unwrap();
    let writer_b = SqliteKindlingStore::open(&path).unwrap();

    writer_a
        .insert_observation(&observation("obs-a", "from a", 1000, "sess-1"))
        .unwrap();
    writer_b
        .insert_observation(&observation("obs-b", "from b", 2000, "sess-1"))
        .unwrap();

    assert!(writer_a.get_observation_by_id("obs-b").unwrap().is_some());
    assert!(writer_b.get_observation_by_id("obs-a").unwrap().is_some());
}

#[test]
fn per_project_paths_isolate_databases() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path_a = kindling_store::project_db_path(home, "/proj/a");
    let path_b = kindling_store::project_db_path(home, "/proj/b");
    assert_ne!(path_a, path_b);

    let store_a = SqliteKindlingStore::open(&path_a).unwrap();
    let store_b = SqliteKindlingStore::open(&path_b).unwrap();
    store_a
        .insert_observation(&observation("obs-a", "only in a", 1000, "s"))
        .unwrap();
    assert!(store_b.get_observation_by_id("obs-a").unwrap().is_none());
    assert!(Path::exists(&home.join("projects")));
}
