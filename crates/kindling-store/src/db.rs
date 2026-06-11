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

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::error::{StoreError, StoreResult};
use crate::schema::{schema_version, SCHEMA_SQL};

/// Database open options.
#[derive(Debug, Clone, Default)]
pub struct StoreOptions {
    /// Open the database read-only. Schema bootstrap is skipped; opening an
    /// uninitialized database read-only is an error.
    pub readonly: bool,
}

/// Open and initialize a Kindling database at `path`.
///
/// Creates parent directories as needed (read-write mode only), configures
/// the connection, applies the canonical schema to fresh databases, and
/// verifies schema-version compatibility on existing ones.
pub fn open_database(path: &Path, options: &StoreOptions) -> StoreResult<Connection> {
    if !options.readonly {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
    }

    let flags = if options.readonly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
    } else {
        OpenFlags::default()
    };

    let conn = Connection::open_with_flags(path, flags)?;
    configure(&conn, options.readonly)?;
    ensure_schema(&conn, options.readonly)?;
    Ok(conn)
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
    Ok(())
}
