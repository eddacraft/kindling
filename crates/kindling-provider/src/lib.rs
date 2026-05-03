//! Local FTS5 retrieval provider.
//!
//! Implements deterministic, tiered retrieval over the SQLite FTS5 index:
//! pins first (non-evictable), then the current capsule summary, then BM25-
//! ranked candidates normalised to [0, 1].
//!
//! Filled in by PORT-005.
