//! Schema revision 7 tests (KINTEG-015a):
//! - Migration 007 idempotence under racing opens
//! - No-coupling golden test (no FK, trigger, CHECK, or FTS coupling to existing tables)
//! - `generic_event_usage` trigger accounting
//! - Frozen 0.4.1 open gate compatibility (accepts revision-7 DB; INSERT/UPDATE/DELETE observations)
//! - Every open-rule row for `user_version` 0, 1–4, 5, 6, 7, and above the current revision (writable & read-only)

use std::path::Path;
use std::sync::{Arc, Barrier};

use kindling_store::{open_database, SqliteKindlingStore, StoreError, StoreOptions};
use rusqlite::Connection;

/// Create a minimal pre-007 database with `observations` and `schema_migrations`
/// at the given `user_version` and migration rows `1..=max_migration`.
fn create_legacy_db(path: &Path, user_version: i64, max_migration: i64) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE schema_migrations (
          version     INTEGER PRIMARY KEY,
          name        TEXT    NOT NULL,
          applied_at  INTEGER NOT NULL
        );
        CREATE TABLE observations (
          id          TEXT    PRIMARY KEY,
          kind        TEXT    NOT NULL,
          content     TEXT    NOT NULL,
          provenance  TEXT    NOT NULL DEFAULT '{}',
          ts          INTEGER NOT NULL,
          scope_ids   TEXT    NOT NULL DEFAULT '{}',
          redacted    INTEGER NOT NULL DEFAULT 0,
          session_id  TEXT,
          repo_id     TEXT,
          agent_id    TEXT,
          user_id     TEXT
        );
        "#,
    )
    .unwrap();
    for v in 1..=max_migration {
        conn.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, 0)",
            rusqlite::params![v, format!("00{v}_legacy")],
        )
        .unwrap();
    }
    conn.pragma_update(None, "user_version", user_version)
        .unwrap();
}

fn has_table(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [name],
        |row| row.get(0),
    )
    .unwrap()
}

fn has_migration(conn: &Connection, version: i64) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
        [version],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn open_rule_user_version_0_refused_writable_and_readonly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v0.db");
    create_legacy_db(&path, 0, 4);

    assert!(matches!(
        open_database(&path, &StoreOptions { readonly: false }),
        Err(StoreError::SchemaTooOld { found: 0, .. })
    ));
    assert!(matches!(
        open_database(&path, &StoreOptions { readonly: true }),
        Err(StoreError::SchemaTooOld { found: 0, .. })
    ));
}

#[test]
fn open_rule_user_version_1_to_4() {
    for uv in 1..=4 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("v{uv}.db"));
        create_legacy_db(&path, uv, uv);

        // Read-only opens without migrating.
        let ro = open_database(&path, &StoreOptions { readonly: true }).unwrap();
        assert!(!has_table(&ro, "generic_events"));
        assert!(!has_migration(&ro, 6));
        assert!(!has_migration(&ro, 7));
        drop(ro);

        // Writable opens without running 006, runs 007, and does not change user_version.
        let rw = open_database(&path, &StoreOptions { readonly: false }).unwrap();
        let after_uv: i64 = rw
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(after_uv, uv, "user_version {uv} must not be altered");
        assert!(!has_migration(&rw, 6), "006 must not run for v{uv}");
        assert!(has_migration(&rw, 7), "007 must run for v{uv}");
        assert!(has_table(&rw, "generic_events"));
        assert!(has_table(&rw, "generic_event_usage"));
    }
}

#[test]
fn open_rule_user_version_5() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v5.db");
    create_legacy_db(&path, 5, 5);

    // Read-only opens without migrating.
    let ro = open_database(&path, &StoreOptions { readonly: true }).unwrap();
    assert!(!has_migration(&ro, 6));
    assert!(!has_migration(&ro, 7));
    drop(ro);

    // Writable runs 006 (raising user_version to 6) then 007.
    let rw = open_database(&path, &StoreOptions { readonly: false }).unwrap();
    let after_uv: i64 = rw
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(after_uv, 6);
    assert!(has_migration(&rw, 6));
    assert!(has_migration(&rw, 7));
    assert!(has_table(&rw, "generic_events"));
    assert!(has_table(&rw, "generic_event_usage"));
}

#[test]
fn open_rule_user_version_6_and_7_missing_and_present_row_7() {
    for uv in [6, 7] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("v{uv}.db"));
        create_legacy_db(&path, uv, 6);

        // Read-only opens without migrating when row 7 is missing.
        let ro = open_database(&path, &StoreOptions { readonly: true }).unwrap();
        assert!(!has_table(&ro, "generic_events"));
        assert!(!has_migration(&ro, 7));
        drop(ro);

        // Writable runs 007 and never lowers user_version.
        let rw = open_database(&path, &StoreOptions { readonly: false }).unwrap();
        let after_uv: i64 = rw
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(after_uv, uv, "user_version {uv} must be preserved");
        assert!(has_migration(&rw, 7));
        assert!(has_table(&rw, "generic_events"));
        drop(rw);

        // Second writable & read-only opens with row 7 present succeed cleanly.
        let rw2 = open_database(&path, &StoreOptions { readonly: false }).unwrap();
        assert!(has_migration(&rw2, 7));
        drop(rw2);

        let ro2 = open_database(&path, &StoreOptions { readonly: true }).unwrap();
        assert!(has_migration(&ro2, 7));
    }
}

#[test]
fn open_rule_user_version_above_current_revision_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v9.db");
    create_legacy_db(&path, 9, 7);

    assert!(matches!(
        open_database(&path, &StoreOptions { readonly: false }),
        Err(StoreError::SchemaTooNew {
            found: 9,
            supported: 8,
        })
    ));
    assert!(matches!(
        open_database(&path, &StoreOptions { readonly: true }),
        Err(StoreError::SchemaTooNew {
            found: 9,
            supported: 8,
        })
    ));
}

#[test]
fn migration_007_is_idempotent_under_racing_opens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("race.db");
    create_legacy_db(&path, 6, 6);

    let threads = 8;
    let barrier = Arc::new(Barrier::new(threads));
    let mut handles = Vec::with_capacity(threads);

    for _ in 0..threads {
        let p = path.clone();
        let b = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            b.wait();
            let conn = open_database(&p, &StoreOptions { readonly: false }).unwrap();
            assert!(has_table(&conn, "generic_events"));
            assert!(has_table(&conn, "generic_event_usage"));
            assert!(has_migration(&conn, 7));
        }));
    }

    for h in handles {
        h.join().expect("racing open thread must succeed");
    }

    let conn = open_database(&path, &StoreOptions { readonly: true }).unwrap();
    let usage_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM generic_event_usage", [], |r| r.get(0))
        .unwrap();
    assert_eq!(usage_rows, 1);
    let rev7_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 7",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rev7_rows, 1);
}

#[test]
fn no_coupling_golden_test() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let conn = store.connection();

    // 1. Neither generic_events nor generic_event_usage has any foreign keys.
    for table in ["generic_events", "generic_event_usage"] {
        let mut stmt = conn
            .prepare(&format!("PRAGMA foreign_key_list({table})"))
            .unwrap();
        let fk_count = stmt.query_map([], |_| Ok(())).unwrap().count();
        assert_eq!(fk_count, 0, "{table} must have no foreign keys");
    }

    // 2. No FTS table mentions generic_events.
    let fts_coupling: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND sql LIKE '%fts5%' AND sql LIKE '%generic_events%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fts_coupling, 0, "generic_events must not be indexed by FTS");

    // 3. Triggers on generic_events only reference generic_event_usage (never core tables).
    let core_tables = [
        "observations",
        "capsules",
        "capsule_observations",
        "summaries",
        "pins",
        "observations_fts",
        "summaries_fts",
    ];
    let mut stmt = conn
        .prepare("SELECT name, tbl_name, sql FROM sqlite_master WHERE type = 'trigger'")
        .unwrap();
    let triggers: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    for (name, tbl_name, sql) in &triggers {
        let lower = sql.to_ascii_lowercase();
        if tbl_name == "generic_events" || tbl_name == "generic_event_usage" {
            for core in core_tables {
                assert!(
                    !lower.contains(core),
                    "trigger {name} on {tbl_name} must not reference core table {core}"
                );
            }
        } else {
            assert!(
                !lower.contains("generic_events") && !lower.contains("generic_event_usage"),
                "core trigger {name} on {tbl_name} must not reference generic_events"
            );
        }
    }

    // 4. Table DDL for generic_events has no CHECK referencing another table.
    let ddl: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'generic_events'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!ddl.to_ascii_uppercase().contains("REFERENCES"));
    assert!(!ddl.to_ascii_uppercase().contains("CHECK"));
}

#[test]
fn generic_event_usage_triggers_track_live_bytes_and_tombstones() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let conn = store.connection();

    let usage = |c: &Connection| -> (i64, i64) {
        c.query_row(
            "SELECT payload_bytes, event_count FROM generic_event_usage WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    };

    assert_eq!(usage(conn), (0, 0));

    // Insert a live row with 5-byte payload.
    conn.execute(
        "INSERT INTO generic_events (event_id, envelope_version, schema_name, event_key, schema_version, project_id, digest, payload, committed_at, detector_version)
         VALUES ('e1', 1, 'example.audit', 'k1', 1, '0123456789ab', 'sha256:00', X'0102030405', 1000, 1)",
        [],
    )
    .unwrap();
    assert_eq!(usage(conn), (5, 1));

    // Insert a tombstone row (NULL payload): 0 bytes, count increments.
    conn.execute(
        "INSERT INTO generic_events (event_id, envelope_version, schema_name, event_key, schema_version, project_id, digest, payload, committed_at, redacted_at, detector_version)
         VALUES ('e2', 1, 'example.audit', 'k2', 1, '0123456789ab', 'sha256:00', NULL, 1000, 2000, 1)",
        [],
    )
    .unwrap();
    assert_eq!(usage(conn), (5, 2));

    // Tombstone e1 via UPDATE: payload_bytes drops to 0, event_count stays 2.
    conn.execute(
        "UPDATE generic_events SET payload = NULL, redacted_at = 3000 WHERE event_id = 'e1'",
        [],
    )
    .unwrap();
    assert_eq!(usage(conn), (0, 2));

    // Delete e2: count drops to 1.
    conn.execute("DELETE FROM generic_events WHERE event_id = 'e2'", [])
        .unwrap();
    assert_eq!(usage(conn), (0, 1));
}

#[test]
fn frozen_041_gate_opens_revision_7_db_and_mutates_observations() {
    // Fresh revision-7 DB created by 0.5.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rev7.db");
    {
        let store = SqliteKindlingStore::open(&path).unwrap();
        // Seed a generic event row to prove its presence doesn't interfere with 0.4.1.
        store
            .connection()
            .execute(
                "INSERT INTO generic_events (event_id, envelope_version, schema_name, event_key, schema_version, project_id, digest, payload, committed_at, detector_version)
                 VALUES ('e1', 1, 'example.audit', 'k1', 1, '0123456789ab', 'sha256:00', X'0102', 1000, 1)",
                [],
            )
            .unwrap();
    }

    // Frozen v0.4.1 open gate: accepts user_version in 1..=6 (contract.version == 6, minCompatible == 1).
    let conn = Connection::open(&path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    let uv: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert!(
        (1..=6).contains(&uv),
        "0.4.1 gate requires 1 <= user_version <= 6, got {uv}"
    );

    // 0.4.1 INSERT, UPDATE, and DELETE on observations must all succeed.
    conn.execute(
        "INSERT INTO observations (id, kind, content, provenance, ts, scope_ids, redacted)
         VALUES ('obs-041', 'message', 'written by 0.4.1', '{}', 1000, '{}', 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE observations SET content = 'updated by 0.4.1' WHERE id = 'obs-041'",
        [],
    )
    .unwrap();
    conn.execute("DELETE FROM observations WHERE id = 'obs-041'", [])
        .unwrap();

    // Soft-redaction (forget) also succeeds.
    conn.execute(
        "INSERT INTO observations (id, kind, content, provenance, ts, scope_ids, redacted)
         VALUES ('obs-041-forget', 'message', 'to redact', '{}', 1001, '{}', 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE observations SET content = '[redacted]', redacted = 1 WHERE id = 'obs-041-forget'",
        [],
    )
    .unwrap();

    // Generic event row is untouched.
    let ge_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM generic_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ge_count, 1);
}
