//! SQLite-backed Kindling store.
//!
//! Mirrors `SqliteKindlingStore` in
//! `packages/kindling-store-sqlite/src/store/sqlite.ts` method-for-method.
//! FTS index sync happens automatically via the triggers defined in the
//! schema contract; no method here touches the FTS tables directly.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::types::Value as SqlValue;
use rusqlite::{named_params, params_from_iter, Connection, OptionalExtension};

use kindling_types::{
    Capsule, CapsuleStatus, CapsuleType, Observation, ObservationKind, Pin, PinTargetType,
    ScopeIds, Summary, Timestamp,
};

use crate::db::{open_database, open_in_memory, StoreOptions};
use crate::error::{StoreError, StoreResult};

/// Evidence snippet with context. Mirrors `EvidenceSnippet` in the TS store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSnippet {
    pub observation_id: String,
    pub snippet: String,
    pub kind: ObservationKind,
}

/// Default maximum snippet length used by [`SqliteKindlingStore::get_evidence_snippets`]
/// callers that have no opinion (matches the TS default parameter).
pub const DEFAULT_SNIPPET_MAX_CHARS: usize = 200;

/// SQLite-based Kindling store.
pub struct SqliteKindlingStore {
    conn: Connection,
}

impl SqliteKindlingStore {
    /// Open (and initialize if fresh) the database at `path`.
    pub fn open(path: &Path) -> StoreResult<Self> {
        Self::open_with_options(path, &StoreOptions::default())
    }

    /// Open the database at `path` with explicit options.
    pub fn open_with_options(path: &Path, options: &StoreOptions) -> StoreResult<Self> {
        Ok(Self {
            conn: open_database(path, options)?,
        })
    }

    /// Open a fresh in-memory store (test/scratch use).
    pub fn open_in_memory() -> StoreResult<Self> {
        Ok(Self {
            conn: open_in_memory()?,
        })
    }

    /// Wrap an already-configured connection (advanced use).
    pub fn from_connection(conn: Connection) -> Self {
        Self { conn }
    }

    /// Borrow the underlying connection (e.g. for the retrieval provider).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    // ===== WRITE PATH =====

    /// Insert an observation. FTS sync happens automatically via triggers.
    pub fn insert_observation(&self, observation: &Observation) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO observations (
               id, kind, content, provenance, ts, scope_ids, redacted,
               session_id, repo_id, agent_id, user_id
             ) VALUES (
               :id, :kind, :content, :provenance, :ts, :scope_ids, :redacted,
               :session_id, :repo_id, :agent_id, :user_id
             )",
            named_params! {
                ":id": observation.id,
                ":kind": observation_kind_to_str(observation.kind),
                ":content": observation.content,
                ":provenance": serde_json::to_string(&observation.provenance)?,
                ":ts": observation.ts,
                ":scope_ids": serde_json::to_string(&observation.scope_ids)?,
                ":redacted": observation.redacted,
                ":session_id": observation.scope_ids.session_id,
                ":repo_id": observation.scope_ids.repo_id,
                ":agent_id": observation.scope_ids.agent_id,
                ":user_id": observation.scope_ids.user_id,
            },
        )?;
        Ok(())
    }

    /// Create a new capsule.
    pub fn create_capsule(&self, capsule: &Capsule) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO capsules (
               id, type, intent, status, opened_at, closed_at, scope_ids,
               session_id, repo_id, agent_id, user_id
             ) VALUES (
               :id, :type, :intent, :status, :opened_at, :closed_at, :scope_ids,
               :session_id, :repo_id, :agent_id, :user_id
             )",
            named_params! {
                ":id": capsule.id,
                ":type": capsule_type_to_str(capsule.kind),
                ":intent": capsule.intent,
                ":status": capsule_status_to_str(capsule.status),
                ":opened_at": capsule.opened_at,
                ":closed_at": capsule.closed_at,
                ":scope_ids": serde_json::to_string(&capsule.scope_ids)?,
                ":session_id": capsule.scope_ids.session_id,
                ":repo_id": capsule.scope_ids.repo_id,
                ":agent_id": capsule.scope_ids.agent_id,
                ":user_id": capsule.scope_ids.user_id,
            },
        )?;
        Ok(())
    }

    /// Close a capsule: set status to `closed` and stamp `closed_at`
    /// (defaults to now). Errors if the capsule is missing or already closed.
    /// When `summary_id` is given, validates that the summary exists for this
    /// capsule.
    pub fn close_capsule(
        &self,
        capsule_id: &str,
        closed_at: Option<Timestamp>,
        summary_id: Option<&str>,
    ) -> StoreResult<()> {
        let changes = self.conn.execute(
            "UPDATE capsules
             SET status = 'closed', closed_at = :closed_at
             WHERE id = :id AND status = 'open'",
            named_params! {
                ":id": capsule_id,
                ":closed_at": closed_at.unwrap_or_else(now_ms),
            },
        )?;
        if changes == 0 {
            return Err(StoreError::CapsuleNotOpen(capsule_id.to_string()));
        }

        if let Some(summary_id) = summary_id {
            let exists: Option<String> = self
                .conn
                .query_row(
                    "SELECT id FROM summaries WHERE id = ?1 AND capsule_id = ?2",
                    [summary_id, capsule_id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                return Err(StoreError::SummaryNotFound {
                    summary_id: summary_id.to_string(),
                    capsule_id: capsule_id.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Attach an observation to a capsule, appending at the next `seq` to
    /// keep deterministic ordering.
    pub fn attach_observation_to_capsule(
        &self,
        capsule_id: &str,
        observation_id: &str,
    ) -> StoreResult<()> {
        let next_seq: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM capsule_observations WHERE capsule_id = ?1",
            [capsule_id],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO capsule_observations (capsule_id, observation_id, seq)
             VALUES (?1, ?2, ?3)",
            (capsule_id, observation_id, next_seq),
        )?;
        Ok(())
    }

    /// Insert a summary. FTS sync happens automatically via triggers.
    pub fn insert_summary(&self, summary: &Summary) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO summaries (
               id, capsule_id, content, confidence, created_at, evidence_refs
             ) VALUES (:id, :capsule_id, :content, :confidence, :created_at, :evidence_refs)",
            named_params! {
                ":id": summary.id,
                ":capsule_id": summary.capsule_id,
                ":content": summary.content,
                ":confidence": summary.confidence,
                ":created_at": summary.created_at,
                ":evidence_refs": serde_json::to_string(&summary.evidence_refs)?,
            },
        )?;
        Ok(())
    }

    /// Insert a pin.
    pub fn insert_pin(&self, pin: &Pin) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO pins (
               id, target_type, target_id, reason, created_at, expires_at, scope_ids,
               session_id, repo_id, agent_id, user_id
             ) VALUES (
               :id, :target_type, :target_id, :reason, :created_at, :expires_at, :scope_ids,
               :session_id, :repo_id, :agent_id, :user_id
             )",
            named_params! {
                ":id": pin.id,
                ":target_type": pin_target_type_to_str(pin.target_type),
                ":target_id": pin.target_id,
                ":reason": pin.reason,
                ":created_at": pin.created_at,
                ":expires_at": pin.expires_at,
                ":scope_ids": serde_json::to_string(&pin.scope_ids)?,
                ":session_id": pin.scope_ids.session_id,
                ":repo_id": pin.scope_ids.repo_id,
                ":agent_id": pin.scope_ids.agent_id,
                ":user_id": pin.scope_ids.user_id,
            },
        )?;
        Ok(())
    }

    /// Delete a pin. Errors if the pin does not exist.
    pub fn delete_pin(&self, pin_id: &str) -> StoreResult<()> {
        let changes = self
            .conn
            .execute("DELETE FROM pins WHERE id = ?1", [pin_id])?;
        if changes == 0 {
            return Err(StoreError::PinNotFound(pin_id.to_string()));
        }
        Ok(())
    }

    /// Active (non-expired) pins, optionally filtered by scope, newest first.
    /// `now` defaults to the current time.
    pub fn list_active_pins(
        &self,
        scope: Option<&ScopeIds>,
        now: Option<Timestamp>,
    ) -> StoreResult<Vec<Pin>> {
        let mut sql = String::from(
            "SELECT id, target_type, target_id, reason, created_at, expires_at, scope_ids
             FROM pins
             WHERE (expires_at IS NULL OR expires_at > ?)",
        );
        let mut params: Vec<SqlValue> = vec![SqlValue::Integer(now.unwrap_or_else(now_ms))];
        if let Some(scope) = scope {
            push_scope_filters(&mut sql, &mut params, scope);
        }
        sql.push_str(" ORDER BY created_at DESC");

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params), |row| {
            Ok(RawPin {
                id: row.get(0)?,
                target_type: row.get(1)?,
                target_id: row.get(2)?,
                reason: row.get(3)?,
                created_at: row.get(4)?,
                expires_at: row.get(5)?,
                scope_ids: row.get(6)?,
            })
        })?;

        rows.map(|row| row.map_err(StoreError::from).and_then(RawPin::into_pin))
            .collect()
    }

    /// Redact an observation: replace content with `[redacted]` and set the
    /// redacted flag. The FTS update trigger drops it from the index.
    pub fn redact_observation(&self, observation_id: &str) -> StoreResult<()> {
        let changes = self.conn.execute(
            "UPDATE observations SET content = '[redacted]', redacted = 1 WHERE id = ?1",
            [observation_id],
        )?;
        if changes == 0 {
            return Err(StoreError::ObservationNotFound(observation_id.to_string()));
        }
        Ok(())
    }

    /// Run `f` inside a transaction: commits on `Ok`, rolls back on `Err`.
    pub fn transaction<T>(&self, f: impl FnOnce(&Self) -> StoreResult<T>) -> StoreResult<T> {
        let tx = self.conn.unchecked_transaction()?;
        match f(self) {
            Ok(value) => {
                tx.commit()?;
                Ok(value)
            }
            Err(err) => Err(err), // tx dropped here → rollback
        }
    }

    // ===== READ PATH =====

    /// Most recently opened capsule with status `open` for a session.
    pub fn get_open_capsule_for_session(&self, session_id: &str) -> StoreResult<Option<Capsule>> {
        let raw = self
            .conn
            .query_row(
                "SELECT id, type, intent, status, opened_at, closed_at, scope_ids
                 FROM capsules
                 WHERE status = 'open' AND session_id = ?1
                 ORDER BY opened_at DESC
                 LIMIT 1",
                [session_id],
                RawCapsule::from_row,
            )
            .optional()?;
        match raw {
            None => Ok(None),
            Some(raw) => Ok(Some(self.hydrate_capsule(raw)?)),
        }
    }

    /// Capsule by ID, with its ordered observation IDs.
    pub fn get_capsule(&self, capsule_id: &str) -> StoreResult<Option<Capsule>> {
        let raw = self
            .conn
            .query_row(
                "SELECT id, type, intent, status, opened_at, closed_at, scope_ids
                 FROM capsules
                 WHERE id = ?1",
                [capsule_id],
                RawCapsule::from_row,
            )
            .optional()?;
        match raw {
            None => Ok(None),
            Some(raw) => Ok(Some(self.hydrate_capsule(raw)?)),
        }
    }

    /// Latest summary for a capsule, if any.
    pub fn get_latest_summary_for_capsule(&self, capsule_id: &str) -> StoreResult<Option<Summary>> {
        self.conn
            .query_row(
                "SELECT id, capsule_id, content, confidence, created_at, evidence_refs
                 FROM summaries
                 WHERE capsule_id = ?1
                 ORDER BY created_at DESC
                 LIMIT 1",
                [capsule_id],
                RawSummary::from_row,
            )
            .optional()?
            .map(RawSummary::into_summary)
            .transpose()
    }

    /// Latest summary across every capsule in a scope, newest `created_at`
    /// first. Joins `summaries` to `capsules` and filters by the denormalized
    /// scope columns (the same set [`push_scope_filters`] understands).
    ///
    /// Ports the PreCompact query in
    /// `plugins/kindling-claude-code/hooks/pre-compact.js`:
    ///
    /// ```sql
    /// SELECT s.content, s.confidence FROM summaries s
    ///   JOIN capsules c ON s.capsule_id = c.id
    ///   WHERE c.repo_id = ?
    ///   ORDER BY s.created_at DESC LIMIT 1
    /// ```
    ///
    /// The Node hook filters on `repo_id` only; this method accepts a full
    /// [`ScopeIds`] and applies a filter for each dimension that is set (so a
    /// repo-only scope reproduces the Node behaviour exactly).
    pub fn latest_summary_for_scope(
        &self,
        scope: Option<&ScopeIds>,
    ) -> StoreResult<Option<Summary>> {
        let mut sql = String::from(
            "SELECT s.id, s.capsule_id, s.content, s.confidence, s.created_at, s.evidence_refs
             FROM summaries s
             JOIN capsules c ON s.capsule_id = c.id
             WHERE 1 = 1",
        );
        let mut params: Vec<SqlValue> = Vec::new();
        if let Some(scope) = scope {
            // Scope columns live on `capsules`; qualify them to avoid ambiguity.
            push_scope_filters_prefixed(&mut sql, &mut params, scope, "c.");
        }
        sql.push_str(" ORDER BY s.created_at DESC LIMIT 1");

        let mut stmt = self.conn.prepare(&sql)?;
        stmt.query_row(params_from_iter(params), RawSummary::from_row)
            .optional()?
            .map(RawSummary::into_summary)
            .transpose()
    }

    /// Summary by ID.
    pub fn get_summary_by_id(&self, summary_id: &str) -> StoreResult<Option<Summary>> {
        self.conn
            .query_row(
                "SELECT id, capsule_id, content, confidence, created_at, evidence_refs
                 FROM summaries
                 WHERE id = ?1",
                [summary_id],
                RawSummary::from_row,
            )
            .optional()?
            .map(RawSummary::into_summary)
            .transpose()
    }

    /// Observation by ID.
    pub fn get_observation_by_id(&self, observation_id: &str) -> StoreResult<Option<Observation>> {
        self.conn
            .query_row(
                "SELECT id, kind, content, provenance, ts, scope_ids, redacted
                 FROM observations
                 WHERE id = ?1",
                [observation_id],
                RawObservation::from_row,
            )
            .optional()?
            .map(RawObservation::into_observation)
            .transpose()
    }

    /// Non-redacted observations filtered by scope and time range, newest
    /// first, capped at `limit` (TS default is 100).
    pub fn query_observations(
        &self,
        scope: Option<&ScopeIds>,
        from_ts: Option<Timestamp>,
        to_ts: Option<Timestamp>,
        limit: u32,
    ) -> StoreResult<Vec<Observation>> {
        let mut sql = String::from(
            "SELECT id, kind, content, provenance, ts, scope_ids, redacted
             FROM observations
             WHERE redacted = 0",
        );
        let mut params: Vec<SqlValue> = Vec::new();
        if let Some(scope) = scope {
            push_scope_filters(&mut sql, &mut params, scope);
        }
        if let Some(from_ts) = from_ts {
            sql.push_str(" AND ts >= ?");
            params.push(SqlValue::Integer(from_ts));
        }
        if let Some(to_ts) = to_ts {
            sql.push_str(" AND ts <= ?");
            params.push(SqlValue::Integer(to_ts));
        }
        sql.push_str(" ORDER BY ts DESC LIMIT ?");
        params.push(SqlValue::Integer(i64::from(limit)));

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params), RawObservation::from_row)?;
        rows.map(|row| {
            row.map_err(StoreError::from)
                .and_then(RawObservation::into_observation)
        })
        .collect()
    }

    /// Evidence snippets for the given observation IDs, truncated to
    /// `max_chars` characters (with `...` appended when truncated). Preserves
    /// input order; silently skips IDs that do not exist.
    pub fn get_evidence_snippets(
        &self,
        observation_ids: &[String],
        max_chars: usize,
    ) -> StoreResult<Vec<EvidenceSnippet>> {
        if observation_ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = vec!["?"; observation_ids.len()].join(",");
        let sql =
            format!("SELECT id, kind, content FROM observations WHERE id IN ({placeholders})");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(observation_ids), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut by_id = std::collections::HashMap::new();
        for row in rows {
            let (id, kind, content) = row?;
            by_id.insert(id, (kind, content));
        }

        observation_ids
            .iter()
            .filter_map(|id| by_id.remove(id).map(|found| (id, found)))
            .map(|(id, (kind, content))| {
                Ok(EvidenceSnippet {
                    observation_id: id.clone(),
                    kind: observation_kind_from_str(&kind)?,
                    snippet: truncate_snippet(&content, max_chars),
                })
            })
            .collect()
    }

    // ===== export / import / stats =====
    //
    // These back the CLI `export`, `import`, and `status` verbs (PORT-012) and
    // mirror the deterministic ordering of
    // `packages/kindling-store-sqlite/src/store/export.ts` so a Rust export
    // round-trips byte-compatibly with the TS importer (and vice versa).

    /// All non-redacted observations (or all, when `include_redacted`),
    /// optionally scoped, ordered `ts ASC, id ASC`. Matches the TS
    /// `exportDatabase` observation query.
    pub fn export_observations(
        &self,
        scope: Option<&ScopeIds>,
        include_redacted: bool,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Observation>> {
        let mut sql = String::from(
            "SELECT id, kind, content, provenance, ts, scope_ids, redacted
             FROM observations
             WHERE 1 = 1",
        );
        let mut params: Vec<SqlValue> = Vec::new();
        if let Some(scope) = scope {
            push_scope_filters(&mut sql, &mut params, scope);
        }
        if !include_redacted {
            sql.push_str(" AND redacted = 0");
        }
        sql.push_str(" ORDER BY ts ASC, id ASC");
        if let Some(limit) = limit {
            sql.push_str(" LIMIT ?");
            params.push(SqlValue::Integer(i64::from(limit)));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params), RawObservation::from_row)?;
        rows.map(|row| {
            row.map_err(StoreError::from)
                .and_then(RawObservation::into_observation)
        })
        .collect()
    }

    /// All capsules (with ordered observation ids), optionally scoped, ordered
    /// `opened_at ASC, id ASC`. Matches the TS `exportDatabase` capsule query.
    pub fn export_capsules(&self, scope: Option<&ScopeIds>) -> StoreResult<Vec<Capsule>> {
        let mut sql = String::from(
            "SELECT id, type, intent, status, opened_at, closed_at, scope_ids
             FROM capsules
             WHERE 1 = 1",
        );
        let mut params: Vec<SqlValue> = Vec::new();
        if let Some(scope) = scope {
            push_scope_filters(&mut sql, &mut params, scope);
        }
        sql.push_str(" ORDER BY opened_at ASC, id ASC");

        let mut stmt = self.conn.prepare(&sql)?;
        let raws = stmt
            .query_map(params_from_iter(params), RawCapsule::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        raws.into_iter()
            .map(|raw| self.hydrate_capsule(raw))
            .collect()
    }

    /// All summaries whose capsule matches `scope`, ordered
    /// `created_at ASC, id ASC`. Matches the TS `exportDatabase` summary query
    /// (joins `summaries` to `capsules` and filters on the capsule scope).
    pub fn export_summaries(&self, scope: Option<&ScopeIds>) -> StoreResult<Vec<Summary>> {
        let mut sql = String::from(
            "SELECT s.id, s.capsule_id, s.content, s.confidence, s.created_at, s.evidence_refs
             FROM summaries s
             JOIN capsules c ON s.capsule_id = c.id
             WHERE 1 = 1",
        );
        let mut params: Vec<SqlValue> = Vec::new();
        if let Some(scope) = scope {
            push_scope_filters_prefixed(&mut sql, &mut params, scope, "c.");
        }
        sql.push_str(" ORDER BY s.created_at ASC, s.id ASC");

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params), RawSummary::from_row)?;
        rows.map(|row| {
            row.map_err(StoreError::from)
                .and_then(RawSummary::into_summary)
        })
        .collect()
    }

    /// All pins (including expired — export is a full snapshot), optionally
    /// scoped, ordered `created_at ASC, id ASC`. Matches the TS `exportDatabase`
    /// pin query.
    pub fn export_pins(&self, scope: Option<&ScopeIds>) -> StoreResult<Vec<Pin>> {
        let mut sql = String::from(
            "SELECT id, target_type, target_id, reason, created_at, expires_at, scope_ids
             FROM pins
             WHERE 1 = 1",
        );
        let mut params: Vec<SqlValue> = Vec::new();
        if let Some(scope) = scope {
            push_scope_filters(&mut sql, &mut params, scope);
        }
        sql.push_str(" ORDER BY created_at ASC, id ASC");

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params), |row| {
            Ok(RawPin {
                id: row.get(0)?,
                target_type: row.get(1)?,
                target_id: row.get(2)?,
                reason: row.get(3)?,
                created_at: row.get(4)?,
                expires_at: row.get(5)?,
                scope_ids: row.get(6)?,
            })
        })?;
        rows.map(|row| row.map_err(StoreError::from).and_then(RawPin::into_pin))
            .collect()
    }

    /// Insert an observation if its id is not already present; returns whether a
    /// row was written. Mirrors the TS importer's `INSERT OR IGNORE`.
    pub fn import_observation(&self, observation: &Observation) -> StoreResult<bool> {
        let changes = self.conn.execute(
            "INSERT OR IGNORE INTO observations (
               id, kind, content, provenance, ts, scope_ids, redacted,
               session_id, repo_id, agent_id, user_id
             ) VALUES (
               :id, :kind, :content, :provenance, :ts, :scope_ids, :redacted,
               :session_id, :repo_id, :agent_id, :user_id
             )",
            named_params! {
                ":id": observation.id,
                ":kind": observation_kind_to_str(observation.kind),
                ":content": observation.content,
                ":provenance": serde_json::to_string(&observation.provenance)?,
                ":ts": observation.ts,
                ":scope_ids": serde_json::to_string(&observation.scope_ids)?,
                ":redacted": observation.redacted,
                ":session_id": observation.scope_ids.session_id,
                ":repo_id": observation.scope_ids.repo_id,
                ":agent_id": observation.scope_ids.agent_id,
                ":user_id": observation.scope_ids.user_id,
            },
        )?;
        Ok(changes > 0)
    }

    /// Insert a capsule if absent (and, when written, its ordered
    /// `capsule_observations` links). Returns whether the capsule row was
    /// written. Mirrors the TS importer.
    pub fn import_capsule(&self, capsule: &Capsule) -> StoreResult<bool> {
        let changes = self.conn.execute(
            "INSERT OR IGNORE INTO capsules (
               id, type, intent, status, opened_at, closed_at, scope_ids,
               session_id, repo_id, agent_id, user_id
             ) VALUES (
               :id, :type, :intent, :status, :opened_at, :closed_at, :scope_ids,
               :session_id, :repo_id, :agent_id, :user_id
             )",
            named_params! {
                ":id": capsule.id,
                ":type": capsule_type_to_str(capsule.kind),
                ":intent": capsule.intent,
                ":status": capsule_status_to_str(capsule.status),
                ":opened_at": capsule.opened_at,
                ":closed_at": capsule.closed_at,
                ":scope_ids": serde_json::to_string(&capsule.scope_ids)?,
                ":session_id": capsule.scope_ids.session_id,
                ":repo_id": capsule.scope_ids.repo_id,
                ":agent_id": capsule.scope_ids.agent_id,
                ":user_id": capsule.scope_ids.user_id,
            },
        )?;
        if changes == 0 {
            return Ok(false);
        }
        let mut stmt = self.conn.prepare(
            "INSERT OR IGNORE INTO capsule_observations (capsule_id, observation_id, seq)
             VALUES (?1, ?2, ?3)",
        )?;
        for (seq, obs_id) in capsule.observation_ids.iter().enumerate() {
            stmt.execute((&capsule.id, obs_id, seq as i64))?;
        }
        Ok(true)
    }

    /// Insert a summary if its id is absent; returns whether a row was written.
    pub fn import_summary(&self, summary: &Summary) -> StoreResult<bool> {
        let changes = self.conn.execute(
            "INSERT OR IGNORE INTO summaries (
               id, capsule_id, content, confidence, created_at, evidence_refs
             ) VALUES (:id, :capsule_id, :content, :confidence, :created_at, :evidence_refs)",
            named_params! {
                ":id": summary.id,
                ":capsule_id": summary.capsule_id,
                ":content": summary.content,
                ":confidence": summary.confidence,
                ":created_at": summary.created_at,
                ":evidence_refs": serde_json::to_string(&summary.evidence_refs)?,
            },
        )?;
        Ok(changes > 0)
    }

    /// Insert a pin if its id is absent; returns whether a row was written.
    pub fn import_pin(&self, pin: &Pin) -> StoreResult<bool> {
        let changes = self.conn.execute(
            "INSERT OR IGNORE INTO pins (
               id, target_type, target_id, reason, created_at, expires_at, scope_ids,
               session_id, repo_id, agent_id, user_id
             ) VALUES (
               :id, :target_type, :target_id, :reason, :created_at, :expires_at, :scope_ids,
               :session_id, :repo_id, :agent_id, :user_id
             )",
            named_params! {
                ":id": pin.id,
                ":target_type": pin_target_type_to_str(pin.target_type),
                ":target_id": pin.target_id,
                ":reason": pin.reason,
                ":created_at": pin.created_at,
                ":expires_at": pin.expires_at,
                ":scope_ids": serde_json::to_string(&pin.scope_ids)?,
                ":session_id": pin.scope_ids.session_id,
                ":repo_id": pin.scope_ids.repo_id,
                ":agent_id": pin.scope_ids.agent_id,
                ":user_id": pin.scope_ids.user_id,
            },
        )?;
        Ok(changes > 0)
    }

    /// Aggregate counts + database size for the `status` verb. Mirrors the
    /// pragmas + `COUNT(*)` queries in
    /// `packages/kindling-cli/src/commands/status.ts`.
    pub fn database_stats(&self) -> StoreResult<DatabaseStats> {
        let count =
            |sql: &str| -> StoreResult<i64> { Ok(self.conn.query_row(sql, [], |row| row.get(0))?) };
        let observations = count("SELECT COUNT(*) FROM observations")?;
        let capsules = count("SELECT COUNT(*) FROM capsules")?;
        let summaries = count("SELECT COUNT(*) FROM summaries")?;
        let pins = count("SELECT COUNT(*) FROM pins")?;
        let redacted = count("SELECT COUNT(*) FROM observations WHERE redacted = 1")?;
        let open_capsules = count("SELECT COUNT(*) FROM capsules WHERE status = 'open'")?;
        let latest_ts: Option<i64> = self
            .conn
            .query_row("SELECT MAX(ts) FROM observations", [], |row| row.get(0))
            .optional()?
            .flatten();

        let page_count: i64 = self
            .conn
            .query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let page_size: i64 = self
            .conn
            .query_row("PRAGMA page_size", [], |row| row.get(0))?;

        Ok(DatabaseStats {
            observations,
            capsules,
            summaries,
            pins,
            redacted,
            open_capsules,
            latest_ts,
            size_bytes: page_count * page_size,
        })
    }

    fn hydrate_capsule(&self, raw: RawCapsule) -> StoreResult<Capsule> {
        let mut stmt = self.conn.prepare(
            "SELECT observation_id FROM capsule_observations WHERE capsule_id = ?1 ORDER BY seq",
        )?;
        let observation_ids = stmt
            .query_map([&raw.id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        raw.into_capsule(observation_ids)
    }
}

/// Aggregate database statistics returned by
/// [`SqliteKindlingStore::database_stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseStats {
    pub observations: i64,
    pub capsules: i64,
    pub summaries: i64,
    pub pins: i64,
    pub redacted: i64,
    pub open_capsules: i64,
    /// Newest observation timestamp, or `None` when there are no observations.
    pub latest_ts: Option<Timestamp>,
    /// `page_count * page_size`.
    pub size_bytes: i64,
}

fn now_ms() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_millis() as Timestamp
}

/// Truncate to `max_chars` characters, appending `...` when content was cut.
/// Counts `char`s (the TS implementation counts UTF-16 code units; the two
/// agree for everything below the astral planes).
fn truncate_snippet(content: &str, max_chars: usize) -> String {
    match content.char_indices().nth(max_chars) {
        None => content.to_string(),
        Some((byte_idx, _)) => format!("{}...", &content[..byte_idx]),
    }
}

/// Append `AND <column> = ?` filters for each scope dimension that is set.
/// `task_id` has no denormalized column and is intentionally not filterable,
/// matching the TS store.
fn push_scope_filters(sql: &mut String, params: &mut Vec<SqlValue>, scope: &ScopeIds) {
    push_scope_filters_prefixed(sql, params, scope, "");
}

/// [`push_scope_filters`] with a column prefix (e.g. `"c."`) so the same scope
/// filter can target a specific table in a join.
fn push_scope_filters_prefixed(
    sql: &mut String,
    params: &mut Vec<SqlValue>,
    scope: &ScopeIds,
    prefix: &str,
) {
    let filters = [
        ("session_id", &scope.session_id),
        ("repo_id", &scope.repo_id),
        ("agent_id", &scope.agent_id),
        ("user_id", &scope.user_id),
    ];
    for (column, value) in filters {
        if let Some(value) = value {
            sql.push_str(" AND ");
            sql.push_str(prefix);
            sql.push_str(column);
            sql.push_str(" = ?");
            params.push(SqlValue::Text(value.clone()));
        }
    }
}

// ===== enum <-> TEXT column mappings =====
// Exhaustive matches so adding a variant in kindling-types fails compilation
// here until the mapping (and the schema CHECK constraint) is updated.

fn observation_kind_to_str(kind: ObservationKind) -> &'static str {
    match kind {
        ObservationKind::ToolCall => "tool_call",
        ObservationKind::Command => "command",
        ObservationKind::FileDiff => "file_diff",
        ObservationKind::Error => "error",
        ObservationKind::Message => "message",
        ObservationKind::NodeStart => "node_start",
        ObservationKind::NodeEnd => "node_end",
        ObservationKind::NodeOutput => "node_output",
        ObservationKind::NodeError => "node_error",
    }
}

fn observation_kind_from_str(value: &str) -> StoreResult<ObservationKind> {
    Ok(match value {
        "tool_call" => ObservationKind::ToolCall,
        "command" => ObservationKind::Command,
        "file_diff" => ObservationKind::FileDiff,
        "error" => ObservationKind::Error,
        "message" => ObservationKind::Message,
        "node_start" => ObservationKind::NodeStart,
        "node_end" => ObservationKind::NodeEnd,
        "node_output" => ObservationKind::NodeOutput,
        "node_error" => ObservationKind::NodeError,
        other => {
            return Err(StoreError::UnexpectedRowValue {
                column: "observations.kind",
                value: other.to_string(),
            })
        }
    })
}

fn capsule_type_to_str(kind: CapsuleType) -> &'static str {
    match kind {
        CapsuleType::Session => "session",
        CapsuleType::PocketflowNode => "pocketflow_node",
    }
}

fn capsule_type_from_str(value: &str) -> StoreResult<CapsuleType> {
    Ok(match value {
        "session" => CapsuleType::Session,
        "pocketflow_node" => CapsuleType::PocketflowNode,
        other => {
            return Err(StoreError::UnexpectedRowValue {
                column: "capsules.type",
                value: other.to_string(),
            })
        }
    })
}

fn capsule_status_to_str(status: CapsuleStatus) -> &'static str {
    match status {
        CapsuleStatus::Open => "open",
        CapsuleStatus::Closed => "closed",
    }
}

fn capsule_status_from_str(value: &str) -> StoreResult<CapsuleStatus> {
    Ok(match value {
        "open" => CapsuleStatus::Open,
        "closed" => CapsuleStatus::Closed,
        other => {
            return Err(StoreError::UnexpectedRowValue {
                column: "capsules.status",
                value: other.to_string(),
            })
        }
    })
}

fn pin_target_type_to_str(target: PinTargetType) -> &'static str {
    match target {
        PinTargetType::Observation => "observation",
        PinTargetType::Summary => "summary",
    }
}

fn pin_target_type_from_str(value: &str) -> StoreResult<PinTargetType> {
    Ok(match value {
        "observation" => PinTargetType::Observation,
        "summary" => PinTargetType::Summary,
        other => {
            return Err(StoreError::UnexpectedRowValue {
                column: "pins.target_type",
                value: other.to_string(),
            })
        }
    })
}

// ===== raw row intermediates =====
// Row closures may only return rusqlite errors, so rows are read into
// SQL-native shapes first and converted (JSON parsing, enum mapping) outside.

struct RawObservation {
    id: String,
    kind: String,
    content: String,
    provenance: String,
    ts: i64,
    scope_ids: String,
    redacted: i64,
}

impl RawObservation {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            kind: row.get(1)?,
            content: row.get(2)?,
            provenance: row.get(3)?,
            ts: row.get(4)?,
            scope_ids: row.get(5)?,
            redacted: row.get(6)?,
        })
    }

    fn into_observation(self) -> StoreResult<Observation> {
        Ok(Observation {
            id: self.id,
            kind: observation_kind_from_str(&self.kind)?,
            content: self.content,
            provenance: serde_json::from_str(&self.provenance)?,
            ts: self.ts,
            scope_ids: serde_json::from_str(&self.scope_ids)?,
            redacted: self.redacted == 1,
        })
    }
}

struct RawCapsule {
    id: String,
    kind: String,
    intent: String,
    status: String,
    opened_at: i64,
    closed_at: Option<i64>,
    scope_ids: String,
}

impl RawCapsule {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            kind: row.get(1)?,
            intent: row.get(2)?,
            status: row.get(3)?,
            opened_at: row.get(4)?,
            closed_at: row.get(5)?,
            scope_ids: row.get(6)?,
        })
    }

    fn into_capsule(self, observation_ids: Vec<String>) -> StoreResult<Capsule> {
        Ok(Capsule {
            id: self.id,
            kind: capsule_type_from_str(&self.kind)?,
            intent: self.intent,
            status: capsule_status_from_str(&self.status)?,
            opened_at: self.opened_at,
            closed_at: self.closed_at,
            scope_ids: serde_json::from_str(&self.scope_ids)?,
            observation_ids,
            // The capsules table has no summary_id column; the relationship
            // lives in summaries.capsule_id (matches the TS store).
            summary_id: None,
        })
    }
}

struct RawSummary {
    id: String,
    capsule_id: String,
    content: String,
    confidence: f64,
    created_at: i64,
    evidence_refs: String,
}

impl RawSummary {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            capsule_id: row.get(1)?,
            content: row.get(2)?,
            confidence: row.get(3)?,
            created_at: row.get(4)?,
            evidence_refs: row.get(5)?,
        })
    }

    fn into_summary(self) -> StoreResult<Summary> {
        Ok(Summary {
            id: self.id,
            capsule_id: self.capsule_id,
            content: self.content,
            confidence: self.confidence,
            created_at: self.created_at,
            evidence_refs: serde_json::from_str(&self.evidence_refs)?,
        })
    }
}

struct RawPin {
    id: String,
    target_type: String,
    target_id: String,
    reason: Option<String>,
    created_at: i64,
    expires_at: Option<i64>,
    scope_ids: String,
}

impl RawPin {
    fn into_pin(self) -> StoreResult<Pin> {
        Ok(Pin {
            id: self.id,
            target_type: pin_target_type_from_str(&self.target_type)?,
            target_id: self.target_id,
            reason: self.reason,
            created_at: self.created_at,
            expires_at: self.expires_at,
            scope_ids: serde_json::from_str(&self.scope_ids)?,
        })
    }
}
