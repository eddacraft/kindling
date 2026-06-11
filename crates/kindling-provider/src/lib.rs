//! Local FTS5 retrieval provider.
//!
//! Implements deterministic, tiered retrieval over the SQLite FTS5 index:
//! pins first (non-evictable), then the current capsule summary, then BM25-
//! ranked candidates normalised to [0, 1].
//!
//! The public surface mirrors `LocalFtsProvider` in
//! `packages/kindling-provider-local` and the retrieval orchestrator in
//! `packages/kindling-core/src/retrieval/orchestrator.ts` — identical queries
//! against the same database produce the same ranked results.

mod error;
mod orchestrator;
mod provider;

pub use error::{ProviderError, ProviderResult};
pub use orchestrator::{retrieve, retrieve_at};
pub use provider::{LocalFtsProvider, RetrievalProvider};
