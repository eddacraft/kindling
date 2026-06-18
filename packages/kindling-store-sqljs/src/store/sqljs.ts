/**
 * sql.js kindling Store - Write and Read Path
 *
 * Provides atomic, deterministic writes for observations, capsules, summaries, and pins
 * using sql.js (WASM SQLite) for browser compatibility.
 */

import type { Database, QueryExecResult } from 'sql.js';
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
 * Helper to get first row from query result
 */
function getFirst<T>(result: QueryExecResult[]): T | undefined {
  if (result.length === 0 || result[0].values.length === 0) {
    return undefined;
  }

  const columns = result[0].columns;
  const values = result[0].values[0];
  const row: Record<string, unknown> = {};

  for (let i = 0; i < columns.length; i++) {
    row[columns[i]] = values[i];
  }

  return row as T;
}

/**
 * Helper to get all rows from query result
 */
function getAll<T>(result: QueryExecResult[]): T[] {
  if (result.length === 0) {
    return [];
  }

  const columns = result[0].columns;
  return result[0].values.map((values) => {
    const row: Record<string, unknown> = {};
    for (let i = 0; i < columns.length; i++) {
      row[columns[i]] = values[i];
    }
    return row as T;
  });
}

/**
 * sql.js-based kindling store implementation
 *
 * This is a drop-in replacement for SqliteKindlingStore that uses
 * sql.js instead of better-sqlite3, enabling browser compatibility.
 */
export class SqljsKindlingStore {
  constructor(private db: Database) {}

  // ===== WRITE PATH =====

  /**
   * Insert an observation
   */
  insertObservation(observation: Observation): void {
    this.db.run(
      `INSERT INTO observations (
        id, kind, content, provenance, ts, scope_ids, redacted
      ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
      [
        observation.id,
        observation.kind,
        observation.content,
        JSON.stringify(observation.provenance),
        observation.ts,
        JSON.stringify(observation.scopeIds),
        observation.redacted ? 1 : 0,
      ],
    );
  }

  /**
   * Create a new capsule
   */
  createCapsule(capsule: Capsule): void {
    this.db.run(
      `INSERT INTO capsules (
        id, type, intent, status, opened_at, closed_at, scope_ids
      ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
      [
        capsule.id,
        capsule.type,
        capsule.intent,
        capsule.status,
        capsule.openedAt,
        capsule.closedAt ?? null,
        JSON.stringify(capsule.scopeIds),
      ],
    );
  }

  /**
   * Close a capsule
   */
  closeCapsule(capsuleId: string, closedAt?: number, summaryId?: string): void {
    this.db.run(
      `UPDATE capsules
       SET status = 'closed', closed_at = ?
       WHERE id = ? AND status = 'open'`,
      [closedAt ?? Date.now(), capsuleId],
    );

    const changes = this.db.getRowsModified();
    if (changes === 0) {
      throw new Error(`Capsule ${capsuleId} not found or already closed`);
    }

    if (summaryId) {
      const result = this.db.exec(`SELECT id FROM summaries WHERE id = ? AND capsule_id = ?`, [
        summaryId,
        capsuleId,
      ]);
      if (result.length === 0 || result[0].values.length === 0) {
        throw new Error(`Summary ${summaryId} not found for capsule ${capsuleId}`);
      }
    }
  }

  /**
   * Attach an observation to a capsule
   */
  attachObservationToCapsule(capsuleId: string, observationId: string): void {
    const result = this.db.exec(
      `SELECT COALESCE(MAX(seq), -1) + 1 as next_seq
       FROM capsule_observations
       WHERE capsule_id = ?`,
      [capsuleId],
    );

    const nextSeq = getFirst<{ next_seq: number }>(result)?.next_seq ?? 0;

    this.db.run(
      `INSERT INTO capsule_observations (capsule_id, observation_id, seq)
       VALUES (?, ?, ?)`,
      [capsuleId, observationId, nextSeq],
    );
  }

  /**
   * Insert a summary
   */
  insertSummary(summary: Summary): void {
    this.db.run(
      `INSERT INTO summaries (
        id, capsule_id, content, confidence, created_at, evidence_refs
      ) VALUES (?, ?, ?, ?, ?, ?)`,
      [
        summary.id,
        summary.capsuleId,
        summary.content,
        summary.confidence,
        summary.createdAt,
        JSON.stringify(summary.evidenceRefs),
      ],
    );
  }

  /**
   * Insert a pin
   */
  insertPin(pin: Pin): void {
    this.db.run(
      `INSERT INTO pins (
        id, target_type, target_id, reason, created_at, expires_at, scope_ids
      ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
      [
        pin.id,
        pin.targetType,
        pin.targetId,
        pin.reason ?? null,
        pin.createdAt,
        pin.expiresAt ?? null,
        JSON.stringify(pin.scopeIds),
      ],
    );
  }

  /**
   * Delete a pin
   */
  deletePin(pinId: string): void {
    this.db.run(`DELETE FROM pins WHERE id = ?`, [pinId]);

    if (this.db.getRowsModified() === 0) {
      throw new Error(`Pin ${pinId} not found`);
    }
  }

  /**
   * Get active pins (TTL-aware)
   */
  listActivePins(scopeIds?: Partial<Record<string, string>>, now?: number): Pin[] {
    const currentTime = now ?? Date.now();
    let query = `
      SELECT id, target_type, target_id, reason, created_at, expires_at, scope_ids
      FROM pins
      WHERE (expires_at IS NULL OR expires_at > ?)
    `;
    const params: (string | number)[] = [currentTime];

    if (scopeIds) {
      if (scopeIds.sessionId) {
        query += ` AND json_extract(scope_ids, '$.sessionId') = ?`;
        params.push(scopeIds.sessionId);
      }
      if (scopeIds.repoId) {
        query += ` AND json_extract(scope_ids, '$.repoId') = ?`;
        params.push(scopeIds.repoId);
      }
      if (scopeIds.agentId) {
        query += ` AND json_extract(scope_ids, '$.agentId') = ?`;
        params.push(scopeIds.agentId);
      }
      if (scopeIds.userId) {
        query += ` AND json_extract(scope_ids, '$.userId') = ?`;
        params.push(scopeIds.userId);
      }
    }

    query += ` ORDER BY created_at DESC`;

    const result = this.db.exec(query, params);
    const rows = getAll<{
      id: string;
      target_type: string;
      target_id: string;
      reason: string | null;
      created_at: number;
      expires_at: number | null;
      scope_ids: string;
    }>(result);

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
   * Note: sql.js doesn't have the same transaction API as better-sqlite3,
   * so we use manual BEGIN/COMMIT/ROLLBACK.
   */
  transaction<T>(fn: () => T): T {
    this.db.run('BEGIN TRANSACTION');
    try {
      const result = fn();
      this.db.run('COMMIT');
      return result;
    } catch (err) {
      this.db.run('ROLLBACK');
      throw err;
    }
  }

  /**
   * Redact an observation
   */
  redactObservation(observationId: string): void {
    this.db.run(
      `UPDATE observations
       SET content = '[redacted]', redacted = 1
       WHERE id = ?`,
      [observationId],
    );

    if (this.db.getRowsModified() === 0) {
      throw new Error(`Observation ${observationId} not found`);
    }
  }

  // ===== READ PATH =====

  /**
   * Get open capsule for a session
   */
  getOpenCapsuleForSession(sessionId: string): Capsule | undefined {
    const result = this.db.exec(
      `SELECT id, type, intent, status, opened_at, closed_at, scope_ids
       FROM capsules
       WHERE status = 'open'
         AND json_extract(scope_ids, '$.sessionId') = ?
       ORDER BY opened_at DESC
       LIMIT 1`,
      [sessionId],
    );

    const row = getFirst<{
      id: string;
      type: string;
      intent: string;
      status: string;
      opened_at: number;
      closed_at: number | null;
      scope_ids: string;
    }>(result);

    if (!row) {
      return undefined;
    }

    const obsResult = this.db.exec(
      `SELECT observation_id
       FROM capsule_observations
       WHERE capsule_id = ?
       ORDER BY seq`,
      [row.id],
    );

    const obsRows = getAll<{ observation_id: string }>(obsResult);

    return {
      id: row.id,
      type: row.type as Capsule['type'],
      intent: row.intent,
      status: row.status as Capsule['status'],
      openedAt: row.opened_at,
      closedAt: row.closed_at ?? undefined,
      scopeIds: JSON.parse(row.scope_ids),
      observationIds: obsRows.map((r) => r.observation_id),
      summaryId: undefined,
    };
  }

  /**
   * Get latest summary for a capsule
   */
  getLatestSummaryForCapsule(capsuleId: string): Summary | undefined {
    const result = this.db.exec(
      `SELECT id, capsule_id, content, confidence, created_at, evidence_refs
       FROM summaries
       WHERE capsule_id = ?
       ORDER BY created_at DESC
       LIMIT 1`,
      [capsuleId],
    );

    const row = getFirst<{
      id: string;
      capsule_id: string;
      content: string;
      confidence: number;
      created_at: number;
      evidence_refs: string;
    }>(result);

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
   */
  getEvidenceSnippets(observationIds: string[], maxChars: number = 200): EvidenceSnippet[] {
    if (observationIds.length === 0) {
      return [];
    }

    const placeholders = observationIds.map(() => '?').join(',');
    const result = this.db.exec(
      `SELECT id, kind, content
       FROM observations
       WHERE id IN (${placeholders})`,
      observationIds,
    );

    const rows = getAll<{ id: string; kind: string; content: string }>(result);
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
   */
  getObservationById(observationId: string): Observation | undefined {
    const result = this.db.exec(
      `SELECT id, kind, content, provenance, ts, scope_ids, redacted
       FROM observations
       WHERE id = ?`,
      [observationId],
    );

    const row = getFirst<{
      id: string;
      kind: string;
      content: string;
      provenance: string;
      ts: number;
      scope_ids: string;
      redacted: number;
    }>(result);

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
   */
  getSummaryById(summaryId: string): Summary | undefined {
    const result = this.db.exec(
      `SELECT id, capsule_id, content, confidence, created_at, evidence_refs
       FROM summaries
       WHERE id = ?`,
      [summaryId],
    );

    const row = getFirst<{
      id: string;
      capsule_id: string;
      content: string;
      confidence: number;
      created_at: number;
      evidence_refs: string;
    }>(result);

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

    if (scopeIds?.sessionId) {
      query += ` AND json_extract(scope_ids, '$.sessionId') = ?`;
      params.push(scopeIds.sessionId);
    }
    if (scopeIds?.repoId) {
      query += ` AND json_extract(scope_ids, '$.repoId') = ?`;
      params.push(scopeIds.repoId);
    }
    if (scopeIds?.agentId) {
      query += ` AND json_extract(scope_ids, '$.agentId') = ?`;
      params.push(scopeIds.agentId);
    }
    if (scopeIds?.userId) {
      query += ` AND json_extract(scope_ids, '$.userId') = ?`;
      params.push(scopeIds.userId);
    }

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

    const result = this.db.exec(query, params);
    const rows = getAll<{
      id: string;
      kind: string;
      content: string;
      provenance: string;
      ts: number;
      scope_ids: string;
      redacted: number;
    }>(result);

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
   */
  getCapsule(capsuleId: string): Capsule | undefined {
    const result = this.db.exec(
      `SELECT id, type, intent, status, opened_at, closed_at, scope_ids
       FROM capsules
       WHERE id = ?`,
      [capsuleId],
    );

    const row = getFirst<{
      id: string;
      type: string;
      intent: string;
      status: string;
      opened_at: number;
      closed_at: number | null;
      scope_ids: string;
    }>(result);

    if (!row) {
      return undefined;
    }

    const obsResult = this.db.exec(
      `SELECT observation_id
       FROM capsule_observations
       WHERE capsule_id = ?
       ORDER BY seq ASC`,
      [capsuleId],
    );

    const obsIds = getAll<{ observation_id: string }>(obsResult);

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
   * Create summary (alias for insertSummary)
   */
  createSummary(summary: Summary): void {
    this.insertSummary(summary);
  }

  /**
   * Create pin (alias for insertPin)
   */
  createPin(pin: Pin): void {
    this.insertPin(pin);
  }

  /**
   * Remove pin (alias for deletePin)
   */
  removePin(pinId: string): void {
    this.deletePin(pinId);
  }

  /**
   * Export database to dataset
   */
  exportDatabase(options?: ExportOptions) {
    return exportDB(this.db, options);
  }

  /**
   * Import dataset into database
   */
  importDatabase(dataset: ExportDataset) {
    return importDB(this.db, dataset);
  }

  /**
   * Get the underlying database instance
   * Useful for persistence operations
   */
  getDatabase(): Database {
    return this.db;
  }
}
