//! Embedded copies of the cross-language schema contract.
//!
//! `schema/schema.sql` and `schema/version.json` at the repo root are the
//! canonical contract shared with the TypeScript store. This crate vendors a
//! COMMITTED copy of them under `crates/kindling-store/schema/` and embeds the
//! vendored copy with `include_str!`. Vendoring keeps the files inside the
//! crate directory so `cargo publish` packages them (the repo-root canonical
//! source is outside the crate dir and would not be in the published tarball).
//!
//! The vendored copy is kept in lock-step with the canonical source by
//! `scripts/sync-vendored-schema.sh`; a CI drift gate (the `vendored-schema`
//! job in `.github/workflows/rust.yml`) re-runs the sync and fails on any
//! uncommitted diff, so the crate can never silently drift from the contract.

use std::sync::OnceLock;

use serde::Deserialize;

/// Canonical DDL — the state of the schema after all migrations.
///
/// Embedded from the vendored copy (`crates/kindling-store/schema/schema.sql`),
/// kept in sync with the repo-root canonical `schema/schema.sql`.
pub const SCHEMA_SQL: &str = include_str!("../schema/schema.sql");

const VERSION_JSON: &str = include_str!("../schema/version.json");

/// Machine-readable schema version metadata from `schema/version.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaVersion {
    /// Current schema revision; matches `MAX(schema_migrations.version)` after migrations.
    pub version: i64,
    /// Lowest schema revision a binary must know to open the DB (`PRAGMA user_version`).
    pub min_reader: i64,
    /// HTTP `/v1/health` wire contract version (`Capability.schemaVersion`).
    pub wire_version: u32,
    /// Oldest schema version this implementation can safely read.
    pub min_compatible: i64,
    /// Pinned FTS5 tokenizer — changing it is a breaking change.
    pub fts_tokenizer: String,
}

/// The schema version this crate was compiled against.
pub fn schema_version() -> &'static SchemaVersion {
    static VERSION: OnceLock<SchemaVersion> = OnceLock::new();
    VERSION.get_or_init(|| {
        serde_json::from_str(VERSION_JSON).expect("schema/version.json is valid JSON")
    })
}

/// Wire contract version (`wireVersion` from `schema/version.json`) reported as
/// `Capability.schemaVersion` by `/v1/health` and `kindling status`.
pub fn wire_version() -> u32 {
    schema_version().wire_version
}

/// Latest additive schema revision (`version` from `schema/version.json`)
/// reported as `Capability.schemaRevision` by `/v1/health` and `kindling status`.
pub fn schema_revision() -> u32 {
    schema_version().version as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_json_parses() {
        let v = schema_version();
        assert!(v.version >= v.min_reader);
        assert!(v.min_reader >= v.min_compatible);
        assert_eq!(v.version, 7);
        assert_eq!(v.min_reader, 6);
        assert_eq!(v.wire_version, 6);
        assert_eq!(wire_version(), 6);
        assert_eq!(schema_revision(), 7);
        assert_eq!(v.fts_tokenizer, "porter unicode61");
    }

    #[test]
    fn schema_sql_pins_user_version() {
        let v = schema_version();
        let pragma = format!("PRAGMA user_version = {};", v.min_reader);
        assert!(
            SCHEMA_SQL.contains(&pragma),
            "schema.sql must set PRAGMA user_version to match version.json minReader"
        );
        let migration_seed = format!("({}, '007_generic_events',", v.version);
        assert!(
            SCHEMA_SQL.contains(&migration_seed),
            "schema.sql must seed schema_migrations row for version.json revision"
        );
    }

    #[test]
    fn schema_sql_pins_tokenizer() {
        let v = schema_version();
        let tokenize = format!("tokenize='{}'", v.fts_tokenizer);
        assert!(
            SCHEMA_SQL.contains(&tokenize),
            "schema.sql FTS tables must use the tokenizer pinned in version.json"
        );
    }
}
