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
//! where `fts_relevance` is the BM25 rank normalized to `[0, 1]` across the
//! full filtered match set (both entity types, after scope, redaction and
//! `exclude_ids` filtering) and `recency_score = MAX(0, 1.0 - age_ms /
//! max_age_ms)`.
//!
//! Ranking is exact: every filtered match is scored before `max_results` is
//! applied, so a result's score does not depend on the limit and a limited
//! result list is always a prefix of the unlimited one. Equal scores are
//! ordered observations first, then by BM25 rank (best first), then by `id`.
//! Only the returned rows are read in full; the candidate scan and that read
//! share one read snapshot.
//!
//! The one deliberate API deviation from TypeScript: the TS provider reads
//! `Date.now()` internally, while [`RetrievalProvider::search`] takes `now`
//! explicitly so retrieval is deterministic and testable.

use std::cmp::Ordering;

use rusqlite::types::Value as SqlValue;
use rusqlite::{params_from_iter, Connection, OptionalExtension};

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

/// Entity type of a candidate. The derived order is the tie-break order:
/// observations before summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EntityType {
    Observation,
    Summary,
}

/// A filtered FTS match before hydration: just enough to score and order it.
#[derive(Debug)]
struct Candidate {
    entity_type: EntityType,
    rowid: i64,
    id: String,
    fts_rank: f64,
    recency: f64,
    score: f64,
}

/// Total result order: score descending, then observations before summaries,
/// then BM25 rank ascending (FTS5 rank is negative; lower is more relevant),
/// then `id`. `id` is the primary key, so no two candidates compare equal.
/// BM25 ranks are compared only within one entity type, because the two FTS
/// indexes have different corpus statistics.
fn result_order(a: &Candidate, b: &Candidate) -> Ordering {
    b.score
        .total_cmp(&a.score)
        .then(a.entity_type.cmp(&b.entity_type))
        .then(a.fts_rank.total_cmp(&b.fts_rank))
        .then_with(|| a.id.cmp(&b.id))
}

/// FTS5 + recency retrieval over a kindling SQLite database.
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

    /// Every filtered match of one entity type, unscored. FTS syntax errors
    /// yield no candidates (malformed user queries are not provider failures).
    fn candidates(
        &self,
        entity_type: EntityType,
        options: &ProviderSearchOptions,
        now: Timestamp,
    ) -> ProviderResult<Vec<Candidate>> {
        let (sql, params) = candidate_query(entity_type, options, now);
        let result = (|| -> Result<Vec<Candidate>, rusqlite::Error> {
            let mut stmt = self.conn.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(params), |row| {
                Ok(Candidate {
                    entity_type,
                    rowid: row.get(0)?,
                    id: row.get(1)?,
                    fts_rank: row.get(2)?,
                    recency: row.get(3)?,
                    score: 0.0,
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

    /// Read a ranked candidate's full row. Runs in the same snapshot as the
    /// candidate scan, so the row must exist and carry the same `id`; anything
    /// else means the database changed underneath us and is an error.
    fn hydrate(&self, candidate: Candidate) -> ProviderResult<ProviderSearchResult> {
        let entity = match candidate.entity_type {
            EntityType::Observation => {
                let row = self
                    .conn
                    .prepare_cached(
                        "SELECT o.id, o.kind, o.content, o.provenance, o.ts, o.scope_ids, o.redacted
                         FROM observations o WHERE o.rowid = ?1",
                    )?
                    .query_row([candidate.rowid], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, Timestamp>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, i64>(6)?,
                        ))
                    })
                    .optional()?;
                let (id, kind, content, provenance, ts, scope_ids, redacted) =
                    row.ok_or_else(|| missing_row(&candidate))?;
                check_id(&candidate, &id)?;
                RetrievedEntity::Observation(Observation {
                    id,
                    kind: parse_observation_kind(&kind)?,
                    provenance: serde_json::from_str(&provenance)?,
                    ts,
                    scope_ids: serde_json::from_str(&scope_ids)?,
                    redacted: redacted == 1,
                    content,
                })
            }
            EntityType::Summary => {
                let row = self
                    .conn
                    .prepare_cached(
                        "SELECT s.id, s.capsule_id, s.content, s.confidence, s.evidence_refs, s.created_at
                         FROM summaries s WHERE s.rowid = ?1",
                    )?
                    .query_row([candidate.rowid], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, f64>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, Timestamp>(5)?,
                        ))
                    })
                    .optional()?;
                let (id, capsule_id, content, confidence, evidence_refs, created_at) =
                    row.ok_or_else(|| missing_row(&candidate))?;
                check_id(&candidate, &id)?;
                RetrievedEntity::Summary(Summary {
                    id,
                    capsule_id,
                    confidence,
                    created_at,
                    evidence_refs: serde_json::from_str(&evidence_refs)?,
                    content,
                })
            }
        };

        let content = match &entity {
            RetrievedEntity::Observation(obs) => &obs.content,
            RetrievedEntity::Summary(sum) => &sum.content,
        };
        Ok(ProviderSearchResult {
            score: candidate.score,
            match_context: Some(match_context(content)),
            entity,
        })
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
        let max_results = options.max_results.unwrap_or(DEFAULT_MAX_RESULTS) as usize;
        if max_results == 0 {
            return Ok(Vec::new());
        }

        // One read snapshot for the candidate scan and hydration, so a row
        // cannot be redacted, removed or renumbered between the two. A caller
        // already inside a transaction on this connection has a consistent
        // view; nesting is not possible, and not needed. Dropping the
        // transaction ends it (rollback is a no-op for reads).
        let _snapshot = if self.conn.is_autocommit() {
            Some(self.conn.unchecked_transaction()?)
        } else {
            None
        };

        let mut candidates = self.candidates(EntityType::Observation, options, now)?;
        candidates.extend(self.candidates(EntityType::Summary, options, now)?);

        // Normalize BM25 ranks across the full filtered match set of BOTH
        // entity types, so scores are comparable and independent of
        // `max_results`. FTS5 rank is negative; more negative = more relevant.
        let mut min_rank = f64::INFINITY;
        let mut max_rank = f64::NEG_INFINITY;
        for candidate in &candidates {
            min_rank = min_rank.min(candidate.fts_rank);
            max_rank = max_rank.max(candidate.fts_rank);
        }
        let rank_range = if min_rank <= max_rank {
            max_rank - min_rank
        } else {
            0.0 // no candidates at all; loop below never runs
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
            let score = (score * 1e10).round() / 1e10;
            // A non-finite BM25 rank (only possible with a reconfigured FTS5
            // `rank`) must not sort ahead of everything as NaN does.
            if score.is_finite() {
                score
            } else {
                0.0
            }
        };
        for candidate in &mut candidates {
            candidate.score = score_of(candidate.fts_rank, candidate.recency);
        }

        // Top `max_results` by the total order, without sorting the rest.
        if candidates.len() > max_results {
            candidates.select_nth_unstable_by(max_results - 1, result_order);
            candidates.truncate(max_results);
        }
        candidates.sort_by(result_order);

        candidates
            .into_iter()
            .map(|candidate| self.hydrate(candidate))
            .collect()
    }
}

/// SQL + parameters listing every filtered FTS match of one entity type as
/// `(rowid, id, fts_rank, recency)`, in no particular order.
///
/// Rust orders the results, so the query has no `ORDER BY`: sorting by rank
/// in SQL only added cost, and leaving the join order to the planner lets a
/// selective scope drive from its index. Correctness never depends on the
/// plan (see the `ranking_does_not_depend_on_the_query_plan` test).
///
/// The scan reads `ts` (or `created_at`), `redacted` and the scope columns from
/// the base row for every match, not just the rowid, because ranking and
/// filtering need them before the limit is applied. Those columns sit after
/// `content` in the row, so for large content they are behind its overflow
/// pages: that read is the dominant cost of this query. Deferring them to
/// [`LocalFtsProvider::hydrate`] would break exact ranking, since hydration
/// sees only the rows already chosen.
fn candidate_query(
    entity_type: EntityType,
    options: &ProviderSearchOptions,
    now: Timestamp,
) -> (String, Vec<SqlValue>) {
    let exclude_ids = options.exclude_ids.as_deref().unwrap_or(&[]);
    let include_redacted = options.include_redacted.unwrap_or(false);

    let scope_prefix = match entity_type {
        EntityType::Observation => "o",
        EntityType::Summary => "c", // summaries are scoped through their capsule
    };
    let (scope_clauses, scope_params) = build_scope_filters(&options.scope_ids, scope_prefix);
    let scope_filter = and_clauses(&scope_clauses);

    let sql = match entity_type {
        EntityType::Observation => {
            let redacted_filter = if include_redacted {
                ""
            } else {
                "AND o.redacted = 0"
            };
            format!(
                "WITH fts_hits AS (
                   SELECT rowid, rank FROM observations_fts WHERE content MATCH ?
                 )
                 SELECT
                   o.rowid, o.id,
                   f.rank AS fts_rank,
                   MAX(0.0, 1.0 - CAST(? - o.ts AS REAL) / ?) AS recency
                 FROM fts_hits f
                 JOIN observations o ON f.rowid = o.rowid
                 WHERE 1=1
                   {redacted_filter}
                   {scope_filter}
                   {exclude_filter}",
                exclude_filter = exclude_id_filter("o", exclude_ids),
            )
        }
        EntityType::Summary => {
            format!(
                "WITH fts_hits AS (
                   SELECT rowid, rank FROM summaries_fts WHERE content MATCH ?
                 )
                 SELECT
                   s.rowid, s.id,
                   f.rank AS fts_rank,
                   MAX(0.0, 1.0 - CAST(? - s.created_at AS REAL) / ?) AS recency
                 FROM fts_hits f
                 JOIN summaries s ON f.rowid = s.rowid
                 JOIN capsules c ON s.capsule_id = c.id
                 WHERE 1=1
                   {scope_filter}
                   {exclude_filter}",
                exclude_filter = exclude_id_filter("s", exclude_ids),
            )
        }
    };

    let mut params: Vec<SqlValue> = vec![
        SqlValue::Text(options.query.clone()),
        SqlValue::Integer(now),
        SqlValue::Integer(MAX_AGE_MS),
    ];
    params.extend(scope_params);
    params.extend(exclude_ids.iter().map(|id| SqlValue::Text(id.clone())));
    (sql, params)
}

/// `AND a AND b …` for `clauses`, or empty.
fn and_clauses(clauses: &[String]) -> String {
    if clauses.is_empty() {
        String::new()
    } else {
        format!("AND {}", clauses.join(" AND "))
    }
}

fn missing_row(candidate: &Candidate) -> ProviderError {
    ProviderError::UnexpectedRowValue {
        column: "rowid",
        value: format!("{} (row for {} vanished)", candidate.rowid, candidate.id),
    }
}

fn check_id(candidate: &Candidate, hydrated_id: &str) -> ProviderResult<()> {
    if hydrated_id == candidate.id {
        Ok(())
    } else {
        Err(ProviderError::UnexpectedRowValue {
            column: "id",
            value: format!(
                "{hydrated_id} (expected {} at rowid {})",
                candidate.id, candidate.rowid
            ),
        })
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
///
/// FTS5 column filters such as `missing:term` surface as SQLite
/// `no such column: missing` with a bare identifier. Provider SQL always
/// qualifies columns (`o.redacted`, `f.rank`, …), so those schema errors
/// still propagate. The FTS `content` column is also treated as schema: it
/// is the only indexed column, so `no such column: content` means the index
/// itself is broken, not that the user typed a bad filter.
fn is_fts_syntax_error(err: &rusqlite::Error) -> bool {
    is_fts_syntax_error_message(&err.to_string())
}

fn is_fts_syntax_error_message(message: &str) -> bool {
    let msg = message.to_lowercase();
    msg.contains("fts5")
        || msg.contains("fts syntax")
        || msg.contains("unterminated string")
        || msg.contains("unknown special query")
        || is_fts_unknown_column_error(&msg)
}

/// FTS5 reports an unknown column filter as `no such column: <name>`.
/// Qualified names and the FTS `content` column are backend/schema failures.
fn is_fts_unknown_column_error(msg: &str) -> bool {
    const PREFIX: &str = "no such column: ";
    let Some(idx) = msg.find(PREFIX) else {
        return false;
    };
    let column = msg[idx + PREFIX.len()..]
        .split_whitespace()
        .next()
        .unwrap_or("");
    !column.is_empty() && !is_schema_column(column)
}

fn is_schema_column(column: &str) -> bool {
    column == "content"
        || matches!(
            column.split_once('.').map(|(alias, _)| alias),
            Some("o" | "s" | "c" | "f")
        )
}

fn parse_observation_kind(value: &str) -> ProviderResult<ObservationKind> {
    serde_json::from_value(serde_json::Value::String(value.to_string())).map_err(|_| {
        ProviderError::UnexpectedRowValue {
            column: "kind",
            value: value.to_string(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::is_fts_syntax_error_message;

    #[test]
    fn unknown_fts_column_is_syntax_error() {
        for msg in [
            "no such column: missing",
            "no such column: col1",
            "NO SUCH COLUMN: missing",
            "no such column: foo.bar",
        ] {
            assert!(
                is_fts_syntax_error_message(msg),
                "{msg:?} should be treated as a malformed FTS query"
            );
        }
    }

    #[test]
    fn schema_and_backend_errors_are_not_syntax_errors() {
        for msg in [
            "no such column: o.redacted",
            "no such column: f.rank",
            "no such column: s.content",
            "no such column: c.session_id",
            "no such column: content",
            "no such table: observations_fts",
            "disk i/o error",
        ] {
            assert!(
                !is_fts_syntax_error_message(msg),
                "{msg:?} should propagate as a backend error"
            );
        }
    }

    #[test]
    fn existing_fts_syntax_signatures_still_match() {
        for msg in [
            "fts5: syntax error near \"AND\"",
            "unterminated string",
            "unknown special query: ",
        ] {
            assert!(
                is_fts_syntax_error_message(msg),
                "{msg:?} should remain a malformed FTS query"
            );
        }
    }
}
