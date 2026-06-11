//! Local FTS-based retrieval provider.
//!
//! Mirrors `LocalFtsProvider` in
//! `packages/kindling-provider-local/src/provider/local-fts.ts`: FTS
//! matching, scope filtering, and recency are computed in SQL; BM25
//! normalization is done in Rust across both entity types so scores are
//! comparable between observations and summaries.
//!
//! Scoring formula:
//!
//! ```text
//! score = (fts_relevance * 0.7) + (recency_score * 0.3)
//! ```
//!
//! where `fts_relevance` is the BM25 rank normalized to `[0, 1]` across all
//! results and `recency_score = MAX(0, 1.0 - age_ms / max_age_ms)`.
//!
//! The one deliberate API deviation from TypeScript: the TS provider reads
//! `Date.now()` internally, while [`RetrievalProvider::search`] takes `now`
//! explicitly so retrieval is deterministic and testable.

use rusqlite::types::Value as SqlValue;
use rusqlite::{params_from_iter, Connection};

use kindling_store::SqliteKindlingStore;
use kindling_types::{
    Id, Observation, ObservationKind, ProviderSearchOptions, ProviderSearchResult, RetrievedEntity,
    ScopeIds, Summary, Timestamp,
};

use crate::error::{ProviderError, ProviderResult};

/// Max age in ms for recency scoring (30 days).
const MAX_AGE_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Default result cap when `max_results` is not given.
const DEFAULT_MAX_RESULTS: u32 = 50;

/// Match-context preview length in UTF-16 code units (JS `string.length`).
const MATCH_CONTEXT_UNITS: usize = 100;

/// A retrieval provider: named source of ranked search results.
///
/// Mirrors the `RetrievalProvider` interface in
/// `packages/kindling-core/src/types/retrieval.ts`, with `now` made explicit
/// (epoch ms) instead of read from the system clock inside the provider.
pub trait RetrievalProvider {
    /// Provider name, recorded in retrieval provenance.
    fn name(&self) -> &str;

    /// Ranked search results for `options`, scored as of `now` (epoch ms).
    fn search(
        &self,
        options: &ProviderSearchOptions,
        now: Timestamp,
    ) -> ProviderResult<Vec<ProviderSearchResult>>;
}

/// Row returned by the raw observations query (pre-normalization).
struct RawObsRow {
    id: String,
    kind: String,
    content: String,
    provenance: String,
    ts: Timestamp,
    scope_ids: String,
    redacted: i64,
    fts_rank: f64,
    recency: f64,
}

/// Row returned by the raw summaries query (pre-normalization).
struct RawSumRow {
    id: String,
    capsule_id: String,
    content: String,
    confidence: f64,
    evidence_refs: String,
    created_at: Timestamp,
    fts_rank: f64,
    recency: f64,
}

/// FTS5 + recency retrieval over a Kindling SQLite database.
pub struct LocalFtsProvider<'conn> {
    conn: &'conn Connection,
}

impl<'conn> LocalFtsProvider<'conn> {
    /// Provider name, as recorded in retrieval provenance.
    pub const NAME: &'static str = "local-fts";

    pub fn new(conn: &'conn Connection) -> Self {
        Self { conn }
    }

    /// Borrow the connection of an open store.
    pub fn from_store(store: &'conn SqliteKindlingStore) -> Self {
        Self::new(store.connection())
    }

    /// Search observations: raw rows with `fts_rank` and `recency`. BM25
    /// normalization happens in the caller across all entity types.
    fn search_observations_raw(
        &self,
        query: &str,
        scope: &ScopeIds,
        exclude_ids: &[Id],
        include_redacted: bool,
        now: Timestamp,
        limit: u32,
    ) -> ProviderResult<Vec<RawObsRow>> {
        let (scope_clauses, scope_params) = build_scope_filters(scope, "o");
        let exclude_filter = exclude_id_filter("o", exclude_ids);
        let redacted_filter = if include_redacted {
            ""
        } else {
            "AND o.redacted = 0"
        };
        let scope_filter = if scope_clauses.is_empty() {
            String::new()
        } else {
            format!("AND {}", scope_clauses.join(" AND "))
        };

        let sql = format!(
            "WITH fts_hits AS (
               SELECT rowid, rank FROM observations_fts WHERE content MATCH ?
             )
             SELECT
               o.id, o.kind, o.content, o.provenance, o.ts, o.scope_ids, o.redacted,
               f.rank AS fts_rank,
               MAX(0.0, 1.0 - CAST(? - o.ts AS REAL) / ?) AS recency
             FROM fts_hits f
             JOIN observations o ON f.rowid = o.rowid
             WHERE 1=1
               {redacted_filter}
               {scope_filter}
               {exclude_filter}
             ORDER BY f.rank ASC
             LIMIT ?"
        );

        let mut params: Vec<SqlValue> = vec![
            SqlValue::Text(query.to_string()),
            SqlValue::Integer(now),
            SqlValue::Integer(MAX_AGE_MS),
        ];
        params.extend(scope_params);
        params.extend(exclude_ids.iter().map(|id| SqlValue::Text(id.clone())));
        params.push(SqlValue::Integer(i64::from(limit)));

        let result = (|| -> Result<Vec<RawObsRow>, rusqlite::Error> {
            let mut stmt = self.conn.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(params), |row| {
                Ok(RawObsRow {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    content: row.get(2)?,
                    provenance: row.get(3)?,
                    ts: row.get(4)?,
                    scope_ids: row.get(5)?,
                    redacted: row.get(6)?,
                    fts_rank: row.get(7)?,
                    recency: row.get(8)?,
                })
            })?;
            rows.collect()
        })();

        match result {
            Ok(rows) => Ok(rows),
            Err(err) if is_fts_syntax_error(&err) => Ok(Vec::new()),
            Err(err) => Err(err.into()),
        }
    }

    /// Search summaries: raw rows with `fts_rank` and `recency`. BM25
    /// normalization happens in the caller across all entity types.
    fn search_summaries_raw(
        &self,
        query: &str,
        scope: &ScopeIds,
        exclude_ids: &[Id],
        now: Timestamp,
        limit: u32,
    ) -> ProviderResult<Vec<RawSumRow>> {
        let (scope_clauses, scope_params) = build_scope_filters(scope, "c");
        let exclude_filter = exclude_id_filter("s", exclude_ids);
        let scope_filter = if scope_clauses.is_empty() {
            String::new()
        } else {
            format!("AND {}", scope_clauses.join(" AND "))
        };

        let sql = format!(
            "WITH fts_hits AS (
               SELECT rowid, rank FROM summaries_fts WHERE content MATCH ?
             )
             SELECT
               s.id, s.capsule_id, s.content, s.confidence, s.evidence_refs, s.created_at,
               f.rank AS fts_rank,
               MAX(0.0, 1.0 - CAST(? - s.created_at AS REAL) / ?) AS recency
             FROM fts_hits f
             JOIN summaries s ON f.rowid = s.rowid
             JOIN capsules c ON s.capsule_id = c.id
             WHERE 1=1
               {scope_filter}
               {exclude_filter}
             ORDER BY f.rank ASC
             LIMIT ?"
        );

        let mut params: Vec<SqlValue> = vec![
            SqlValue::Text(query.to_string()),
            SqlValue::Integer(now),
            SqlValue::Integer(MAX_AGE_MS),
        ];
        params.extend(scope_params);
        params.extend(exclude_ids.iter().map(|id| SqlValue::Text(id.clone())));
        params.push(SqlValue::Integer(i64::from(limit)));

        let result = (|| -> Result<Vec<RawSumRow>, rusqlite::Error> {
            let mut stmt = self.conn.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(params), |row| {
                Ok(RawSumRow {
                    id: row.get(0)?,
                    capsule_id: row.get(1)?,
                    content: row.get(2)?,
                    confidence: row.get(3)?,
                    evidence_refs: row.get(4)?,
                    created_at: row.get(5)?,
                    fts_rank: row.get(6)?,
                    recency: row.get(7)?,
                })
            })?;
            rows.collect()
        })();

        match result {
            Ok(rows) => Ok(rows),
            Err(err) if is_fts_syntax_error(&err) => Ok(Vec::new()),
            Err(err) => Err(err.into()),
        }
    }
}

impl RetrievalProvider for LocalFtsProvider<'_> {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn search(
        &self,
        options: &ProviderSearchOptions,
        now: Timestamp,
    ) -> ProviderResult<Vec<ProviderSearchResult>> {
        let max_results = options.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        let exclude_ids = options.exclude_ids.as_deref().unwrap_or(&[]);
        let include_redacted = options.include_redacted.unwrap_or(false);

        let obs_raw = self.search_observations_raw(
            &options.query,
            &options.scope_ids,
            exclude_ids,
            include_redacted,
            now,
            max_results,
        )?;
        let sum_raw = self.search_summaries_raw(
            &options.query,
            &options.scope_ids,
            exclude_ids,
            now,
            max_results,
        )?;

        // Normalize BM25 ranks across BOTH result sets so scores are
        // comparable. FTS5 rank is negative; more negative = more relevant.
        let mut min_rank = f64::INFINITY;
        let mut max_rank = f64::NEG_INFINITY;
        for rank in obs_raw
            .iter()
            .map(|r| r.fts_rank)
            .chain(sum_raw.iter().map(|r| r.fts_rank))
        {
            min_rank = min_rank.min(rank);
            max_rank = max_rank.max(rank);
        }
        let rank_range = if min_rank <= max_rank {
            max_rank - min_rank
        } else {
            0.0 // no rows at all; loop below never runs
        };
        let normalize_fts = |rank: f64| -> f64 {
            if rank_range == 0.0 {
                0.5 // Unknown relative relevance
            } else {
                (max_rank - rank) / rank_range
            }
        };
        let score_of = |fts_rank: f64, recency: f64| -> f64 {
            let score = (normalize_fts(fts_rank) * 0.7 + recency * 0.3).clamp(0.0, 1.0);
            (score * 1e10).round() / 1e10
        };

        let mut results: Vec<ProviderSearchResult> =
            Vec::with_capacity(obs_raw.len() + sum_raw.len());

        for row in obs_raw {
            results.push(ProviderSearchResult {
                score: score_of(row.fts_rank, row.recency),
                match_context: Some(match_context(&row.content)),
                entity: RetrievedEntity::Observation(Observation {
                    id: row.id,
                    kind: parse_observation_kind(&row.kind)?,
                    provenance: serde_json::from_str(&row.provenance)?,
                    ts: row.ts,
                    scope_ids: serde_json::from_str(&row.scope_ids)?,
                    redacted: row.redacted == 1,
                    content: row.content,
                }),
            });
        }

        for row in sum_raw {
            results.push(ProviderSearchResult {
                score: score_of(row.fts_rank, row.recency),
                match_context: Some(match_context(&row.content)),
                entity: RetrievedEntity::Summary(Summary {
                    id: row.id,
                    capsule_id: row.capsule_id,
                    confidence: row.confidence,
                    created_at: row.created_at,
                    evidence_refs: serde_json::from_str(&row.evidence_refs)?,
                    content: row.content,
                }),
            });
        }

        // Stable sort, descending score: ties keep observations-then-summaries
        // insertion order, matching the TS provider's stable Array.sort.
        results.sort_by(|a, b| b.score.total_cmp(&a.score));
        results.truncate(max_results as usize);
        Ok(results)
    }
}

/// First 100 UTF-16 code units of `content`, with `...` appended when
/// truncated. Counts code units to match JS `string.length` / `substring`;
/// rounds down at a surrogate-pair boundary (same policy as kindling-filter).
fn match_context(content: &str) -> String {
    let mut units = 0usize;
    for (byte_idx, ch) in content.char_indices() {
        let width = ch.len_utf16();
        if units + width > MATCH_CONTEXT_UNITS {
            return format!("{}...", &content[..byte_idx]);
        }
        units += width;
    }
    content.to_string()
}

/// `AND <prefix>.<column> = ?` clauses for each scope dimension that is set.
/// `task_id` has no denormalized column and is intentionally not filterable,
/// matching the TS provider.
fn build_scope_filters(scope: &ScopeIds, prefix: &str) -> (Vec<String>, Vec<SqlValue>) {
    let mut clauses = Vec::new();
    let mut params = Vec::new();
    let filters = [
        ("session_id", &scope.session_id),
        ("repo_id", &scope.repo_id),
        ("agent_id", &scope.agent_id),
        ("user_id", &scope.user_id),
    ];
    for (column, value) in filters {
        if let Some(value) = value {
            clauses.push(format!("{prefix}.{column} = ?"));
            params.push(SqlValue::Text(value.clone()));
        }
    }
    (clauses, params)
}

/// `AND <prefix>.id NOT IN (?, …)` for excluded IDs, or empty.
fn exclude_id_filter(prefix: &str, exclude_ids: &[Id]) -> String {
    if exclude_ids.is_empty() {
        return String::new();
    }
    let placeholders = vec!["?"; exclude_ids.len()].join(",");
    format!("AND {prefix}.id NOT IN ({placeholders})")
}

/// True for FTS5 query-syntax errors, which are swallowed into empty results
/// (malformed user queries are not provider failures). Mirrors
/// `isFtsSyntaxError` in the TS provider.
fn is_fts_syntax_error(err: &rusqlite::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("fts5")
        || msg.contains("fts syntax")
        || msg.contains("unterminated string")
        || msg.contains("unknown special query")
}

fn parse_observation_kind(value: &str) -> ProviderResult<ObservationKind> {
    serde_json::from_value(serde_json::Value::String(value.to_string())).map_err(|_| {
        ProviderError::UnexpectedRowValue {
            column: "kind",
            value: value.to_string(),
        }
    })
}
