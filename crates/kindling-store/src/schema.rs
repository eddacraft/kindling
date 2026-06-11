//! Embedded copies of the cross-language schema contract.
//!
//! `schema/schema.sql` and `schema/version.json` at the repo root are the
//! canonical contract shared with the TypeScript store. Embedding them with
//! `include_str!` means this crate can never drift from the contract — any
//! contract change recompiles into the crate on the next build.
//!
//! NOTE: the `include_str!` paths reach outside the crate directory, which is
//! fine for workspace builds but not for `cargo publish`. Packaging for
//! crates.io (PORT-014) will need a build-time copy step.

use std::sync::OnceLock;

use serde::Deserialize;

/// Canonical DDL — the state of the schema after all migrations.
pub const SCHEMA_SQL: &str = include_str!("../../../schema/schema.sql");

const VERSION_JSON: &str = include_str!("../../../schema/version.json");

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
