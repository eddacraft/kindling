//! Database open path: connection configuration and schema bootstrap.
//!
//! Mirrors `openDatabase` in
//! `packages/kindling-store-sqlite/src/db/open.ts`: WAL journal mode,
//! foreign-key enforcement, 5s busy timeout, NORMAL synchronous, 64MB cache.
//!
//! Where the TypeScript store runs its migration ladder, this crate applies
//! the canonical `schema/schema.sql` to fresh databases and refuses databases
//! whose `PRAGMA user_version` falls outside the compatibility window in
//! `schema/version.json` (see `schema/README.md`).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_millis(5000);

use crate::error::{StoreError, StoreResult};
use crate::schema::{schema_version, SCHEMA_SQL};

const MIGRATION_006_SCOPED_KEYSET_INDEXES: &str = r#"
BEGIN IMMEDIATE;
DROP INDEX IF EXISTS idx_obs_session_ts;
DROP INDEX IF EXISTS idx_obs_repo_ts;
CREATE INDEX idx_obs_session_ts
  ON observations(session_id, ts ASC, id ASC) WHERE session_id IS NOT NULL;
CREATE INDEX idx_obs_repo_ts
  ON observations(repo_id, ts ASC, id ASC) WHERE repo_id IS NOT NULL;
INSERT OR REPLACE INTO schema_migrations (version, name, applied_at)
  VALUES (6, '006_scoped_keyset_indexes', unixepoch('subsec') * 1000);
PRAGMA user_version = 6;
COMMIT;
"#;

/// Additive revision 7: `generic_events`, `generic_event_usage`, and usage triggers.
///
/// Runs inside `BEGIN IMMEDIATE`, uses `IF NOT EXISTS` and `INSERT OR IGNORE`,
/// and deliberately never writes `PRAGMA user_version` so racing 0.5 opens are
/// idempotent and 0.4.x binaries can still open the database.
const MIGRATION_007_GENERIC_EVENTS: &str = r#"
BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS schema_migrations (
  version     INTEGER PRIMARY KEY,
  name        TEXT    NOT NULL,
  applied_at  INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS generic_events (
  event_id          TEXT    PRIMARY KEY,
  envelope_version  INTEGER NOT NULL,
  schema_name       TEXT    NOT NULL,
  event_key         TEXT    NOT NULL,
  schema_version    INTEGER NOT NULL,
  project_id        TEXT    NOT NULL,
  session_id        TEXT,
  repo_id           TEXT,
  agent_id          TEXT,
  user_id           TEXT,
  task_id           TEXT,
  digest            TEXT    NOT NULL,
  payload           BLOB,
  committed_at      INTEGER NOT NULL,
  redacted_at       INTEGER,
  detector_version  INTEGER NOT NULL,
  UNIQUE(schema_name, event_key)
);
CREATE TABLE IF NOT EXISTS generic_event_usage (
  id            INTEGER PRIMARY KEY CHECK(id = 1),
  payload_bytes INTEGER NOT NULL DEFAULT 0,
  event_count   INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO generic_event_usage (id, payload_bytes, event_count)
  VALUES (1, 0, 0);
CREATE TRIGGER IF NOT EXISTS generic_events_usage_insert
AFTER INSERT ON generic_events
BEGIN
  UPDATE generic_event_usage
     SET payload_bytes = payload_bytes + COALESCE(length(CAST(NEW.payload AS BLOB)), 0),
         event_count   = event_count + 1
   WHERE id = 1;
END;
CREATE TRIGGER IF NOT EXISTS generic_events_usage_update
AFTER UPDATE ON generic_events
BEGIN
  UPDATE generic_event_usage
     SET payload_bytes = payload_bytes
                       - COALESCE(length(CAST(OLD.payload AS BLOB)), 0)
                       + COALESCE(length(CAST(NEW.payload AS BLOB)), 0)
   WHERE id = 1;
END;
CREATE TRIGGER IF NOT EXISTS generic_events_usage_delete
AFTER DELETE ON generic_events
BEGIN
  UPDATE generic_event_usage
     SET payload_bytes = payload_bytes - COALESCE(length(CAST(OLD.payload AS BLOB)), 0),
         event_count   = event_count - 1
   WHERE id = 1;
END;
INSERT OR IGNORE INTO schema_migrations (version, name, applied_at)
  VALUES (7, '007_generic_events', unixepoch('subsec') * 1000);
COMMIT;
"#;

/// Additive revision 8: `observation_meta`, a narrow copy of the observation
/// columns retrieval filters and ranks on, plus the triggers that keep it in
/// sync with `observations`.
///
/// Like 007 it runs inside `BEGIN IMMEDIATE`, is idempotent and never writes
/// `PRAGMA user_version`, so older binaries still open the database (their
/// writes fire the triggers). The backfill runs only while row 8 is absent, so
/// a racing opener that loses the lock does not copy the table again.
const MIGRATION_008_OBSERVATION_META: &str = r#"
BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS observation_meta (
  rowid       INTEGER PRIMARY KEY,
  id          TEXT    NOT NULL,
  ts          INTEGER NOT NULL,
  session_id  TEXT,
  repo_id     TEXT,
  agent_id    TEXT,
  user_id     TEXT,
  redacted    INTEGER NOT NULL
);
INSERT OR REPLACE INTO observation_meta (rowid, id, ts, session_id, repo_id, agent_id, user_id, redacted)
  SELECT rowid, id, ts, session_id, repo_id, agent_id, user_id, redacted FROM observations
   WHERE NOT EXISTS (SELECT 1 FROM schema_migrations WHERE version = 8);
CREATE TRIGGER IF NOT EXISTS observation_meta_insert
AFTER INSERT ON observations
BEGIN
  INSERT OR REPLACE INTO observation_meta (rowid, id, ts, session_id, repo_id, agent_id, user_id, redacted)
  VALUES (NEW.rowid, NEW.id, NEW.ts, NEW.session_id, NEW.repo_id, NEW.agent_id, NEW.user_id, NEW.redacted);
END;
CREATE TRIGGER IF NOT EXISTS observation_meta_update
AFTER UPDATE ON observations
BEGIN
  DELETE FROM observation_meta WHERE rowid = OLD.rowid;
  INSERT OR REPLACE INTO observation_meta (rowid, id, ts, session_id, repo_id, agent_id, user_id, redacted)
  VALUES (NEW.rowid, NEW.id, NEW.ts, NEW.session_id, NEW.repo_id, NEW.agent_id, NEW.user_id, NEW.redacted);
END;
CREATE TRIGGER IF NOT EXISTS observation_meta_delete
AFTER DELETE ON observations
BEGIN
  DELETE FROM observation_meta WHERE rowid = OLD.rowid;
END;
INSERT OR IGNORE INTO schema_migrations (version, name, applied_at)
  VALUES (8, '008_observation_meta', unixepoch('subsec') * 1000);
COMMIT;
"#;

/// Database open options.
#[derive(Debug, Clone, Default)]
pub struct StoreOptions {
    /// Open the database read-only. Schema bootstrap is skipped; opening an
    /// uninitialized database read-only is an error.
    pub readonly: bool,
}

/// Open and initialize a kindling database at `path`.
///
/// Creates parent directories as needed (read-write mode only), configures
/// the connection, applies the canonical schema to fresh databases, and
/// verifies schema-version compatibility on existing ones.
///
/// `path` is validated by [`validate_db_path`] before any directory is created
/// or SQLite is opened. The store still accepts any local filesystem location
/// (CLI `--db`, `KINDLING_DB_PATH`, tests) — it does not confine opens to a
/// single root — but it rejects traversal segments, NUL, and SQLite URI names.
pub fn open_database(path: &Path, options: &StoreOptions) -> StoreResult<Connection> {
    let path = validate_db_path(path)?;

    if !options.readonly {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
    }

    let conn = Connection::open_with_flags(&path, open_flags(options.readonly))?;
    configure(&conn, options.readonly)?;
    ensure_schema(&conn, options.readonly)?;
    Ok(conn)
}

/// Open flags for a filesystem database file.
///
/// Explicitly omits `SQLITE_OPEN_URI` (present on [`OpenFlags::default`]) so a
/// path cannot be reinterpreted as a SQLite URI (`file:…?mode=…`). `NOFOLLOW`
/// refuses to open when the final path component is a symlink.
fn open_flags(readonly: bool) -> OpenFlags {
    let flags = OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    if readonly {
        flags | OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        flags | OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    }
}

/// Validate a kindling database path before creating directories or opening
/// SQLite.
///
/// Kindling is local-first: `--db` and `KINDLING_DB_PATH` are operator
/// configuration and may point at any filesystem location. This helper does
/// **not** confine the path to a single root (that would break legitimate
/// local config and temp-dir tests). It rejects the cases that turn a path
/// into an injection:
///
/// - empty / non-UTF-8 / embedded NUL
/// - SQLite URI filenames (`file:…`) and special names (`:memory:`, `:temp:`)
/// - parent-directory segments (`..` as a whole `/` or `\` component)
///
/// Daemon routing already confines per-project DBs: `X-Kindling-Project` is
/// hashed to a 12-hex component under `kindling_home/projects/<id>/`.
pub fn validate_db_path(path: &Path) -> StoreResult<PathBuf> {
    let Some(raw) = path.to_str() else {
        return Err(StoreError::InvalidDbPath {
            path: path.display().to_string(),
            reason: "path is not valid UTF-8",
        });
    };
    if raw.is_empty() {
        return Err(StoreError::InvalidDbPath {
            path: raw.to_string(),
            reason: "path is empty",
        });
    }
    if raw.contains('\0') {
        return Err(StoreError::InvalidDbPath {
            path: raw.to_string(),
            reason: "path contains a NUL byte",
        });
    }
    if is_sqlite_special_filename(raw) {
        return Err(StoreError::InvalidDbPath {
            path: raw.to_string(),
            reason: "SQLite URI and special filenames are not allowed",
        });
    }
    // Reject `..` as a whole path component on both `/` and `\` so names like
    // `archive..old` stay valid while `../x` / `a\..\b` do not. Reconstruct
    // the PathBuf from this checked string so the sink cannot observe the
    // pre-check value.
    if contains_parent_dir_segment(raw) {
        return Err(StoreError::InvalidDbPath {
            path: raw.to_string(),
            reason: "path must not contain '..' segments",
        });
    }
    Ok(PathBuf::from(raw))
}

/// True when `raw` has a parent-directory segment: `..` as a whole component
/// on either `/` or `\`, so Windows-style traversal is rejected on every OS.
/// Adjacent dots inside a name (`my..db`, `a..b/kindling.db`) are not segments.
fn contains_parent_dir_segment(raw: &str) -> bool {
    raw.split(['/', '\\']).any(|component| component == "..")
}

fn is_sqlite_special_filename(raw: &str) -> bool {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("file:") || lower == ":memory:" || lower == ":temp:"
}

/// Open an in-memory database with the full schema applied. Test helper and
/// scratch-space convenience; never version-gated because it is always fresh.
pub fn open_in_memory() -> StoreResult<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn, false)?;
    ensure_schema(&conn, false)?;
    Ok(conn)
}

fn configure(conn: &Connection, readonly: bool) -> StoreResult<()> {
    // Set busy_timeout before any locking PRAGMA so concurrent openers wait
    // instead of failing immediately with SQLITE_BUSY.
    conn.busy_timeout(DEFAULT_BUSY_TIMEOUT)?;
    if !readonly {
        // journal_mode is persistent in the DB file; read-only connections
        // inherit it and may not change it. Switching a legacy DELETE-mode DB
        // to WAL upgrades a shared lock to exclusive, where SQLite returns
        // SQLITE_BUSY immediately (without invoking busy_timeout) when two
        // connections race. Bound all retries by a single busy-timeout deadline
        // and restore DEFAULT_BUSY_TIMEOUT afterward.
        ensure_wal_mode(conn, DEFAULT_BUSY_TIMEOUT)?;
    }
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "cache_size", -64000)?;
    Ok(())
}

fn ensure_wal_mode(conn: &Connection, timeout: Duration) -> StoreResult<()> {
    let deadline = Instant::now() + timeout;
    let retry_sleep = Duration::from_millis(10);

    let result = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        conn.busy_timeout(remaining)?;
        match conn.query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(_) => break Ok(()),
            Err(rusqlite::Error::SqliteFailure(err, msg))
                if err.code == rusqlite::ErrorCode::DatabaseBusy =>
            {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(err, msg)));
                }
                std::thread::sleep(retry_sleep.min(left));
            }
            Err(e) => break Err(e.into()),
        }
    };

    let restore = conn.busy_timeout(DEFAULT_BUSY_TIMEOUT);
    result?;
    restore?;
    Ok(())
}

/// Apply the canonical schema to a fresh database, or verify that an existing
/// database's `PRAGMA user_version` is within the supported window.
fn ensure_schema(conn: &Connection, readonly: bool) -> StoreResult<()> {
    let contract = schema_version();
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let has_tables: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'observations')",
        [],
        |row| row.get(0),
    )?;

    if !has_tables {
        if readonly {
            return Err(StoreError::UninitializedDatabase);
        }
        // Bootstrap atomically inside a single immediate transaction so an
        // interruption or disk-full error cannot leave schema_migrations row 7
        // committed without the generic_events tables and triggers.
        let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
        let already_initialized: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'observations')",
            [],
            |row| row.get(0),
        )?;
        if !already_initialized {
            tx.execute_batch(SCHEMA_SQL)?;
            tx.commit()?;
            return Ok(());
        }
        tx.commit()?;
        // Another opener initialized the DB while we waited on BEGIN IMMEDIATE;
        // fall through with fresh metadata to verify compatibility.
        return ensure_schema(conn, readonly);
    }

    // Pre-005 TypeScript databases have tables but user_version = 0; they
    // need the TS migration runner. Anything below minCompatible is refused.
    if user_version < contract.min_compatible || user_version == 0 {
        return Err(StoreError::SchemaTooOld {
            found: user_version,
            min_compatible: contract.min_compatible,
        });
    }
    if user_version > contract.version {
        return Err(StoreError::SchemaTooNew {
            found: user_version,
            supported: contract.version,
        });
    }
    if !readonly {
        if user_version == 5 {
            conn.execute_batch(MIGRATION_006_SCOPED_KEYSET_INDEXES)?;
        }
        let has_migrations_table: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
            [],
            |row| row.get(0),
        )?;
        let has_rev_7: bool = if has_migrations_table {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 7)",
                [],
                |row| row.get(0),
            )?
        } else {
            false
        };
        if !has_rev_7 {
            conn.execute_batch(MIGRATION_007_GENERIC_EVENTS)?;
        }
        let has_rev_8: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 8)",
            [],
            |row| row.get(0),
        )?;
        if !has_rev_8 {
            conn.execute_batch(MIGRATION_008_OBSERVATION_META)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_db_path_accepts_harmless_adjacent_dots() {
        for raw in [
            "my..db",
            "a..b/kindling.db",
            "/tmp/archive..old/kindling.db",
        ] {
            assert!(
                validate_db_path(Path::new(raw)).is_ok(),
                "expected {raw:?} to be accepted"
            );
        }
    }

    #[test]
    fn validate_db_path_rejects_parent_directory_segments() {
        for raw in ["../x", "a/../b", r"..\x", ".."] {
            assert!(
                matches!(
                    validate_db_path(Path::new(raw)),
                    Err(StoreError::InvalidDbPath { .. })
                ),
                "expected {raw:?} to be rejected as a parent-directory segment"
            );
        }
    }

    #[test]
    fn ensure_wal_mode_bounds_retries_by_single_deadline_and_restores_busy_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("locked.db");
        let holder = Connection::open(&path).unwrap();
        holder
            .execute_batch("CREATE TABLE t (x INTEGER); BEGIN EXCLUSIVE;")
            .unwrap();

        let contender = Connection::open(&path).unwrap();
        let start = Instant::now();
        let err = ensure_wal_mode(&contender, Duration::from_millis(120))
            .expect_err("exclusive lock must cause WAL switch to time out");
        let elapsed = start.elapsed();

        assert!(
            matches!(
                err,
                StoreError::Sqlite(rusqlite::Error::SqliteFailure(e, _))
                    if e.code == rusqlite::ErrorCode::DatabaseBusy
            ),
            "expected DatabaseBusy, got {err:?}"
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "WAL retry exceeded single deadline bound: {elapsed:?}"
        );
        let restored_timeout_ms: i64 = contender
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        assert_eq!(restored_timeout_ms, DEFAULT_BUSY_TIMEOUT.as_millis() as i64);

        holder.execute_batch("ROLLBACK;").unwrap();
    }

    #[test]
    fn fresh_bootstrap_is_atomic_under_concurrent_opens() {
        use std::sync::{Arc, Barrier};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fresh-race.db");
        let threads = 8;
        let barrier = Arc::new(Barrier::new(threads));
        let mut handles = Vec::with_capacity(threads);

        for _ in 0..threads {
            let p = path.clone();
            let b = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                b.wait();
                let conn = open_database(&p, &StoreOptions { readonly: false }).unwrap();
                let uv: i64 = conn
                    .query_row("PRAGMA user_version", [], |r| r.get(0))
                    .unwrap();
                assert_eq!(uv, 6);
                let has_ge: bool = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'generic_events')",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert!(has_ge);
            }));
        }

        for h in handles {
            h.join().expect("concurrent fresh open must succeed");
        }
    }
}
