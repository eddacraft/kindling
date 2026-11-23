//! Schema revision 8 tests (PERF-001): `observation_meta`.
//!
//! - A fresh database has the table, its triggers and migration row 8.
//! - A revision-7 database is backfilled on its first writable open, without
//!   changing `user_version`; a read-only open does not migrate.
//! - Migration 008 is idempotent under racing opens.
//! - The table stays identical to the matching `observations` columns through
//!   inserts, redaction, deletion, and writes from binaries that predate it.

use std::path::Path;
use std::sync::{Arc, Barrier};

use kindling_store::{open_database, SqliteKindlingStore, StoreOptions};
use rusqlite::Connection;

const META_COLUMNS: &str = "rowid, id, ts, session_id, repo_id, agent_id, user_id, redacted";

type MetaRow = (
    i64,
    Option<String>,
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
);

fn rows(conn: &Connection, table: &str) -> Vec<MetaRow> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {META_COLUMNS} FROM {table} ORDER BY rowid"
        ))
        .unwrap();
    stmt.query_map([], |r| {
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get(7)?,
        ))
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

/// `observation_meta` holds exactly the metadata of every observation.
fn assert_meta_matches(conn: &Connection) {
    assert_eq!(rows(conn, "observation_meta"), rows(conn, "observations"));
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

fn insert(conn: &Connection, id: &str, ts: i64, repo: Option<&str>) {
    conn.execute(
        "INSERT INTO observations (id, kind, content, provenance, ts, scope_ids, redacted, session_id, repo_id, agent_id, user_id)
         VALUES (?1, 'message', 'some content ' || ?1, '{}', ?2, '{}', 0, 's1', ?3, 'a1', NULL)",
        rusqlite::params![id, ts, repo],
    )
    .unwrap();
}

/// Turn a current database into a revision-7 one: no `observation_meta`, no
/// triggers, no migration row 8.
fn downgrade_to_revision_7(conn: &Connection) {
    conn.execute_batch(
        "DROP TRIGGER observation_meta_insert;
         DROP TRIGGER observation_meta_update;
         DROP TRIGGER observation_meta_delete;
         DROP TABLE observation_meta;
         DELETE FROM schema_migrations WHERE version = 8;",
    )
    .unwrap();
}

fn revision_7_db(path: &Path, observations: usize) {
    let conn = open_database(path, &StoreOptions { readonly: false }).unwrap();
    downgrade_to_revision_7(&conn);
    for i in 0..observations {
        let repo = if i % 2 == 0 { Some("r1") } else { None };
        insert(&conn, &format!("obs-{i}"), 1_000 + i as i64, repo);
    }
}

#[test]
fn fresh_database_has_observation_meta_and_row_8() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let conn = store.connection();
    assert!(has_table(conn, "observation_meta"));
    assert!(has_migration(conn, 8));
    for trigger in [
        "observation_meta_insert",
        "observation_meta_update",
        "observation_meta_delete",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'trigger' AND name = ?1)",
                [trigger],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "missing trigger {trigger}");
    }
}

#[test]
fn revision_7_database_is_backfilled_on_writable_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rev7.db");
    revision_7_db(&path, 50);

    // Read-only opens never migrate.
    let ro = open_database(&path, &StoreOptions { readonly: true }).unwrap();
    assert!(!has_table(&ro, "observation_meta"));
    assert!(!has_migration(&ro, 8));
    let uv_before: i64 = ro
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    drop(ro);

    let rw = open_database(&path, &StoreOptions { readonly: false }).unwrap();
    assert!(has_migration(&rw, 8));
    assert_eq!(rows(&rw, "observation_meta").len(), 50);
    assert_meta_matches(&rw);
    let uv_after: i64 = rw
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        uv_after, uv_before,
        "an additive revision keeps user_version"
    );

    // Later writes keep it in sync.
    insert(&rw, "obs-new", 9_999, Some("r2"));
    assert_meta_matches(&rw);
    drop(rw);

    // Reopening does not backfill twice.
    let rw2 = open_database(&path, &StoreOptions { readonly: false }).unwrap();
    assert_meta_matches(&rw2);
}

#[test]
fn migration_008_is_idempotent_under_racing_opens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("race.db");
    revision_7_db(&path, 20);

    let threads = 8;
    let barrier = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let p = path.clone();
            let b = Arc::clone(&barrier);
            std::thread::spawn(move || {
                b.wait();
                let conn = open_database(&p, &StoreOptions { readonly: false }).unwrap();
                assert!(has_migration(&conn, 8));
            })
        })
        .collect();
    for h in handles {
        h.join().expect("racing open thread must succeed");
    }

    let conn = open_database(&path, &StoreOptions { readonly: true }).unwrap();
    assert_meta_matches(&conn);
    let rev8_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 8",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rev8_rows, 1);
}

#[test]
fn observation_meta_tracks_insert_redact_and_delete() {
    let store = SqliteKindlingStore::open_in_memory().unwrap();
    let conn = store.connection();
    for i in 0..10 {
        insert(conn, &format!("obs-{i}"), 1_000 + i, Some("r1"));
    }
    assert_meta_matches(conn);

    // Redaction, as `forget` does it.
    conn.execute(
        "UPDATE observations SET content = '[redacted]', redacted = 1 WHERE id = 'obs-3'",
        [],
    )
    .unwrap();
    // Scope and timestamp changes.
    conn.execute(
        "UPDATE observations SET repo_id = 'r9', ts = 5 WHERE id = 'obs-4'",
        [],
    )
    .unwrap();
    // A rowid change.
    conn.execute(
        "UPDATE observations SET rowid = 1000 WHERE id = 'obs-5'",
        [],
    )
    .unwrap();
    conn.execute("DELETE FROM observations WHERE id = 'obs-6'", [])
        .unwrap();
    assert_meta_matches(conn);
}

#[test]
fn frozen_041_writes_keep_observation_meta_in_sync() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rev8.db");
    drop(SqliteKindlingStore::open(&path).unwrap());

    // A 0.4.1 binary knows nothing about observation_meta and never sets the
    // denormalised scope columns explicitly.
    let conn = Connection::open(&path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    let uv: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert!(
        (1..=6).contains(&uv),
        "0.4.1 gate rejects user_version {uv}"
    );

    conn.execute(
        "INSERT INTO observations (id, kind, content, provenance, ts, scope_ids, redacted)
         VALUES ('obs-041', 'message', 'written by 0.4.1', '{}', 1000, '{}', 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE observations SET content = '[redacted]', redacted = 1 WHERE id = 'obs-041'",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO observations (id, kind, content, provenance, ts, scope_ids, redacted)
         VALUES ('obs-041-b', 'message', 'kept', '{}', 1001, '{}', 0)",
        [],
    )
    .unwrap();
    conn.execute("DELETE FROM observations WHERE id = 'obs-041-b'", [])
        .unwrap();
    assert_meta_matches(&conn);
}

/// `observations.id` has no `NOT NULL`, so neither does the copy: a write the
/// base schema accepts must not start failing, and a revision-7 database that
/// already holds such a row must still migrate.
#[test]
fn null_ids_neither_fail_writes_nor_block_migration() {
    let null_id_insert =
        "INSERT INTO observations (id, kind, content, provenance, ts, scope_ids, redacted)
         VALUES (NULL, 'message', 'no id', '{}', 1000, '{}', 0)";

    let store = SqliteKindlingStore::open_in_memory().unwrap();
    store.connection().execute(null_id_insert, []).unwrap();
    assert_meta_matches(store.connection());

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rev7-null-id.db");
    revision_7_db(&path, 3);
    Connection::open(&path)
        .unwrap()
        .execute(null_id_insert, [])
        .unwrap();
    let rw = open_database(&path, &StoreOptions { readonly: false }).unwrap();
    assert!(has_migration(&rw, 8));
    assert_meta_matches(&rw);
}
