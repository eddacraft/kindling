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
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

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
    if !readonly {
        // journal_mode is persistent in the DB file; read-only connections
        // inherit it and may not change it.
        let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    }
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(Duration::from_millis(5000))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "cache_size", -64000)?;
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
        conn.execute_batch(SCHEMA_SQL)?;
        return Ok(());
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
    if user_version == 5 && contract.version == 6 && !readonly {
        conn.execute_batch(MIGRATION_006_SCOPED_KEYSET_INDEXES)?;
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
}
