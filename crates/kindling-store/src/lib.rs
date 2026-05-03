//! SQLite persistence layer for Kindling.
//!
//! Implements the cross-language schema contract from `schema/schema.sql`
//! against `rusqlite` with the `bundled` SQLite + FTS5. WAL mode enabled,
//! per-project DB isolation under `~/.kindling/projects/<hash>/`.
//!
//! Filled in by PORT-003.
