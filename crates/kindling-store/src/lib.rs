//! SQLite persistence layer for kindling.
//!
//! Implements the cross-language schema contract from `schema/schema.sql`
//! against `rusqlite` with bundled SQLite + FTS5. WAL mode enabled,
//! per-project DB isolation under `~/.kindling/projects/<hash>/`.
//!
//! The public surface mirrors `SqliteKindlingStore` in
//! `packages/kindling-store-sqlite` — a database written by either
//! implementation is readable by the other.

mod db;
mod error;
mod paths;
mod schema;
mod store;

pub use db::{open_database, open_in_memory, StoreOptions};
pub use error::{StoreError, StoreResult};
pub use paths::{default_kindling_home, project_db_path, project_id, resolve_db_path};
pub use schema::{schema_version, SchemaVersion, SCHEMA_SQL};
pub use store::{DatabaseStats, EvidenceSnippet, SqliteKindlingStore, DEFAULT_SNIPPET_MAX_CHARS};
