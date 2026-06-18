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
    /// Current schema version; matches `PRAGMA user_version` in a migrated DB.
    pub version: i64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_json_parses() {
        let v = schema_version();
        assert!(v.version >= v.min_compatible);
        assert_eq!(v.fts_tokenizer, "porter unicode61");
    }

    #[test]
    fn schema_sql_pins_user_version() {
        let v = schema_version();
        let pragma = format!("PRAGMA user_version = {};", v.version);
        assert!(
            SCHEMA_SQL.contains(&pragma),
            "schema.sql must set PRAGMA user_version to match version.json"
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
