/**
 * SQLite kindling Store - Write Path
 *
 * Provides atomic, deterministic writes for observations, capsules, summaries, and pins
 */

import type Database from 'better-sqlite3';
import type { Observation, Capsule, Summary, Pin, ScopeIds } from '@eddacraft/kindling-core';
import {
  exportDatabase as exportDB,
  importDatabase as importDB,
  type ExportOptions,
  type ExportDataset,
} from './export.js';

/**
 * Evidence snippet with context
 */
export interface EvidenceSnippet {
  observationId: string;
  snippet: string;
  kind: string;
}

/**
 * SQLite-based kindling store implementation
 */
export class SqliteKindlingStore {
  constructor(private db: Database.Database) {}

  // ===== WRITE PATH =====

  /**
   * Insert an observation
   *
   * FTS sync happens automatically via triggers
   *
   * @param observation - Observation to insert
   */
  insertObservation(observation: Observation): void {
    const stmt = this.db.prepare(`
      INSERT INTO observations (
        id, kind, content, provenance, ts, scope_ids, redacted,
        session_id, repo_id, agent_id, user_id
      ) VALUES (
        @id, @kind, @content, @provenance, @ts, @scopeIds, @redacted,
        @sessionId, @repoId, @agentId, @userId
      )
    `);

    stmt.run({
      id: observation.id,
      kind: observation.kind,
      content: observation.content,
      provenance: JSON.stringify(observation.provenance),
      ts: observation.ts,
      scopeIds: JSON.stringify(observation.scopeIds),
      redacted: observation.redacted ? 1 : 0,
      sessionId: observation.scopeIds.sessionId ?? null,
      repoId: observation.scopeIds.repoId ?? null,
      agentId: observation.scopeIds.agentId ?? null,
      userId: observation.scopeIds.userId ?? null,
    });
  }

  /**
   * Create a new capsule
   *
   * @param capsule - Capsule to create
   */
  createCapsule(capsule: Capsule): void {
    const stmt = this.db.prepare(`
      INSERT INTO capsules (
        id, type, intent, status, opened_at, closed_at, scope_ids,
        session_id, repo_id, agent_id, user_id
      ) VALUES (
        @id, @type, @intent, @status, @openedAt, @closedAt, @scopeIds,
        @sessionId, @repoId, @agentId, @userId
      )
    `);

    stmt.run({
      id: capsule.id,
      type: capsule.type,
      intent: capsule.intent,
      status: capsule.status,
      openedAt: capsule.openedAt,
      closedAt: capsule.closedAt ?? null,
      scopeIds: JSON.stringify(capsule.scopeIds),
      sessionId: capsule.scopeIds.sessionId ?? null,
      repoId: capsule.scopeIds.repoId ?? null,
      agentId: capsule.scopeIds.agentId ?? null,
      userId: capsule.scopeIds.userId ?? null,
    });
  }

  /**
   * Close a capsule
   *
   * Updates status to 'closed' and sets closedAt timestamp
   *
   * @param capsuleId - ID of capsule to close
   * @param closedAt - Timestamp when capsule was closed (defaults to now)
   * @param summaryId - Optional summary ID to attach
   */
  closeCapsule(capsuleId: string, closedAt?: number, summaryId?: string): void {
    const updateStmt = this.db.prepare(`
      UPDATE capsules
      SET status = 'closed',
          closed_at = @closedAt
      WHERE id = @id AND status = 'open'
    `);

    const result = updateStmt.run({
      id: capsuleId,
      closedAt: closedAt ?? Date.now(),
    });

    if (result.changes === 0) {
      throw new Error(`Capsule ${capsuleId} not found or already closed`);
    }

    // If summaryId provided, update the capsule
    if (summaryId) {
      // Note: In SQLite, we don't have a summaryId column in capsules table
      // The relationship is managed via summaries.capsule_id
      // This is just a validation that the summary exists
      const summaryCheck = this.db
        .prepare(
          `
        SELECT id FROM summaries WHERE id = ? AND capsule_id = ?
      `,
        )
        .get(summaryId, capsuleId);

      if (!summaryCheck) {
        throw new Error(`Summary ${summaryId} not found for capsule ${capsuleId}`);
      }
    }
  }

  /**
   * Attach an observation to a capsule
   *
   * Maintains deterministic ordering via seq column
   *
   * @param capsuleId - ID of capsule
   * @param observationId - ID of observation to attach
   */
  attachObservationToCapsule(capsuleId: string, observationId: string): void {
    // Get next sequence number for this capsule
    const seqResult = this.db
      .prepare(
        `
      SELECT COALESCE(MAX(seq), -1) + 1 as next_seq
      FROM capsule_observations
      WHERE capsule_id = ?
    `,
      )
      .get(capsuleId) as { next_seq: number };

    const stmt = this.db.prepare(`
      INSERT INTO capsule_observations (capsule_id, observation_id, seq)
      VALUES (?, ?, ?)
    `);

    stmt.run(capsuleId, observationId, seqResult.next_seq);
  }

  /**
   * Insert a summary
   *
   * FTS sync happens automatically via triggers
   *
   * @param summary - Summary to insert
   */
  insertSummary(summary: Summary): void {
    const stmt = this.db.prepare(`
      INSERT INTO summaries (
        id, capsule_id, content, confidence, created_at, evidence_refs
      ) VALUES (
        @id, @capsuleId, @content, @confidence, @createdAt, @evidenceRefs
      )
    `);

    stmt.run({
      id: summary.id,
      capsuleId: summary.capsuleId,
      content: summary.content,
      confidence: summary.confidence,
      createdAt: summary.createdAt,
      evidenceRefs: JSON.stringify(summary.evidenceRefs),
    });
  }

  /**
   * Insert a pin
   *
   * @param pin - Pin to insert
   */
  insertPin(pin: Pin): void {
    const stmt = this.db.prepare(`
      INSERT INTO pins (
        id, target_type, target_id, reason, created_at, expires_at, scope_ids,
        session_id, repo_id, agent_id, user_id
      ) VALUES (
        @id, @targetType, @targetId, @reason, @createdAt, @expiresAt, @scopeIds,
        @sessionId, @repoId, @agentId, @userId
      )
    `);

    stmt.run({
      id: pin.id,
      targetType: pin.targetType,
      targetId: pin.targetId,
      reason: pin.reason ?? null,
      createdAt: pin.createdAt,
      expiresAt: pin.expiresAt ?? null,
      scopeIds: JSON.stringify(pin.scopeIds),
      sessionId: pin.scopeIds.sessionId ?? null,
      repoId: pin.scopeIds.repoId ?? null,
      agentId: pin.scopeIds.agentId ?? null,
      userId: pin.scopeIds.userId ?? null,
    });
  }

  /**
   * Delete a pin
   *
   * @param pinId - ID of pin to delete
   */
  deletePin(pinId: string): void {
    const stmt = this.db.prepare(`
      DELETE FROM pins WHERE id = ?
    `);

    const result = stmt.run(pinId);

    if (result.changes === 0) {
      throw new Error(`Pin ${pinId} not found`);
    }
  }

  /**
   * Get active pins (TTL-aware)
   *
   * @param scopeIds - Optional scope filter
   * @param now - Current timestamp for TTL check (defaults to Date.now())
   * @returns Array of active pins
   */
  listActivePins(scopeIds?: Partial<Record<string, string>>, now?: number): Pin[] {
    const currentTime = now ?? Date.now();
    let query = `
      SELECT id, target_type, target_id, reason, created_at, expires_at, scope_ids
      FROM pins
      WHERE (expires_at IS NULL OR expires_at > ?)
    `;

    const params: (string | number)[] = [currentTime];

    // Add scope filtering using denormalized columns
    if (scopeIds) {
      if (scopeIds.sessionId) {
        query += ` AND session_id = ?`;
        params.push(scopeIds.sessionId);
      }
      if (scopeIds.repoId) {
        query += ` AND repo_id = ?`;
        params.push(scopeIds.repoId);
      }
      if (scopeIds.agentId) {
        query += ` AND agent_id = ?`;
        params.push(scopeIds.agentId);
      }
      if (scopeIds.userId) {
        query += ` AND user_id = ?`;
        params.push(scopeIds.userId);
      }
    }

    query += ` ORDER BY created_at DESC`;

    const rows = this.db.prepare(query).all(...params) as Array<{
      id: string;
      target_type: string;
      target_id: string;
      reason: string | null;
      created_at: number;
      expires_at: number | null;
      scope_ids: string;
    }>;

    return rows.map((row) => ({
      id: row.id,
      targetType: row.target_type as 'observation' | 'summary',
      targetId: row.target_id,
      reason: row.reason ?? undefined,
      createdAt: row.created_at,
      expiresAt: row.expires_at ?? undefined,
      scopeIds: JSON.parse(row.scope_ids),
    }));
  }

  /**
   * Execute a function within a transaction
   *
   * Automatically commits on success, rolls back on error
   *
   * @param fn - Function to execute within transaction
   * @returns Result of function
   */
  transaction<T>(fn: () => T): T {
    const txn = this.db.transaction(fn);
    return txn();
  }

  /**
   * Redact an observation
   *
   * Sets content to '[redacted]', marks redacted flag, and removes from FTS
   *
   * @param observationId - ID of observation to redact
   */
  redactObservation(observationId: string): void {
    const stmt = this.db.prepare(`
      UPDATE observations
      SET content = '[redacted]',
          redacted = 1
      WHERE id = ?
    `);

    const result = stmt.run(observationId);

    if (result.changes === 0) {
      throw new Error(`Observation ${observationId} not found`);
    }

    // Note: FTS cleanup is handled by the observations_fts_update trigger
    // in migration 002_fts.sql
  }

  // ===== READ PATH =====

  /**
   * Get open capsule for a session
   *
   * @param sessionId - Session ID to search for
   * @returns Open capsule or undefined if none exists
   */
  getOpenCapsuleForSession(sessionId: string): Capsule | undefined {
    const row = this.db
      .prepare(
        `
      SELECT id, type, intent, status, opened_at, closed_at, scope_ids
      FROM capsules
      WHERE status = 'open'
        AND session_id = ?
      ORDER BY opened_at DESC
      LIMIT 1
    `,
      )
      .get(sessionId) as
      | {
          id: string;
          type: string;
          intent: string;
          status: string;
          opened_at: number;
          closed_at: number | null;
          scope_ids: string;
        }
      | undefined;

    if (!row) {
      return undefined;
    }

    // Get observation IDs for this capsule
    const obsRows = this.db
      .prepare(
        `
      SELECT observation_id
      FROM capsule_observations
      WHERE capsule_id = ?
      ORDER BY seq
    `,
      )
      .all(row.id) as Array<{ observation_id: string }>;

    return {
      id: row.id,
      type: row.type as Capsule['type'],
      intent: row.intent,
      status: row.status as Capsule['status'],
      openedAt: row.opened_at,
      closedAt: row.closed_at ?? undefined,
      scopeIds: JSON.parse(row.scope_ids),
      observationIds: obsRows.map((r) => r.observation_id),
      summaryId: undefined, // Will be set if summary exists
    };
  }

  /**
   * Get latest summary for a capsule
   *
   * @param capsuleId - Capsule ID
   * @returns Summary or undefined if none exists
   */
  getLatestSummaryForCapsule(capsuleId: string): Summary | undefined {
    const row = this.db
      .prepare(
        `
      SELECT id, capsule_id, content, confidence, created_at, evidence_refs
      FROM summaries
      WHERE capsule_id = ?
      ORDER BY created_at DESC
      LIMIT 1
    `,
      )
      .get(capsuleId) as
      | {
          id: string;
          capsule_id: string;
          content: string;
          confidence: number;
          created_at: number;
          evidence_refs: string;
        }
      | undefined;

    if (!row) {
      return undefined;
    }

    return {
      id: row.id,
      capsuleId: row.capsule_id,
      content: row.content,
      confidence: row.confidence,
      createdAt: row.created_at,
      evidenceRefs: JSON.parse(row.evidence_refs),
    };
  }

  /**
   * Get evidence snippets for observation IDs
   *
   * Truncates content to maxChars per observation
   *
   * @param observationIds - IDs of observations to retrieve
   * @param maxChars - Maximum characters per snippet (default: 200)
   * @returns Array of evidence snippets
   */
  getEvidenceSnippets(observationIds: string[], maxChars: number = 200): EvidenceSnippet[] {
    if (observationIds.length === 0) {
      return [];
    }

    const placeholders = observationIds.map(() => '?').join(',');
    const rows = this.db
      .prepare(
        `
      SELECT id, kind, content
      FROM observations
      WHERE id IN (${placeholders})
    `,
      )
      .all(...observationIds) as Array<{
      id: string;
      kind: string;
      content: string;
    }>;

    const rowMap = new Map(rows.map((row) => [row.id, row]));

    return observationIds
      .map((id) => rowMap.get(id))
      .filter((row): row is NonNullable<typeof row> => row !== undefined)
      .map((row) => ({
        observationId: row.id,
        kind: row.kind,
        snippet:
          row.content.length > maxChars ? row.content.substring(0, maxChars) + '...' : row.content,
      }));
  }

  /**
   * Get observation by ID
   *
   * @param observationId - Observation ID
   * @returns Observation or undefined
   */
  getObservationById(observationId: string): Observation | undefined {
    const row = this.db
      .prepare(
        `
      SELECT id, kind, content, provenance, ts, scope_ids, redacted
      FROM observations
      WHERE id = ?
    `,
      )
      .get(observationId) as
      | {
          id: string;
          kind: string;
          content: string;
          provenance: string;
          ts: number;
          scope_ids: string;
          redacted: number;
        }
      | undefined;

    if (!row) {
      return undefined;
    }

    return {
      id: row.id,
      kind: row.kind as Observation['kind'],
      content: row.content,
      provenance: JSON.parse(row.provenance),
      ts: row.ts,
      scopeIds: JSON.parse(row.scope_ids),
      redacted: row.redacted === 1,
    };
  }

  /**
   * Get summary by ID
   *
   * @param summaryId - Summary ID
   * @returns Summary or undefined
   */
  getSummaryById(summaryId: string): Summary | undefined {
    const row = this.db
      .prepare(
        `
      SELECT id, capsule_id, content, confidence, created_at, evidence_refs
      FROM summaries
      WHERE id = ?
    `,
      )
      .get(summaryId) as
      | {
          id: string;
          capsule_id: string;
          content: string;
          confidence: number;
          created_at: number;
          evidence_refs: string;
        }
      | undefined;

    if (!row) {
      return undefined;
    }

    return {
      id: row.id,
      capsuleId: row.capsule_id,
      content: row.content,
      confidence: row.confidence,
      createdAt: row.created_at,
      evidenceRefs: JSON.parse(row.evidence_refs),
    };
  }

  /**
   * Query observations by scope and time range
   *
   * @param scopeIds - Optional scope filter
   * @param fromTs - Optional start timestamp
   * @param toTs - Optional end timestamp
   * @param limit - Maximum results to return
   * @returns Array of observations
   */
  queryObservations(
    scopeIds?: Partial<ScopeIds>,
    fromTs?: number,
    toTs?: number,
    limit: number = 100,
  ): Observation[] {
    let query = `
      SELECT id, kind, content, provenance, ts, scope_ids, redacted
      FROM observations
      WHERE redacted = 0
    `;
    const params: (string | number)[] = [];

    // Add scope filtering using denormalized columns
    if (scopeIds?.sessionId) {
      query += ` AND session_id = ?`;
      params.push(scopeIds.sessionId);
    }
    if (scopeIds?.repoId) {
      query += ` AND repo_id = ?`;
      params.push(scopeIds.repoId);
    }
    if (scopeIds?.agentId) {
      query += ` AND agent_id = ?`;
      params.push(scopeIds.agentId);
    }
    if (scopeIds?.userId) {
      query += ` AND user_id = ?`;
      params.push(scopeIds.userId);
    }

    // Add time range filtering
    if (fromTs !== undefined) {
      query += ` AND ts >= ?`;
      params.push(fromTs);
    }
    if (toTs !== undefined) {
      query += ` AND ts <= ?`;
      params.push(toTs);
    }

    query += ` ORDER BY ts DESC LIMIT ?`;
    params.push(limit);

    const rows = this.db.prepare(query).all(...params) as Array<{
      id: string;
      kind: string;
      content: string;
      provenance: string;
      ts: number;
      scope_ids: string;
      redacted: number;
    }>;

    return rows.map((row) => ({
      id: row.id,
      kind: row.kind as Observation['kind'],
      content: row.content,
      provenance: JSON.parse(row.provenance),
      ts: row.ts,
      scopeIds: JSON.parse(row.scope_ids),
      redacted: row.redacted === 1,
    }));
  }

  /**
   * Get capsule by ID
   * Alias for getCapsuleById for compatibility
   */
  getCapsule(capsuleId: string): Capsule | undefined {
    const query = `
      SELECT id, type, intent, status, opened_at, closed_at, scope_ids
      FROM capsules
      WHERE id = ?
    `;

    const row = this.db.prepare(query).get(capsuleId) as
      | {
          id: string;
          type: string;
          intent: string;
          status: string;
          opened_at: number;
          closed_at: number | null;
          scope_ids: string;
        }
      | undefined;

    if (!row) return undefined;

    // Get observation IDs
    const obsIds = this.db
      .prepare(
        `
      SELECT observation_id
      FROM capsule_observations
      WHERE capsule_id = ?
      ORDER BY seq ASC
    `,
      )
      .all(capsuleId) as Array<{ observation_id: string }>;

    return {
      id: row.id,
      type: row.type as Capsule['type'],
      intent: row.intent,
      status: row.status as Capsule['status'],
      openedAt: row.opened_at,
      closedAt: row.closed_at ?? undefined,
      scopeIds: JSON.parse(row.scope_ids),
      observationIds: obsIds.map((o) => o.observation_id),
    };
  }

  /**
   * Create summary
   * Alias for insertSummary for compatibility
   */
  createSummary(summary: Summary): void {
    this.insertSummary(summary);
  }

  /**
   * Create pin
   * Alias for insertPin for compatibility
   */
  createPin(pin: Pin): void {
    this.insertPin(pin);
  }

  /**
   * Remove pin
   * Alias for deletePin for compatibility
   */
  removePin(pinId: string): void {
    this.deletePin(pinId);
  }

  /**
   * Export database to dataset
   *
   * @param options - Export options
   * @returns Export dataset
   */
  exportDatabase(options?: ExportOptions) {
    return exportDB(this.db, options);
  }

  /**
   * Import dataset into database
   *
   * @param dataset - Dataset to import
   * @returns Import result
   */
  importDatabase(dataset: ExportDataset) {
    return importDB(this.db, dataset);
  }
}
