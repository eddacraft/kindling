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
//! The query is interpreted per [`QueryMode`]. Literal mode (the default)
//! compiles plain text into an FTS5 query (see [`compile_literal_query`]);
//! advanced mode passes FTS5 syntax through. An FTS5 syntax error is
//! reported as [`ProviderError::InvalidQuery`], never as an empty result.
//!
//! The one deliberate API deviation from TypeScript: the TS provider reads
//! `Date.now()` internally, while [`RetrievalProvider::search`] takes `now`
//! explicitly so retrieval is deterministic and testable.

use std::cmp::Ordering;

use rusqlite::types::Value as SqlValue;
use rusqlite::{params_from_iter, Connection, OptionalExtension};

use kindling_store::SqliteKindlingStore;
use kindling_types::{
    Id, Observation, ObservationKind, ProviderSearchOptions, ProviderSearchResult, QueryMode,
    RetrievedEntity, ScopeIds, Summary, Timestamp,
};

use crate::error::{ProviderError, ProviderResult};

/// Max age in ms for recency scoring (30 days).
const MAX_AGE_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Default result cap when `max_results` is not given.
const DEFAULT_MAX_RESULTS: u32 = 50;

/// Match-context preview length in UTF-16 code units (JS `string.length`).
const MATCH_CONTEXT_UNITS: usize = 100;

/// Most session rows read to find a session's rowid span: about 1 ms of
/// index reads. A larger session searches without a bound, exactly as before.
const SESSION_SPAN_SCAN_LIMIT: i64 = 10_000;

/// Where a session's observations sit in rowid order.
#[derive(Debug, PartialEq, Eq)]
enum SessionSpan {
    /// The session has no observations.
    Empty,
    /// Every observation in the session has a rowid in `first..=last`.
    Bounded(i64, i64),
    /// The session has more rows than the scan limit: search without a bound.
    Unbounded,
}

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
    session_span_scan_limit: i64,
}

impl<'conn> LocalFtsProvider<'conn> {
    /// Provider name, as recorded in retrieval provenance.
    pub const NAME: &'static str = "local-fts";

    pub fn new(conn: &'conn Connection) -> Self {
        Self {
            conn,
            session_span_scan_limit: SESSION_SPAN_SCAN_LIMIT,
        }
    }

    /// Borrow the connection of an open store.
    pub fn from_store(store: &'conn SqliteKindlingStore) -> Self {
        Self::new(store.connection())
    }

    /// Every filtered match of one entity type for the FTS5 expression
    /// `fts_query`, unscored. An FTS5 syntax error is
    /// [`ProviderError::InvalidQuery`].
    fn candidates(
        &self,
        entity_type: EntityType,
        fts_query: &str,
        options: &ProviderSearchOptions,
        now: Timestamp,
    ) -> ProviderResult<Vec<Candidate>> {
        let result = (|| -> Result<Vec<Candidate>, rusqlite::Error> {
            let observations = matches!(entity_type, EntityType::Observation);
            // A session-scoped observation search reads only the session's
            // rowid span of the FTS index. Every row carrying the session id
            // lies inside [first, last] by definition, and BM25 uses
            // corpus-wide statistics, so candidates and ranks are unchanged.
            // FTS5 applies a rowid range natively, so matches outside the span
            // are never read or scored. Repo, agent and user scopes interleave
            // across the whole store, so a span would not narrow them.
            let rowid_span = match options.scope_ids.session_id.as_deref() {
                Some(session_id) if observations => match self.session_rowid_span(session_id)? {
                    SessionSpan::Empty => return Ok(Vec::new()),
                    SessionSpan::Bounded(first, last) => Some((first, last)),
                    SessionSpan::Unbounded => None,
                },
                _ => None,
            };
            let narrow = observations && self.has_observation_meta()?;
            let (sql, params) =
                candidate_query(entity_type, fts_query, options, now, narrow, rowid_span);
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
            Err(err) if is_fts_syntax_error(&err) => {
                Err(ProviderError::InvalidQuery(err.to_string()))
            }
            Err(err) => Err(err.into()),
        }
    }

    /// The rowid span of `session_id`'s observations, reading at most
    /// `session_span_scan_limit` of them from `idx_obs_session_ts`. That index
    /// does not order a session's rows by rowid, so the lookup reads one index
    /// entry per row; past the limit the session is
    /// [`SessionSpan::Unbounded`]. It reads `observations`, not
    /// `observation_meta`, because only `observations` has a session index;
    /// the triggers keep the two identical. [`RetrievalProvider::search`] runs
    /// it in the same read transaction as the candidate scan, so a concurrent
    /// write cannot land outside a stale span.
    fn session_rowid_span(&self, session_id: &str) -> Result<SessionSpan, rusqlite::Error> {
        let limit = self.session_span_scan_limit;
        let (first, last, count): (Option<i64>, Option<i64>, i64) = self.conn.query_row(
            "SELECT MIN(rowid), MAX(rowid), COUNT(*) FROM (
               SELECT rowid FROM observations WHERE session_id = ? LIMIT ?
             )",
            rusqlite::params![session_id, limit.saturating_add(1)],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok(match (first, last) {
            _ if count > limit => SessionSpan::Unbounded,
            (Some(first), Some(last)) => SessionSpan::Bounded(first, last),
            _ => SessionSpan::Empty,
        })
    }

    /// Whether the database has `observation_meta` (schema revision 8). A
    /// revision-7 database opened without migrating, for example read-only,
    /// does not.
    fn has_observation_meta(&self) -> Result<bool, rusqlite::Error> {
        self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'observation_meta')",
            [],
            |row| row.get(0),
        )
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
                    scope_ids: decode_stored_scope_ids(&scope_ids)?,
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
        let Some(fts_query) = fts_query(options) else {
            return Ok(Vec::new());
        };

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

        let mut candidates = self.candidates(EntityType::Observation, &fts_query, options, now)?;
        candidates.extend(self.candidates(EntityType::Summary, &fts_query, options, now)?);

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
///
/// Observation candidates join `observation_meta`, a narrow copy of the
/// columns used here, when it exists. The wide `observations` row keeps them
/// behind `content` and its overflow pages, and reading it was the largest
/// cost of a broad search. Without the table (a revision-7 database opened
/// without migrating) the same query runs against `observations`.
///
/// `rowid_span`, when set, bounds the observation FTS scan to
/// `first..=last` (see [`LocalFtsProvider::session_rowid_span`]). It must
/// contain every rowid the scope filter can accept.
fn candidate_query(
    entity_type: EntityType,
    fts_query: &str,
    options: &ProviderSearchOptions,
    now: Timestamp,
    has_observation_meta: bool,
    rowid_span: Option<(i64, i64)>,
) -> (String, Vec<SqlValue>) {
    let exclude_ids = options.exclude_ids.as_deref().unwrap_or(&[]);
    let include_redacted = options.include_redacted.unwrap_or(false);

    let scope_prefix = match entity_type {
        EntityType::Observation => "m",
        EntityType::Summary => "c", // summaries are scoped through their capsule
    };
    let (scope_clauses, scope_params) = build_scope_filters(&options.scope_ids, scope_prefix);
    let scope_filter = and_clauses(&scope_clauses);

    let sql = match entity_type {
        EntityType::Observation => {
            let redacted_filter = if include_redacted {
                ""
            } else {
                "AND m.redacted = 0"
            };
            format!(
                "WITH fts_hits AS (
                   SELECT rowid, rank FROM observations_fts WHERE content MATCH ?
                     {span_filter}
                 )
                 SELECT
                   m.rowid, m.id,
                   f.rank AS fts_rank,
                   MAX(0.0, 1.0 - CAST(? - m.ts AS REAL) / ?) AS recency
                 FROM fts_hits f
                 JOIN {meta_table} m ON f.rowid = m.rowid
                 WHERE 1=1
                   {redacted_filter}
                   {scope_filter}
                   {exclude_filter}",
                meta_table = if has_observation_meta {
                    "observation_meta"
                } else {
                    "observations"
                },
                exclude_filter = exclude_id_filter("m", exclude_ids),
                span_filter = if rowid_span.is_some() {
                    "AND rowid BETWEEN ? AND ?"
                } else {
                    ""
                },
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

    let mut params: Vec<SqlValue> = vec![SqlValue::Text(fts_query.to_string())];
    if let (EntityType::Observation, Some((first, last))) = (entity_type, rowid_span) {
        params.push(SqlValue::Integer(first));
        params.push(SqlValue::Integer(last));
    }
    params.push(SqlValue::Integer(now));
    params.push(SqlValue::Integer(MAX_AGE_MS));
    params.extend(scope_params);
    params.extend(exclude_ids.iter().map(|id| SqlValue::Text(id.clone())));
    (sql, params)
}

/// The FTS5 `MATCH` expression for `options`, or `None` when the query has
/// no terms (empty or whitespace only), which matches nothing.
fn fts_query(options: &ProviderSearchOptions) -> Option<String> {
    match options.query_mode.unwrap_or_default() {
        QueryMode::Literal => compile_literal_query(&options.query),
        QueryMode::Advanced => (!options.query.trim().is_empty()).then(|| options.query.clone()),
    }
}

/// Compile plain text into an FTS5 query that is valid for every input.
///
/// The text is split on whitespace. Each term becomes an FTS5 string: any `"`
/// inside it is doubled and the term is wrapped in `"…"`. FTS5 tokenizes a
/// string into a phrase, so `src/cache.rs` matches as the token sequence
/// `src cache rs` and its punctuation is never read as syntax. Terms are
/// joined with spaces, FTS5's implicit AND, so every term must match.
/// Operators and wildcards (`NOT`, `OR`, `*`) are searched for as text.
///
/// A term with no indexable characters (such as `++`) is an empty phrase,
/// which matches nothing on its own and does not constrain other terms.
fn compile_literal_query(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
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

/// True for FTS5 query-syntax errors, which become
/// [`ProviderError::InvalidQuery`]. Every other SQLite error is a backend or
/// schema failure and propagates unchanged. The signatures mirror
/// `isFtsSyntaxError` in the retired TS provider.
///
/// FTS5 column filters such as `missing:term` surface as SQLite
/// `no such column: missing` with a bare identifier. Provider SQL always
/// qualifies columns (`m.redacted`, `o.redacted`, `f.rank`, …), so those schema errors
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
            Some("o" | "m" | "s" | "c" | "f")
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

/// Decode a row's stored `scope_ids` JSON blob.
///
/// The retired TS store wrote `JSON.stringify(scopeIds)` and its validator
/// allowed an array, so a TS-era row can hold `'[]'` or `'["x"]'`. TS read
/// `.sessionId` from that array as `undefined` and wrote NULL denormalized
/// columns, so such a row means "no scope". `ScopeIds` rejects arrays on the
/// wire (GH-216); reading one from disk yields an empty scope instead of an
/// error. Any other malformed value is still an error. Kept in step with the
/// copy in `kindling-store`.
fn decode_stored_scope_ids(raw: &str) -> serde_json::Result<ScopeIds> {
    serde_json::from_str(raw).or_else(|err| {
        match serde_json::from_str::<Vec<serde_json::Value>>(raw) {
            Ok(_) => Ok(ScopeIds::default()),
            Err(_) => Err(err),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        compile_literal_query, is_fts_syntax_error_message, LocalFtsProvider, RetrievalProvider,
        SessionSpan,
    };
    use kindling_store::SqliteKindlingStore;
    use kindling_types::{
        Capsule, CapsuleStatus, CapsuleType, Observation, ObservationKind, ProviderSearchOptions,
        RetrievedEntity, ScopeIds, Summary,
    };

    const NOW: i64 = 1_700_000_000_000;

    fn session(session_id: Option<&str>, repo_id: Option<&str>) -> ScopeIds {
        ScopeIds {
            session_id: session_id.map(String::from),
            repo_id: repo_id.map(String::from),
            ..ScopeIds::default()
        }
    }

    fn insert(store: &SqliteKindlingStore, id: &str, content: &str, scope_ids: ScopeIds, ts: i64) {
        store
            .insert_observation(&Observation {
                id: id.to_string(),
                kind: ObservationKind::Message,
                content: content.to_string(),
                provenance: serde_json::Map::new(),
                ts,
                scope_ids,
                redacted: false,
            })
            .unwrap();
    }

    fn rowid(store: &SqliteKindlingStore, id: &str) -> i64 {
        store
            .connection()
            .query_row("SELECT rowid FROM observations WHERE id = ?", [id], |r| {
                r.get(0)
            })
            .unwrap()
    }

    fn provider(store: &SqliteKindlingStore, limit: i64) -> LocalFtsProvider<'_> {
        let mut provider = LocalFtsProvider::from_store(store);
        provider.session_span_scan_limit = limit;
        provider
    }

    #[test]
    fn session_span_is_bounded_empty_or_unbounded_past_the_scan_limit() {
        let store = SqliteKindlingStore::open_in_memory().unwrap();
        insert(&store, "a1", "cache", session(Some("a"), None), NOW);
        insert(&store, "b1", "cache", session(Some("b"), None), NOW);
        insert(&store, "a2", "cache", session(Some("a"), None), NOW);
        insert(&store, "a3", "cache", session(Some("a"), None), NOW);

        assert_eq!(
            provider(&store, 3).session_rowid_span("a").unwrap(),
            SessionSpan::Bounded(rowid(&store, "a1"), rowid(&store, "a3"))
        );
        assert_eq!(
            provider(&store, 3).session_rowid_span("missing").unwrap(),
            SessionSpan::Empty
        );
        // More rows than the limit: never under-bound, give up instead.
        assert_eq!(
            provider(&store, 2).session_rowid_span("a").unwrap(),
            SessionSpan::Unbounded
        );
    }

    /// Interleaved sessions, rows without a session, a matching row at the
    /// start of the span, a redacted row at its end (redacted rows leave the
    /// FTS index, but still set the span), and a summary in the session's
    /// capsule. Every query
    /// returns exactly what the unbounded scan (limit 0) returns.
    #[test]
    fn bounded_session_search_matches_the_unbounded_scan() {
        let store = SqliteKindlingStore::open_in_memory().unwrap();
        insert(
            &store,
            "before",
            "auth cache token",
            session(None, Some("/r")),
            NOW,
        );
        insert(
            &store,
            "a-first",
            "auth cache",
            session(Some("a"), Some("/r")),
            NOW - 5,
        );
        for i in 0..40 {
            let (sid, repo) = match i % 3 {
                0 => (Some("a"), Some("/r")),
                1 => (Some("b"), Some("/r")),
                _ => (None, Some("/q")),
            };
            let content = format!("auth {} cache {}", "token ".repeat(i % 4), i);
            insert(
                &store,
                &format!("o{i}"),
                &content,
                session(sid, repo),
                NOW - i as i64 * 3_600_000,
            );
        }
        insert(
            &store,
            "a-last",
            "auth token",
            session(Some("a"), Some("/q")),
            NOW,
        );
        insert(
            &store,
            "after",
            "auth cache token",
            session(Some("b"), None),
            NOW,
        );
        store.redact_observation("a-last").unwrap();
        store
            .create_capsule(&Capsule {
                id: "cap-a".to_string(),
                kind: CapsuleType::Session,
                intent: "test".to_string(),
                status: CapsuleStatus::Open,
                opened_at: NOW,
                closed_at: None,
                scope_ids: session(Some("a"), None),
                observation_ids: Vec::new(),
                summary_id: None,
            })
            .unwrap();
        store
            .insert_summary(&Summary {
                id: "sum-a".to_string(),
                capsule_id: "cap-a".to_string(),
                content: "auth cache summary".to_string(),
                confidence: 0.9,
                created_at: NOW,
                evidence_refs: Vec::new(),
            })
            .unwrap();

        let span = provider(&store, 10_000).session_rowid_span("a").unwrap();
        assert_eq!(
            span,
            SessionSpan::Bounded(rowid(&store, "a-first"), rowid(&store, "a-last"))
        );

        let opts = |query: &str, scope_ids: ScopeIds| ProviderSearchOptions {
            query: query.to_string(),
            scope_ids,
            max_results: None,
            exclude_ids: None,
            include_redacted: None,
        };
        let mut queries = vec![
            opts("auth", session(Some("a"), None)),
            opts("token", session(Some("a"), None)),
            opts("auth cache", session(Some("a"), Some("/r"))),
            opts("auth", session(Some("b"), None)),
            opts("auth", session(Some("missing"), None)),
        ];
        let mut redacted = opts("auth", session(Some("a"), None));
        redacted.include_redacted = Some(true);
        let mut limited = opts("auth", session(Some("a"), None));
        limited.max_results = Some(3);
        limited.exclude_ids = Some(vec!["o3".to_string()]);
        queries.extend([redacted, limited]);

        let run = |limit: i64, q: &ProviderSearchOptions| -> Vec<(String, f64)> {
            provider(&store, limit)
                .search(q, NOW)
                .unwrap()
                .into_iter()
                .map(|r| {
                    let id = match r.entity {
                        RetrievedEntity::Observation(o) => o.id,
                        RetrievedEntity::Summary(s) => s.id,
                    };
                    (id, r.score)
                })
                .collect()
        };
        let mut non_empty = 0;
        for q in &queries {
            let bounded = run(10_000, q);
            non_empty += usize::from(!bounded.is_empty());
            assert_eq!(bounded, run(0, q), "query {:?} {:?}", q.query, q.scope_ids);
        }
        assert!(non_empty >= 5, "fixture must exercise real matches");
        // Session scope still finds the session's capsule summary.
        assert!(run(10_000, &queries[0]).iter().any(|(id, _)| id == "sum-a"));
    }

    /// The span reaches FTS5 as a rowid constraint, so matches outside it are
    /// never read. (A filter applied after the scan would be correct but no
    /// faster.)
    #[test]
    fn session_span_is_pushed_into_the_fts_scan() {
        let store = SqliteKindlingStore::open_in_memory().unwrap();
        let opts = ProviderSearchOptions {
            query: "auth".to_string(),
            scope_ids: session(Some("a"), None),
            max_results: None,
            exclude_ids: None,
            include_redacted: None,
        };
        let plan = |span: Option<(i64, i64)>| -> String {
            let (sql, params) =
                super::candidate_query(super::EntityType::Observation, &opts, NOW, true, span);
            let mut stmt = store
                .connection()
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap();
            let rows: Vec<String> = stmt
                .query_map(rusqlite::params_from_iter(params), |r| {
                    r.get::<_, String>(3)
                })
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            rows.into_iter()
                .find(|d| d.contains("observations_fts"))
                .expect("plan scans observations_fts")
        };
        let bounded = plan(Some((10, 20)));
        let unbounded = plan(None);
        assert_ne!(
            bounded, unbounded,
            "the rowid range must change the FTS5 index plan"
        );
        assert!(bounded.contains('<') && bounded.contains('>'), "{bounded}");
    }

    #[test]
    fn literal_query_quotes_each_whitespace_separated_term() {
        for (input, expected) in [
            ("cache.rs", Some(r#""cache.rs""#)),
            ("  fix:   cache\t", Some(r#""fix:" "cache""#)),
            ("what NOT cached", Some(r#""what" "NOT" "cached""#)),
            ("auth*", Some(r#""auth*""#)),
            (r#"say "hi""#, Some(r#""say" """hi""""#)),
            (r#"""#, Some(r#""""""#)),
            ("", None),
            (" \t\n ", None),
        ] {
            assert_eq!(
                compile_literal_query(input).as_deref(),
                expected,
                "input {input:?}"
            );
        }
    }

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
            "no such column: m.redacted",
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
