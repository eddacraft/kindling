"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// scripts/bundle-entry.js
var bundle_entry_exports = {};
__export(bundle_entry_exports, {
  KindlingService: () => KindlingService,
  LocalFtsProvider: () => LocalFtsProvider,
  SqliteKindlingStore: () => SqliteKindlingStore,
  closeDatabase: () => closeDatabase,
  createHookHandlers: () => createHookHandlers,
  extractProvenance: () => extractProvenance,
  filterContent: () => filterContent,
  filterToolResult: () => filterToolResult,
  mapEvent: () => mapEvent,
  maskSecrets: () => maskSecrets,
  openDatabase: () => openDatabase,
  runMigrations: () => runMigrations,
  truncateContent: () => truncateContent,
  validateCapsule: () => validateCapsule,
  validateObservation: () => validateObservation,
  validateSummary: () => validateSummary
});
module.exports = __toCommonJS(bundle_entry_exports);

// ../../packages/kindling-store-sqlite/dist/db/open.js
var import_better_sqlite3 = __toESM(require("better-sqlite3"), 1);

// inline:inline-migrate
function getMigrations() {
  return [
    { version: 1, name: "001_init", sql: "-- Initial schema migration\n-- Creates core tables for observations, capsules, summaries, and pins\n\n-- Schema migrations tracking table\nCREATE TABLE IF NOT EXISTS schema_migrations (\n  version INTEGER PRIMARY KEY,\n  name TEXT NOT NULL,\n  applied_at INTEGER NOT NULL\n);\n\n-- Observations table\nCREATE TABLE IF NOT EXISTS observations (\n  id TEXT PRIMARY KEY,\n  kind TEXT NOT NULL CHECK(kind IN (\n    'tool_call',\n    'command',\n    'file_diff',\n    'error',\n    'message',\n    'node_start',\n    'node_end',\n    'node_output',\n    'node_error'\n  )),\n  content TEXT NOT NULL,\n  provenance TEXT NOT NULL DEFAULT '{}', -- JSON blob\n  ts INTEGER NOT NULL,\n  scope_ids TEXT NOT NULL DEFAULT '{}', -- JSON blob\n  redacted INTEGER NOT NULL DEFAULT 0 CHECK(redacted IN (0, 1))\n);\n\n-- Capsules table\nCREATE TABLE IF NOT EXISTS capsules (\n  id TEXT PRIMARY KEY,\n  type TEXT NOT NULL CHECK(type IN ('session', 'pocketflow_node')),\n  intent TEXT NOT NULL,\n  status TEXT NOT NULL CHECK(status IN ('open', 'closed')) DEFAULT 'open',\n  opened_at INTEGER NOT NULL,\n  closed_at INTEGER,\n  scope_ids TEXT NOT NULL DEFAULT '{}' -- JSON blob\n);\n\n-- Capsule-observation relationship table (many-to-many with ordering)\nCREATE TABLE IF NOT EXISTS capsule_observations (\n  capsule_id TEXT NOT NULL,\n  observation_id TEXT NOT NULL,\n  seq INTEGER NOT NULL, -- Ordering within capsule\n  PRIMARY KEY (capsule_id, observation_id),\n  FOREIGN KEY (capsule_id) REFERENCES capsules(id) ON DELETE CASCADE,\n  FOREIGN KEY (observation_id) REFERENCES observations(id) ON DELETE CASCADE\n);\n\n-- Summaries table\nCREATE TABLE IF NOT EXISTS summaries (\n  id TEXT PRIMARY KEY,\n  capsule_id TEXT NOT NULL UNIQUE,\n  content TEXT NOT NULL,\n  confidence REAL NOT NULL CHECK(confidence >= 0.0 AND confidence <= 1.0),\n  created_at INTEGER NOT NULL,\n  evidence_refs TEXT NOT NULL DEFAULT '[]', -- JSON array of observation IDs\n  FOREIGN KEY (capsule_id) REFERENCES capsules(id) ON DELETE CASCADE\n);\n\n-- Pins table\nCREATE TABLE IF NOT EXISTS pins (\n  id TEXT PRIMARY KEY,\n  target_type TEXT NOT NULL CHECK(target_type IN ('observation', 'summary')),\n  target_id TEXT NOT NULL,\n  reason TEXT,\n  created_at INTEGER NOT NULL,\n  expires_at INTEGER,\n  scope_ids TEXT NOT NULL DEFAULT '{}' -- JSON blob\n);\n\n-- Record this migration\nINSERT OR IGNORE INTO schema_migrations (version, name, applied_at)\nVALUES (1, '001_init', strftime('%s', 'now') * 1000);\n" },
    { version: 2, name: "002_fts", sql: "-- Full-text search indexes migration\n-- Creates FTS5 virtual tables for observations and summaries\n\n-- FTS table for observations content\nCREATE VIRTUAL TABLE IF NOT EXISTS observations_fts USING fts5(\n  content,\n  content='observations',\n  content_rowid='rowid',\n  tokenize='porter unicode61'\n);\n\n-- Populate FTS table with existing observations\nINSERT INTO observations_fts(rowid, content)\nSELECT rowid, content FROM observations WHERE redacted = 0;\n\n-- Trigger to keep FTS in sync on INSERT\nCREATE TRIGGER IF NOT EXISTS observations_fts_insert\nAFTER INSERT ON observations\nWHEN NEW.redacted = 0\nBEGIN\n  INSERT INTO observations_fts(rowid, content)\n  VALUES (NEW.rowid, NEW.content);\nEND;\n\n-- Trigger to keep FTS in sync on UPDATE\nCREATE TRIGGER IF NOT EXISTS observations_fts_update\nAFTER UPDATE ON observations\nBEGIN\n  -- Remove old entry (FTS5 external content tables require special delete syntax)\n  INSERT INTO observations_fts(observations_fts, rowid, content) VALUES('delete', OLD.rowid, OLD.content);\n  -- Add new entry only if not redacted\n  INSERT INTO observations_fts(rowid, content)\n  SELECT NEW.rowid, NEW.content WHERE NEW.redacted = 0;\nEND;\n\n-- Trigger to keep FTS in sync on DELETE\nCREATE TRIGGER IF NOT EXISTS observations_fts_delete\nAFTER DELETE ON observations\nBEGIN\n  INSERT INTO observations_fts(observations_fts, rowid, content) VALUES('delete', OLD.rowid, OLD.content);\nEND;\n\n-- FTS table for summaries content\nCREATE VIRTUAL TABLE IF NOT EXISTS summaries_fts USING fts5(\n  content,\n  content='summaries',\n  content_rowid='rowid',\n  tokenize='porter unicode61'\n);\n\n-- Populate FTS table with existing summaries\nINSERT INTO summaries_fts(rowid, content)\nSELECT rowid, content FROM summaries;\n\n-- Trigger to keep FTS in sync on INSERT\nCREATE TRIGGER IF NOT EXISTS summaries_fts_insert\nAFTER INSERT ON summaries\nBEGIN\n  INSERT INTO summaries_fts(rowid, content)\n  VALUES (NEW.rowid, NEW.content);\nEND;\n\n-- Trigger to keep FTS in sync on UPDATE\nCREATE TRIGGER IF NOT EXISTS summaries_fts_update\nAFTER UPDATE ON summaries\nBEGIN\n  INSERT INTO summaries_fts(summaries_fts, rowid, content) VALUES('delete', OLD.rowid, OLD.content);\n  INSERT INTO summaries_fts(rowid, content)\n  VALUES (NEW.rowid, NEW.content);\nEND;\n\n-- Trigger to keep FTS in sync on DELETE\nCREATE TRIGGER IF NOT EXISTS summaries_fts_delete\nAFTER DELETE ON summaries\nBEGIN\n  INSERT INTO summaries_fts(summaries_fts, rowid, content) VALUES('delete', OLD.rowid, OLD.content);\nEND;\n\n-- Record this migration\nINSERT OR IGNORE INTO schema_migrations (version, name, applied_at)\nVALUES (2, '002_fts', strftime('%s', 'now') * 1000);\n" },
    { version: 3, name: "003_indexes", sql: "-- Indexes migration\n-- Creates indexes for common query patterns\n\n-- Observations indexes\n\n-- Index for queries by timestamp (global time window)\nCREATE INDEX IF NOT EXISTS idx_observations_ts\nON observations(ts DESC);\n\n-- Index for queries by session + timestamp\nCREATE INDEX IF NOT EXISTS idx_observations_session_ts\nON observations(\n  json_extract(scope_ids, '$.sessionId'),\n  ts DESC\n) WHERE json_extract(scope_ids, '$.sessionId') IS NOT NULL;\n\n-- Index for queries by repo + timestamp\nCREATE INDEX IF NOT EXISTS idx_observations_repo_ts\nON observations(\n  json_extract(scope_ids, '$.repoId'),\n  ts DESC\n) WHERE json_extract(scope_ids, '$.repoId') IS NOT NULL;\n\n-- Index for queries by kind (for filtering)\nCREATE INDEX IF NOT EXISTS idx_observations_kind\nON observations(kind);\n\n-- Capsules indexes\n\n-- Index for queries by status + session (find open capsule for session)\nCREATE INDEX IF NOT EXISTS idx_capsules_status_session\nON capsules(\n  status,\n  json_extract(scope_ids, '$.sessionId')\n) WHERE json_extract(scope_ids, '$.sessionId') IS NOT NULL;\n\n-- Index for queries by opened_at (chronological listing)\nCREATE INDEX IF NOT EXISTS idx_capsules_opened_at\nON capsules(opened_at DESC);\n\n-- Index for queries by repo\nCREATE INDEX IF NOT EXISTS idx_capsules_repo\nON capsules(\n  json_extract(scope_ids, '$.repoId')\n) WHERE json_extract(scope_ids, '$.repoId') IS NOT NULL;\n\n-- Capsule-observations indexes\n\n-- Index for efficient lookups of observations by capsule\nCREATE INDEX IF NOT EXISTS idx_capsule_observations_capsule\nON capsule_observations(capsule_id, seq);\n\n-- Index for efficient lookups of capsules by observation\nCREATE INDEX IF NOT EXISTS idx_capsule_observations_observation\nON capsule_observations(observation_id);\n\n-- Summaries indexes\n\n-- Index for lookups by capsule_id (already unique, but helps with joins)\nCREATE INDEX IF NOT EXISTS idx_summaries_capsule\nON summaries(capsule_id);\n\n-- Index for queries by creation timestamp\nCREATE INDEX IF NOT EXISTS idx_summaries_created_at\nON summaries(created_at DESC);\n\n-- Pins indexes\n\n-- Index for TTL-aware queries (active pins)\nCREATE INDEX IF NOT EXISTS idx_pins_expires_at\nON pins(expires_at)\nWHERE expires_at IS NOT NULL;\n\n-- Index for queries by target\nCREATE INDEX IF NOT EXISTS idx_pins_target\nON pins(target_type, target_id);\n\n-- Index for queries by session\nCREATE INDEX IF NOT EXISTS idx_pins_session\nON pins(\n  json_extract(scope_ids, '$.sessionId')\n) WHERE json_extract(scope_ids, '$.sessionId') IS NOT NULL;\n\n-- Index for queries by repo\nCREATE INDEX IF NOT EXISTS idx_pins_repo\nON pins(\n  json_extract(scope_ids, '$.repoId')\n) WHERE json_extract(scope_ids, '$.repoId') IS NOT NULL;\n\n-- Record this migration\nINSERT OR IGNORE INTO schema_migrations (version, name, applied_at)\nVALUES (3, '003_indexes', strftime('%s', 'now') * 1000);\n" },
    { version: 4, name: "004_denormalize_scopes", sql: "-- Denormalize scope IDs from JSON blobs to real columns\n-- Eliminates json_extract() in WHERE clauses for ~20-30% faster filtered queries\n\n-- === observations ===\nALTER TABLE observations ADD COLUMN session_id TEXT;\nALTER TABLE observations ADD COLUMN repo_id TEXT;\nALTER TABLE observations ADD COLUMN agent_id TEXT;\nALTER TABLE observations ADD COLUMN user_id TEXT;\n\nUPDATE observations SET\n  session_id = json_extract(scope_ids, '$.sessionId'),\n  repo_id    = json_extract(scope_ids, '$.repoId'),\n  agent_id   = json_extract(scope_ids, '$.agentId'),\n  user_id    = json_extract(scope_ids, '$.userId');\n\n-- === capsules ===\nALTER TABLE capsules ADD COLUMN session_id TEXT;\nALTER TABLE capsules ADD COLUMN repo_id TEXT;\nALTER TABLE capsules ADD COLUMN agent_id TEXT;\nALTER TABLE capsules ADD COLUMN user_id TEXT;\n\nUPDATE capsules SET\n  session_id = json_extract(scope_ids, '$.sessionId'),\n  repo_id    = json_extract(scope_ids, '$.repoId'),\n  agent_id   = json_extract(scope_ids, '$.agentId'),\n  user_id    = json_extract(scope_ids, '$.userId');\n\n-- === pins ===\nALTER TABLE pins ADD COLUMN session_id TEXT;\nALTER TABLE pins ADD COLUMN repo_id TEXT;\nALTER TABLE pins ADD COLUMN agent_id TEXT;\nALTER TABLE pins ADD COLUMN user_id TEXT;\n\nUPDATE pins SET\n  session_id = json_extract(scope_ids, '$.sessionId'),\n  repo_id    = json_extract(scope_ids, '$.repoId'),\n  agent_id   = json_extract(scope_ids, '$.agentId'),\n  user_id    = json_extract(scope_ids, '$.userId');\n\n-- === New indexes on real columns ===\n\n-- Observations: session + timestamp (replaces idx_observations_session_ts)\nCREATE INDEX IF NOT EXISTS idx_obs_session_ts\nON observations(session_id, ts DESC)\nWHERE session_id IS NOT NULL;\n\n-- Observations: repo + timestamp (replaces idx_observations_repo_ts)\nCREATE INDEX IF NOT EXISTS idx_obs_repo_ts\nON observations(repo_id, ts DESC)\nWHERE repo_id IS NOT NULL;\n\n-- Capsules: status + session (replaces idx_capsules_status_session)\nCREATE INDEX IF NOT EXISTS idx_caps_status_session\nON capsules(status, session_id)\nWHERE session_id IS NOT NULL;\n\n-- Capsules: repo (replaces idx_capsules_repo)\nCREATE INDEX IF NOT EXISTS idx_caps_repo\nON capsules(repo_id)\nWHERE repo_id IS NOT NULL;\n\n-- Pins: session (replaces idx_pins_session)\nCREATE INDEX IF NOT EXISTS idx_pins_session_id\nON pins(session_id)\nWHERE session_id IS NOT NULL;\n\n-- Pins: repo (replaces idx_pins_repo)\nCREATE INDEX IF NOT EXISTS idx_pins_repo_id\nON pins(repo_id)\nWHERE repo_id IS NOT NULL;\n\n-- Drop old json_extract indexes (they're now redundant and slow)\nDROP INDEX IF EXISTS idx_observations_session_ts;\nDROP INDEX IF EXISTS idx_observations_repo_ts;\nDROP INDEX IF EXISTS idx_capsules_status_session;\nDROP INDEX IF EXISTS idx_capsules_repo;\nDROP INDEX IF EXISTS idx_pins_session;\nDROP INDEX IF EXISTS idx_pins_repo;\n\n-- Record this migration\nINSERT OR IGNORE INTO schema_migrations (version, name, applied_at)\nVALUES (4, '004_denormalize_scopes', strftime('%s', 'now') * 1000);\n" },
    { version: 5, name: "005_pragma_user_version", sql: "-- Set PRAGMA user_version so any SQLite client (including the Rust crate)\n-- can discover the schema version with a single read:\n--   PRAGMA user_version;\n--\n-- Convention: user_version tracks the latest migration number.\n-- Each future migration MUST include: PRAGMA user_version = <N>;\n\nPRAGMA user_version = 5;\n\n-- Record this migration\nINSERT OR IGNORE INTO schema_migrations (version, name, applied_at)\nVALUES (5, '005_pragma_user_version', strftime('%s', 'now') * 1000);\n" }
  ];
}
function getCurrentVersion(db) {
  try {
    const row = db.prepare("SELECT MAX(version) as version FROM schema_migrations").get();
    return row?.version ?? 0;
  } catch {
    return 0;
  }
}
function runMigrations(db) {
  const currentVersion = getCurrentVersion(db);
  const migrations = getMigrations();
  let applied = 0;
  for (const migration of migrations) {
    if (migration.version > currentVersion) {
      const applyMigration = db.transaction(() => {
        db.exec(migration.sql);
      });
      applyMigration();
      applied++;
    }
  }
  return applied;
}

// ../../packages/kindling-store-sqlite/dist/db/open.js
var import_os = require("os");
var import_path = require("path");
var import_fs = require("fs");
function getDefaultDbPath() {
  const kindlingDir = (0, import_path.join)((0, import_os.homedir)(), ".kindling");
  try {
    (0, import_fs.mkdirSync)(kindlingDir, { recursive: true });
  } catch (e) {
    const err2 = e;
    if (err2.code !== "EEXIST") {
      throw err2;
    }
  }
  return (0, import_path.join)(kindlingDir, "kindling.db");
}
function openDatabase(options = {}) {
  const dbPath = options.path ?? getDefaultDbPath();
  const db = new import_better_sqlite3.default(dbPath, {
    verbose: options.verbose ? console.log : void 0,
    readonly: options.readonly ?? false
  });
  db.pragma("journal_mode = WAL");
  db.pragma("foreign_keys = ON");
  db.pragma("busy_timeout = 5000");
  db.pragma("synchronous = NORMAL");
  db.pragma("cache_size = -64000");
  if (!options.readonly) {
    runMigrations(db);
  }
  return db;
}
function closeDatabase(db) {
  db.close();
}

// ../../packages/kindling-store-sqlite/dist/store/export.js
function exportDatabase(db, options = {}) {
  const { scope, includeRedacted = false, limit } = options;
  const buildScopeFilter = (tableName) => {
    if (!scope) {
      return { where: "", params: [] };
    }
    const conditions = [];
    const params = [];
    if (scope.sessionId) {
      conditions.push(`json_extract(${tableName}.scope_ids, '$.sessionId') = ?`);
      params.push(scope.sessionId);
    }
    if (scope.repoId) {
      conditions.push(`json_extract(${tableName}.scope_ids, '$.repoId') = ?`);
      params.push(scope.repoId);
    }
    if (scope.agentId) {
      conditions.push(`json_extract(${tableName}.scope_ids, '$.agentId') = ?`);
      params.push(scope.agentId);
    }
    if (scope.userId) {
      conditions.push(`json_extract(${tableName}.scope_ids, '$.userId') = ?`);
      params.push(scope.userId);
    }
    return {
      where: conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : "",
      params
    };
  };
  const obsFilter = buildScopeFilter("observations");
  const obsRedactedFilter = includeRedacted ? "" : "AND redacted = 0";
  const obsLimitClause = limit ? `LIMIT ${limit}` : "";
  const observationsQuery = `
    SELECT id, kind, content, provenance, ts, scope_ids, redacted
    FROM observations
    ${obsFilter.where}
    ${obsRedactedFilter ? obsFilter.where ? obsRedactedFilter : `WHERE ${obsRedactedFilter.substring(4)}` : ""}
    ORDER BY ts ASC, id ASC
    ${obsLimitClause}
  `;
  const obsRows = db.prepare(observationsQuery).all(...obsFilter.params);
  const observations = obsRows.map((row) => ({
    id: row.id,
    kind: row.kind,
    content: row.content,
    provenance: JSON.parse(row.provenance),
    ts: row.ts,
    scopeIds: JSON.parse(row.scope_ids),
    redacted: row.redacted === 1
  }));
  const capsuleFilter = buildScopeFilter("capsules");
  const capsulesQuery = `
    SELECT id, type, intent, status, opened_at, closed_at, scope_ids
    FROM capsules
    ${capsuleFilter.where}
    ORDER BY opened_at ASC, id ASC
  `;
  const capsuleRows = db.prepare(capsulesQuery).all(...capsuleFilter.params);
  const capsules = capsuleRows.map((row) => {
    const obsIds = db.prepare(`
      SELECT observation_id
      FROM capsule_observations
      WHERE capsule_id = ?
      ORDER BY seq ASC
    `).all(row.id);
    return {
      id: row.id,
      type: row.type,
      intent: row.intent,
      status: row.status,
      openedAt: row.opened_at,
      closedAt: row.closed_at ?? void 0,
      scopeIds: JSON.parse(row.scope_ids),
      observationIds: obsIds.map((o) => o.observation_id)
    };
  });
  const summariesQuery = `
    SELECT s.id, s.capsule_id, s.content, s.confidence, s.created_at, s.evidence_refs
    FROM summaries s
    INNER JOIN capsules c ON s.capsule_id = c.id
    ${capsuleFilter.where.replace("capsules.", "c.")}
    ORDER BY s.created_at ASC, s.id ASC
  `;
  const summaryRows = db.prepare(summariesQuery).all(...capsuleFilter.params);
  const summaries = summaryRows.map((row) => ({
    id: row.id,
    capsuleId: row.capsule_id,
    content: row.content,
    confidence: row.confidence,
    createdAt: row.created_at,
    evidenceRefs: JSON.parse(row.evidence_refs)
  }));
  const pinFilter = buildScopeFilter("pins");
  const pinsQuery = `
    SELECT id, target_type, target_id, reason, created_at, expires_at, scope_ids
    FROM pins
    ${pinFilter.where}
    ORDER BY created_at ASC, id ASC
  `;
  const pinRows = db.prepare(pinsQuery).all(...pinFilter.params);
  const pins = pinRows.map((row) => ({
    id: row.id,
    targetType: row.target_type,
    targetId: row.target_id,
    reason: row.reason ?? void 0,
    createdAt: row.created_at,
    expiresAt: row.expires_at ?? void 0,
    scopeIds: JSON.parse(row.scope_ids)
  }));
  return {
    version: "1.0",
    exportedAt: Date.now(),
    scope,
    observations,
    capsules,
    summaries,
    pins
  };
}
function importDatabase(db, dataset) {
  const errors = [];
  let obsCount = 0;
  let capsuleCount = 0;
  let summaryCount = 0;
  let pinCount = 0;
  if (dataset.version !== "1.0") {
    errors.push(`Unsupported schema version: ${dataset.version}`);
    return {
      observations: 0,
      capsules: 0,
      summaries: 0,
      pins: 0,
      errors
    };
  }
  const importTxn = db.transaction(() => {
    const obsStmt = db.prepare(`
      INSERT OR IGNORE INTO observations (id, kind, content, provenance, ts, scope_ids, redacted)
      VALUES (?, ?, ?, ?, ?, ?, ?)
    `);
    for (const obs of dataset.observations) {
      try {
        const result = obsStmt.run(obs.id, obs.kind, obs.content, JSON.stringify(obs.provenance), obs.ts, JSON.stringify(obs.scopeIds), obs.redacted ? 1 : 0);
        if (result.changes > 0)
          obsCount++;
      } catch (err2) {
        errors.push(`Failed to import observation ${obs.id}: ${err2}`);
      }
    }
    const capsuleStmt = db.prepare(`
      INSERT OR IGNORE INTO capsules (id, type, intent, status, opened_at, closed_at, scope_ids)
      VALUES (?, ?, ?, ?, ?, ?, ?)
    `);
    const capsuleObsStmt = db.prepare(`
      INSERT OR IGNORE INTO capsule_observations (capsule_id, observation_id, seq)
      VALUES (?, ?, ?)
    `);
    for (const capsule of dataset.capsules) {
      try {
        const result = capsuleStmt.run(capsule.id, capsule.type, capsule.intent, capsule.status, capsule.openedAt, capsule.closedAt ?? null, JSON.stringify(capsule.scopeIds));
        if (result.changes > 0) {
          capsuleCount++;
          capsule.observationIds.forEach((obsId, seq) => {
            capsuleObsStmt.run(capsule.id, obsId, seq);
          });
        }
      } catch (err2) {
        errors.push(`Failed to import capsule ${capsule.id}: ${err2}`);
      }
    }
    const summaryStmt = db.prepare(`
      INSERT OR IGNORE INTO summaries (id, capsule_id, content, confidence, created_at, evidence_refs)
      VALUES (?, ?, ?, ?, ?, ?)
    `);
    for (const summary of dataset.summaries) {
      try {
        const result = summaryStmt.run(summary.id, summary.capsuleId, summary.content, summary.confidence, summary.createdAt, JSON.stringify(summary.evidenceRefs));
        if (result.changes > 0)
          summaryCount++;
      } catch (err2) {
        errors.push(`Failed to import summary ${summary.id}: ${err2}`);
      }
    }
    const pinStmt = db.prepare(`
      INSERT OR IGNORE INTO pins (id, target_type, target_id, reason, created_at, expires_at, scope_ids)
      VALUES (?, ?, ?, ?, ?, ?, ?)
    `);
    for (const pin of dataset.pins) {
      try {
        const result = pinStmt.run(pin.id, pin.targetType, pin.targetId, pin.reason ?? null, pin.createdAt, pin.expiresAt ?? null, JSON.stringify(pin.scopeIds));
        if (result.changes > 0)
          pinCount++;
      } catch (err2) {
        errors.push(`Failed to import pin ${pin.id}: ${err2}`);
      }
    }
  });
  try {
    importTxn();
  } catch (err2) {
    errors.push(`Transaction failed: ${err2}`);
  }
  return {
    observations: obsCount,
    capsules: capsuleCount,
    summaries: summaryCount,
    pins: pinCount,
    errors
  };
}

// ../../packages/kindling-store-sqlite/dist/store/sqlite.js
var SqliteKindlingStore = class {
  db;
  constructor(db) {
    this.db = db;
  }
  // ===== WRITE PATH =====
  /**
   * Insert an observation
   *
   * FTS sync happens automatically via triggers
   *
   * @param observation - Observation to insert
   */
  insertObservation(observation) {
    const stmt = this.db.prepare(`
      INSERT INTO observations (
        id, kind, content, provenance, ts, scope_ids, redacted
      ) VALUES (
        @id, @kind, @content, @provenance, @ts, @scopeIds, @redacted
      )
    `);
    stmt.run({
      id: observation.id,
      kind: observation.kind,
      content: observation.content,
      provenance: JSON.stringify(observation.provenance),
      ts: observation.ts,
      scopeIds: JSON.stringify(observation.scopeIds),
      redacted: observation.redacted ? 1 : 0
    });
  }
  /**
   * Create a new capsule
   *
   * @param capsule - Capsule to create
   */
  createCapsule(capsule) {
    const stmt = this.db.prepare(`
      INSERT INTO capsules (
        id, type, intent, status, opened_at, closed_at, scope_ids
      ) VALUES (
        @id, @type, @intent, @status, @openedAt, @closedAt, @scopeIds
      )
    `);
    stmt.run({
      id: capsule.id,
      type: capsule.type,
      intent: capsule.intent,
      status: capsule.status,
      openedAt: capsule.openedAt,
      closedAt: capsule.closedAt ?? null,
      scopeIds: JSON.stringify(capsule.scopeIds)
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
  closeCapsule(capsuleId, closedAt, summaryId) {
    const updateStmt = this.db.prepare(`
      UPDATE capsules
      SET status = 'closed',
          closed_at = @closedAt
      WHERE id = @id AND status = 'open'
    `);
    const result = updateStmt.run({
      id: capsuleId,
      closedAt: closedAt ?? Date.now()
    });
    if (result.changes === 0) {
      throw new Error(`Capsule ${capsuleId} not found or already closed`);
    }
    if (summaryId) {
      const summaryCheck = this.db.prepare(`
        SELECT id FROM summaries WHERE id = ? AND capsule_id = ?
      `).get(summaryId, capsuleId);
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
  attachObservationToCapsule(capsuleId, observationId) {
    const seqResult = this.db.prepare(`
      SELECT COALESCE(MAX(seq), -1) + 1 as next_seq
      FROM capsule_observations
      WHERE capsule_id = ?
    `).get(capsuleId);
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
  insertSummary(summary) {
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
      evidenceRefs: JSON.stringify(summary.evidenceRefs)
    });
  }
  /**
   * Insert a pin
   *
   * @param pin - Pin to insert
   */
  insertPin(pin) {
    const stmt = this.db.prepare(`
      INSERT INTO pins (
        id, target_type, target_id, reason, created_at, expires_at, scope_ids
      ) VALUES (
        @id, @targetType, @targetId, @reason, @createdAt, @expiresAt, @scopeIds
      )
    `);
    stmt.run({
      id: pin.id,
      targetType: pin.targetType,
      targetId: pin.targetId,
      reason: pin.reason ?? null,
      createdAt: pin.createdAt,
      expiresAt: pin.expiresAt ?? null,
      scopeIds: JSON.stringify(pin.scopeIds)
    });
  }
  /**
   * Delete a pin
   *
   * @param pinId - ID of pin to delete
   */
  deletePin(pinId) {
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
  listActivePins(scopeIds, now) {
    const currentTime = now ?? Date.now();
    let query = `
      SELECT id, target_type, target_id, reason, created_at, expires_at, scope_ids
      FROM pins
      WHERE (expires_at IS NULL OR expires_at > ?)
    `;
    const params = [currentTime];
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
    const rows = this.db.prepare(query).all(...params);
    return rows.map((row) => ({
      id: row.id,
      targetType: row.target_type,
      targetId: row.target_id,
      reason: row.reason ?? void 0,
      createdAt: row.created_at,
      expiresAt: row.expires_at ?? void 0,
      scopeIds: JSON.parse(row.scope_ids)
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
  transaction(fn) {
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
  redactObservation(observationId) {
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
  }
  // ===== READ PATH =====
  /**
   * Get open capsule for a session
   *
   * @param sessionId - Session ID to search for
   * @returns Open capsule or undefined if none exists
   */
  getOpenCapsuleForSession(sessionId) {
    const row = this.db.prepare(`
      SELECT id, type, intent, status, opened_at, closed_at, scope_ids
      FROM capsules
      WHERE status = 'open'
        AND json_extract(scope_ids, '$.sessionId') = ?
      ORDER BY opened_at DESC
      LIMIT 1
    `).get(sessionId);
    if (!row) {
      return void 0;
    }
    const obsRows = this.db.prepare(`
      SELECT observation_id
      FROM capsule_observations
      WHERE capsule_id = ?
      ORDER BY seq
    `).all(row.id);
    return {
      id: row.id,
      type: row.type,
      intent: row.intent,
      status: row.status,
      openedAt: row.opened_at,
      closedAt: row.closed_at ?? void 0,
      scopeIds: JSON.parse(row.scope_ids),
      observationIds: obsRows.map((r) => r.observation_id),
      summaryId: void 0
      // Will be set if summary exists
    };
  }
  /**
   * Get latest summary for a capsule
   *
   * @param capsuleId - Capsule ID
   * @returns Summary or undefined if none exists
   */
  getLatestSummaryForCapsule(capsuleId) {
    const row = this.db.prepare(`
      SELECT id, capsule_id, content, confidence, created_at, evidence_refs
      FROM summaries
      WHERE capsule_id = ?
      ORDER BY created_at DESC
      LIMIT 1
    `).get(capsuleId);
    if (!row) {
      return void 0;
    }
    return {
      id: row.id,
      capsuleId: row.capsule_id,
      content: row.content,
      confidence: row.confidence,
      createdAt: row.created_at,
      evidenceRefs: JSON.parse(row.evidence_refs)
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
  getEvidenceSnippets(observationIds, maxChars = 200) {
    if (observationIds.length === 0) {
      return [];
    }
    const placeholders = observationIds.map(() => "?").join(",");
    const rows = this.db.prepare(`
      SELECT id, kind, content
      FROM observations
      WHERE id IN (${placeholders})
    `).all(...observationIds);
    const rowMap = new Map(rows.map((row) => [row.id, row]));
    return observationIds.map((id) => rowMap.get(id)).filter((row) => row !== void 0).map((row) => ({
      observationId: row.id,
      kind: row.kind,
      snippet: row.content.length > maxChars ? row.content.substring(0, maxChars) + "..." : row.content
    }));
  }
  /**
   * Get observation by ID
   *
   * @param observationId - Observation ID
   * @returns Observation or undefined
   */
  getObservationById(observationId) {
    const row = this.db.prepare(`
      SELECT id, kind, content, provenance, ts, scope_ids, redacted
      FROM observations
      WHERE id = ?
    `).get(observationId);
    if (!row) {
      return void 0;
    }
    return {
      id: row.id,
      kind: row.kind,
      content: row.content,
      provenance: JSON.parse(row.provenance),
      ts: row.ts,
      scopeIds: JSON.parse(row.scope_ids),
      redacted: row.redacted === 1
    };
  }
  /**
   * Get summary by ID
   *
   * @param summaryId - Summary ID
   * @returns Summary or undefined
   */
  getSummaryById(summaryId) {
    const row = this.db.prepare(`
      SELECT id, capsule_id, content, confidence, created_at, evidence_refs
      FROM summaries
      WHERE id = ?
    `).get(summaryId);
    if (!row) {
      return void 0;
    }
    return {
      id: row.id,
      capsuleId: row.capsule_id,
      content: row.content,
      confidence: row.confidence,
      createdAt: row.created_at,
      evidenceRefs: JSON.parse(row.evidence_refs)
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
  queryObservations(scopeIds, fromTs, toTs, limit = 100) {
    let query = `
      SELECT id, kind, content, provenance, ts, scope_ids, redacted
      FROM observations
      WHERE redacted = 0
    `;
    const params = [];
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
    if (fromTs !== void 0) {
      query += ` AND ts >= ?`;
      params.push(fromTs);
    }
    if (toTs !== void 0) {
      query += ` AND ts <= ?`;
      params.push(toTs);
    }
    query += ` ORDER BY ts DESC LIMIT ?`;
    params.push(limit);
    const rows = this.db.prepare(query).all(...params);
    return rows.map((row) => ({
      id: row.id,
      kind: row.kind,
      content: row.content,
      provenance: JSON.parse(row.provenance),
      ts: row.ts,
      scopeIds: JSON.parse(row.scope_ids),
      redacted: row.redacted === 1
    }));
  }
  /**
   * Get capsule by ID
   * Alias for getCapsuleById for compatibility
   */
  getCapsule(capsuleId) {
    const query = `
      SELECT id, type, intent, status, opened_at, closed_at, scope_ids
      FROM capsules
      WHERE id = ?
    `;
    const row = this.db.prepare(query).get(capsuleId);
    if (!row)
      return void 0;
    const obsIds = this.db.prepare(`
      SELECT observation_id
      FROM capsule_observations
      WHERE capsule_id = ?
      ORDER BY seq ASC
    `).all(capsuleId);
    return {
      id: row.id,
      type: row.type,
      intent: row.intent,
      status: row.status,
      openedAt: row.opened_at,
      closedAt: row.closed_at ?? void 0,
      scopeIds: JSON.parse(row.scope_ids),
      observationIds: obsIds.map((o) => o.observation_id)
    };
  }
  /**
   * Create summary
   * Alias for insertSummary for compatibility
   */
  createSummary(summary) {
    this.insertSummary(summary);
  }
  /**
   * Create pin
   * Alias for insertPin for compatibility
   */
  createPin(pin) {
    this.insertPin(pin);
  }
  /**
   * Remove pin
   * Alias for deletePin for compatibility
   */
  removePin(pinId) {
    this.deletePin(pinId);
  }
  /**
   * Export database to dataset
   *
   * @param options - Export options
   * @returns Export dataset
   */
  exportDatabase(options) {
    return exportDatabase(this.db, options);
  }
  /**
   * Import dataset into database
   *
   * @param dataset - Dataset to import
   * @returns Import result
   */
  importDatabase(dataset) {
    return importDatabase(this.db, dataset);
  }
};

// ../../packages/kindling-provider-local/dist/provider/local-fts.js
var LocalFtsProvider = class {
  name = "local-fts";
  db;
  // Weight for FTS relevance vs recency
  FTS_WEIGHT = 0.7;
  RECENCY_WEIGHT = 0.3;
  // Max age in days for recency scoring (30 days)
  MAX_AGE_DAYS = 30;
  constructor(db) {
    this.db = db;
  }
  async search(options) {
    const { query, scopeIds, maxResults = 50, excludeIds = [], includeRedacted = false } = options;
    const ftsMatches = this.findFtsMatches(query);
    if (ftsMatches.length === 0) {
      return [];
    }
    const entities = this.fetchEntities(ftsMatches, scopeIds, excludeIds, includeRedacted);
    if (entities.length === 0) {
      return [];
    }
    const scoredResults = this.calculateScores(entities);
    scoredResults.sort((a, b) => b.score - a.score);
    return scoredResults.slice(0, maxResults);
  }
  /**
   * Find FTS matches using SQLite FTS5
   */
  findFtsMatches(query) {
    const matches = [];
    try {
      const obsStmt = this.db.prepare(`
        SELECT rowid, rank
        FROM observations_fts
        WHERE content MATCH ?
        ORDER BY rank
      `);
      const obsMatches = obsStmt.all(query);
      matches.push(...obsMatches.map((m) => ({
        rowid: m.rowid,
        table_name: "observations",
        rank: m.rank
      })));
    } catch (err2) {
      if (!this.isFtsSyntaxError(err2))
        throw err2;
    }
    try {
      const sumStmt = this.db.prepare(`
        SELECT rowid, rank
        FROM summaries_fts
        WHERE content MATCH ?
        ORDER BY rank
      `);
      const sumMatches = sumStmt.all(query);
      matches.push(...sumMatches.map((m) => ({
        rowid: m.rowid,
        table_name: "summaries",
        rank: m.rank
      })));
    } catch (err2) {
      if (!this.isFtsSyntaxError(err2))
        throw err2;
    }
    return matches;
  }
  /**
   * Fetch entities and apply scope/redaction/exclusion filters
   */
  fetchEntities(matches, scopeIds, excludeIds, includeRedacted) {
    const results = [];
    const obsMatches = matches.filter((m) => m.table_name === "observations");
    if (obsMatches.length > 0) {
      const obsRowids = obsMatches.map((m) => m.rowid);
      const placeholders = obsRowids.map(() => "?").join(",");
      const scopeFilter = this.buildScopeFilters(scopeIds);
      let obsQuery = `
        SELECT o.rowid, o.id, o.kind, o.content, o.provenance, o.ts, o.scope_ids, o.redacted
        FROM observations o
        WHERE o.rowid IN (${placeholders})
      `;
      if (!includeRedacted) {
        obsQuery += ` AND o.redacted = 0`;
      }
      if (scopeFilter.clauses.length > 0) {
        obsQuery += ` AND (${scopeFilter.clauses.join(" AND ")})`;
      }
      const obsStmt = this.db.prepare(obsQuery);
      const observations = obsStmt.all(...obsRowids, ...scopeFilter.params);
      for (const row of observations) {
        if (excludeIds.includes(row.id))
          continue;
        const observation = {
          id: row.id,
          kind: row.kind,
          content: row.content,
          provenance: JSON.parse(row.provenance),
          ts: row.ts,
          scopeIds: JSON.parse(row.scope_ids),
          redacted: row.redacted === 1
        };
        const ftsMatch = obsMatches.find((m) => m.rowid === row.rowid);
        if (ftsMatch) {
          results.push({ entity: observation, ftsMatch });
        }
      }
    }
    const sumMatches = matches.filter((m) => m.table_name === "summaries");
    if (sumMatches.length > 0) {
      const sumRowids = sumMatches.map((m) => m.rowid);
      const placeholders = sumRowids.map(() => "?").join(",");
      const sumScopeFilter = this.buildScopeFilters(scopeIds, "c");
      let sumQuery = `
        SELECT s.rowid, s.id, s.capsule_id, s.content, s.confidence, s.evidence_refs, s.created_at
        FROM summaries s
        INNER JOIN capsules c ON s.capsule_id = c.id
        WHERE s.rowid IN (${placeholders})
      `;
      if (sumScopeFilter.clauses.length > 0) {
        sumQuery += ` AND (${sumScopeFilter.clauses.join(" AND ")})`;
      }
      const sumStmt = this.db.prepare(sumQuery);
      const summaries = sumStmt.all(...sumRowids, ...sumScopeFilter.params);
      for (const row of summaries) {
        if (excludeIds.includes(row.id))
          continue;
        const summary = {
          id: row.id,
          capsuleId: row.capsule_id,
          content: row.content,
          confidence: row.confidence,
          evidenceRefs: JSON.parse(row.evidence_refs),
          createdAt: row.created_at
        };
        const ftsMatch = sumMatches.find((m) => m.rowid === row.rowid);
        if (ftsMatch) {
          results.push({ entity: summary, ftsMatch });
        }
      }
    }
    return results;
  }
  /**
   * Build scope filter SQL clauses with parameterized queries
   */
  buildScopeFilters(scopeIds, tablePrefix = "") {
    const clauses = [];
    const params = [];
    const col = tablePrefix ? `${tablePrefix}.scope_ids` : "scope_ids";
    if (scopeIds.sessionId !== void 0) {
      clauses.push(`json_extract(${col}, '$.sessionId') = ?`);
      params.push(scopeIds.sessionId);
    }
    if (scopeIds.repoId !== void 0) {
      clauses.push(`json_extract(${col}, '$.repoId') = ?`);
      params.push(scopeIds.repoId);
    }
    if (scopeIds.agentId !== void 0) {
      clauses.push(`json_extract(${col}, '$.agentId') = ?`);
      params.push(scopeIds.agentId);
    }
    if (scopeIds.userId !== void 0) {
      clauses.push(`json_extract(${col}, '$.userId') = ?`);
      params.push(scopeIds.userId);
    }
    return { clauses, params };
  }
  /**
   * Calculate combined score: FTS relevance + recency
   */
  calculateScores(entities) {
    const ftsRanks = entities.map((e) => e.ftsMatch.rank);
    const minRank = Math.min(...ftsRanks);
    const maxRank = Math.max(...ftsRanks);
    const rankRange = maxRank - minRank;
    const now = Date.now();
    return entities.map(({ entity, ftsMatch }) => {
      const ftsRelevance = rankRange > 0 ? (maxRank - ftsMatch.rank) / rankRange : 1;
      const entityTs = this.getTimestamp(entity);
      const ageDays = (now - entityTs) / (1e3 * 60 * 60 * 24);
      const recencyScore = Math.max(0, 1 - ageDays / this.MAX_AGE_DAYS);
      const score = ftsRelevance * this.FTS_WEIGHT + recencyScore * this.RECENCY_WEIGHT;
      const matchContext = this.extractMatchContext(entity);
      const clampedScore = Math.min(1, Math.max(0, score));
      const roundedScore = Math.round(clampedScore * 1e10) / 1e10;
      return {
        entity,
        score: roundedScore,
        matchContext
      };
    });
  }
  /**
   * Get timestamp from entity (observations have ts, summaries have createdAt)
   */
  getTimestamp(entity) {
    if ("ts" in entity) {
      return entity.ts;
    } else {
      return entity.createdAt;
    }
  }
  /**
   * Check if an error is an FTS5 query syntax error (safe to swallow).
   * Covers all known SQLite/FTS5 error messages for malformed MATCH input.
   * All other database errors are propagated.
   */
  isFtsSyntaxError(err2) {
    if (err2 instanceof Error) {
      const msg = err2.message.toLowerCase();
      return msg.includes("fts5") || msg.includes("fts syntax") || msg.includes("unterminated string") || msg.includes("unknown special query");
    }
    return false;
  }
  /**
   * Extract snippet showing match context
   */
  extractMatchContext(entity) {
    const content = entity.content;
    const maxLength = 100;
    if (content.length <= maxLength) {
      return content;
    }
    return content.substring(0, maxLength) + "...";
  }
};

// ../../packages/kindling-core/dist/types/common.js
function ok(value) {
  return { ok: true, value };
}
function err(error) {
  return { ok: false, error };
}

// ../../packages/kindling-core/dist/types/observation.js
var OBSERVATION_KINDS = [
  "tool_call",
  "command",
  "file_diff",
  "error",
  "message",
  "node_start",
  "node_end",
  "node_output",
  "node_error"
];
function isObservationKind(value) {
  return typeof value === "string" && OBSERVATION_KINDS.includes(value);
}

// ../../packages/kindling-core/dist/types/capsule.js
var CAPSULE_TYPES = [
  "session",
  "pocketflow_node"
];
var CAPSULE_STATUSES = [
  "open",
  "closed"
];
function isCapsuleType(value) {
  return typeof value === "string" && CAPSULE_TYPES.includes(value);
}
function isCapsuleStatus(value) {
  return typeof value === "string" && CAPSULE_STATUSES.includes(value);
}

// ../../packages/kindling-core/dist/types/summary.js
function isValidConfidence(value) {
  return typeof value === "number" && value >= 0 && value <= 1 && !isNaN(value);
}

// ../../packages/kindling-core/dist/types/pin.js
var PIN_TARGET_TYPES = [
  "observation",
  "summary"
];
function isPinTargetType(value) {
  return typeof value === "string" && PIN_TARGET_TYPES.includes(value);
}

// ../../packages/kindling-core/dist/validation/observation.js
var import_crypto = require("crypto");
function validateObservation(input) {
  const errors = [];
  if (typeof input !== "object" || input === null) {
    return err([{ field: "input", message: "Input must be an object" }]);
  }
  const data = input;
  if (!data.kind) {
    errors.push({ field: "kind", message: "kind is required" });
  } else if (!isObservationKind(data.kind)) {
    errors.push({
      field: "kind",
      message: `Invalid observation kind: ${data.kind}`,
      value: data.kind
    });
  }
  if (!data.content) {
    errors.push({ field: "content", message: "content is required" });
  } else if (typeof data.content !== "string") {
    errors.push({
      field: "content",
      message: "content must be a string",
      value: typeof data.content
    });
  } else if (data.content.trim().length === 0) {
    errors.push({ field: "content", message: "content cannot be empty" });
  }
  if (!data.scopeIds) {
    errors.push({ field: "scopeIds", message: "scopeIds is required" });
  } else if (typeof data.scopeIds !== "object" || data.scopeIds === null) {
    errors.push({
      field: "scopeIds",
      message: "scopeIds must be an object"
    });
  }
  if (data.provenance !== void 0) {
    if (typeof data.provenance !== "object" || data.provenance === null || Array.isArray(data.provenance)) {
      errors.push({
        field: "provenance",
        message: "provenance must be an object"
      });
    }
  }
  if (data.ts !== void 0) {
    if (typeof data.ts !== "number") {
      errors.push({
        field: "ts",
        message: "ts must be a number",
        value: typeof data.ts
      });
    } else if (data.ts < 0) {
      errors.push({
        field: "ts",
        message: "ts must be non-negative",
        value: data.ts
      });
    }
  }
  if (data.redacted !== void 0 && typeof data.redacted !== "boolean") {
    errors.push({
      field: "redacted",
      message: "redacted must be a boolean",
      value: typeof data.redacted
    });
  }
  if (errors.length > 0) {
    return err(errors);
  }
  const observation = {
    id: data.id || (0, import_crypto.randomUUID)(),
    kind: data.kind,
    content: data.content,
    provenance: data.provenance || {},
    ts: data.ts || Date.now(),
    scopeIds: data.scopeIds,
    redacted: data.redacted || false
  };
  return ok(observation);
}

// ../../packages/kindling-core/dist/validation/capsule.js
var import_crypto2 = require("crypto");
function validateCapsule(input) {
  const errors = [];
  if (typeof input !== "object" || input === null) {
    return err([{ field: "input", message: "Input must be an object" }]);
  }
  const data = input;
  if (!data.type) {
    errors.push({ field: "type", message: "type is required" });
  } else if (!isCapsuleType(data.type)) {
    errors.push({
      field: "type",
      message: `Invalid capsule type: ${data.type}`,
      value: data.type
    });
  }
  if (!data.intent) {
    errors.push({ field: "intent", message: "intent is required" });
  } else if (typeof data.intent !== "string") {
    errors.push({
      field: "intent",
      message: "intent must be a string",
      value: typeof data.intent
    });
  } else if (data.intent.trim().length === 0) {
    errors.push({ field: "intent", message: "intent cannot be empty" });
  }
  if (!data.scopeIds) {
    errors.push({ field: "scopeIds", message: "scopeIds is required" });
  } else if (typeof data.scopeIds !== "object" || data.scopeIds === null) {
    errors.push({
      field: "scopeIds",
      message: "scopeIds must be an object"
    });
  }
  if (data.status !== void 0 && !isCapsuleStatus(data.status)) {
    errors.push({
      field: "status",
      message: `Invalid capsule status: ${data.status}`,
      value: data.status
    });
  }
  if (data.openedAt !== void 0) {
    if (typeof data.openedAt !== "number") {
      errors.push({
        field: "openedAt",
        message: "openedAt must be a number",
        value: typeof data.openedAt
      });
    } else if (data.openedAt < 0) {
      errors.push({
        field: "openedAt",
        message: "openedAt must be non-negative",
        value: data.openedAt
      });
    }
  }
  if (data.closedAt !== void 0) {
    if (typeof data.closedAt !== "number") {
      errors.push({
        field: "closedAt",
        message: "closedAt must be a number",
        value: typeof data.closedAt
      });
    } else if (data.closedAt < 0) {
      errors.push({
        field: "closedAt",
        message: "closedAt must be non-negative",
        value: data.closedAt
      });
    }
  }
  if (data.observationIds !== void 0) {
    if (!Array.isArray(data.observationIds)) {
      errors.push({
        field: "observationIds",
        message: "observationIds must be an array"
      });
    } else if (!data.observationIds.every((id) => typeof id === "string")) {
      errors.push({
        field: "observationIds",
        message: "observationIds must contain only strings"
      });
    }
  }
  if (data.summaryId !== void 0 && typeof data.summaryId !== "string") {
    errors.push({
      field: "summaryId",
      message: "summaryId must be a string",
      value: typeof data.summaryId
    });
  }
  if (errors.length > 0) {
    return err(errors);
  }
  const capsule = {
    id: data.id || (0, import_crypto2.randomUUID)(),
    type: data.type,
    intent: data.intent,
    status: data.status || "open",
    openedAt: data.openedAt || Date.now(),
    closedAt: data.closedAt,
    scopeIds: data.scopeIds,
    observationIds: data.observationIds || [],
    summaryId: data.summaryId
  };
  return ok(capsule);
}

// ../../packages/kindling-core/dist/validation/summary.js
var import_crypto3 = require("crypto");
function validateSummary(input) {
  const errors = [];
  if (typeof input !== "object" || input === null) {
    return err([{ field: "input", message: "Input must be an object" }]);
  }
  const data = input;
  if (!data.capsuleId) {
    errors.push({ field: "capsuleId", message: "capsuleId is required" });
  } else if (typeof data.capsuleId !== "string") {
    errors.push({
      field: "capsuleId",
      message: "capsuleId must be a string",
      value: typeof data.capsuleId
    });
  } else if (data.capsuleId.trim().length === 0) {
    errors.push({ field: "capsuleId", message: "capsuleId cannot be empty" });
  }
  if (!data.content) {
    errors.push({ field: "content", message: "content is required" });
  } else if (typeof data.content !== "string") {
    errors.push({
      field: "content",
      message: "content must be a string",
      value: typeof data.content
    });
  } else if (data.content.trim().length === 0) {
    errors.push({ field: "content", message: "content cannot be empty" });
  }
  if (data.confidence === void 0 || data.confidence === null) {
    errors.push({ field: "confidence", message: "confidence is required" });
  } else if (typeof data.confidence !== "number") {
    errors.push({
      field: "confidence",
      message: "confidence must be a number",
      value: typeof data.confidence
    });
  } else if (!isValidConfidence(data.confidence)) {
    errors.push({
      field: "confidence",
      message: "confidence must be between 0.0 and 1.0",
      value: data.confidence
    });
  }
  if (!data.evidenceRefs) {
    errors.push({ field: "evidenceRefs", message: "evidenceRefs is required" });
  } else if (!Array.isArray(data.evidenceRefs)) {
    errors.push({
      field: "evidenceRefs",
      message: "evidenceRefs must be an array"
    });
  } else if (!data.evidenceRefs.every((id) => typeof id === "string")) {
    errors.push({
      field: "evidenceRefs",
      message: "evidenceRefs must contain only strings"
    });
  }
  if (data.createdAt !== void 0) {
    if (typeof data.createdAt !== "number") {
      errors.push({
        field: "createdAt",
        message: "createdAt must be a number",
        value: typeof data.createdAt
      });
    } else if (data.createdAt < 0) {
      errors.push({
        field: "createdAt",
        message: "createdAt must be non-negative",
        value: data.createdAt
      });
    }
  }
  if (errors.length > 0) {
    return err(errors);
  }
  const summary = {
    id: data.id || (0, import_crypto3.randomUUID)(),
    capsuleId: data.capsuleId,
    content: data.content,
    confidence: data.confidence,
    createdAt: data.createdAt || Date.now(),
    evidenceRefs: data.evidenceRefs
  };
  return ok(summary);
}

// ../../packages/kindling-core/dist/validation/pin.js
var import_crypto4 = require("crypto");
function validatePin(input) {
  const errors = [];
  if (typeof input !== "object" || input === null) {
    return err([{ field: "input", message: "Input must be an object" }]);
  }
  const data = input;
  if (!data.targetType) {
    errors.push({ field: "targetType", message: "targetType is required" });
  } else if (!isPinTargetType(data.targetType)) {
    errors.push({
      field: "targetType",
      message: `Invalid pin target type: ${data.targetType}`,
      value: data.targetType
    });
  }
  if (!data.targetId) {
    errors.push({ field: "targetId", message: "targetId is required" });
  } else if (typeof data.targetId !== "string") {
    errors.push({
      field: "targetId",
      message: "targetId must be a string",
      value: typeof data.targetId
    });
  } else if (data.targetId.trim().length === 0) {
    errors.push({ field: "targetId", message: "targetId cannot be empty" });
  }
  if (!data.scopeIds) {
    errors.push({ field: "scopeIds", message: "scopeIds is required" });
  } else if (typeof data.scopeIds !== "object" || data.scopeIds === null) {
    errors.push({
      field: "scopeIds",
      message: "scopeIds must be an object"
    });
  }
  if (data.reason !== void 0 && typeof data.reason !== "string") {
    errors.push({
      field: "reason",
      message: "reason must be a string",
      value: typeof data.reason
    });
  }
  if (data.createdAt !== void 0) {
    if (typeof data.createdAt !== "number") {
      errors.push({
        field: "createdAt",
        message: "createdAt must be a number",
        value: typeof data.createdAt
      });
    } else if (data.createdAt < 0) {
      errors.push({
        field: "createdAt",
        message: "createdAt must be non-negative",
        value: data.createdAt
      });
    }
  }
  if (data.expiresAt !== void 0) {
    if (typeof data.expiresAt !== "number") {
      errors.push({
        field: "expiresAt",
        message: "expiresAt must be a number",
        value: typeof data.expiresAt
      });
    } else if (data.expiresAt < 0) {
      errors.push({
        field: "expiresAt",
        message: "expiresAt must be non-negative",
        value: data.expiresAt
      });
    }
  }
  if (errors.length > 0) {
    return err(errors);
  }
  const pin = {
    id: data.id || (0, import_crypto4.randomUUID)(),
    targetType: data.targetType,
    targetId: data.targetId,
    reason: data.reason,
    createdAt: data.createdAt || Date.now(),
    expiresAt: data.expiresAt,
    scopeIds: data.scopeIds
  };
  return ok(pin);
}

// ../../packages/kindling-core/dist/capsule/lifecycle.js
function openCapsule(store, options) {
  const { type, intent, scopeIds, id } = options;
  if (type === "session" && scopeIds.sessionId) {
    const existingCapsule = store.getOpenCapsuleForSession(scopeIds.sessionId);
    if (existingCapsule) {
      throw new Error(`Cannot open capsule: session ${scopeIds.sessionId} already has an open capsule (${existingCapsule.id})`);
    }
  }
  const capsuleResult = validateCapsule({
    id,
    type,
    intent,
    status: "open",
    scopeIds
  });
  if (!capsuleResult.ok) {
    throw new Error(`Capsule validation failed: ${capsuleResult.error.map((e) => e.message).join(", ")}`);
  }
  const capsule = capsuleResult.value;
  store.createCapsule(capsule);
  return capsule;
}
function closeCapsule(store, capsuleId, signals = {}) {
  const capsule = store.getCapsuleById(capsuleId);
  if (!capsule) {
    throw new Error(`Capsule ${capsuleId} not found`);
  }
  if (capsule.status === "closed") {
    throw new Error(`Capsule ${capsuleId} is already closed`);
  }
  const closedAt = Date.now();
  store.closeCapsule(capsuleId, closedAt);
  if (signals.summaryContent) {
    const summaryResult = validateSummary({
      capsuleId,
      content: signals.summaryContent,
      confidence: signals.summaryConfidence ?? 0.8,
      evidenceRefs: signals.evidenceRefs ?? []
    });
    if (!summaryResult.ok) {
      throw new Error(`Summary validation failed: ${summaryResult.error.map((e) => e.message).join(", ")}`);
    }
    store.insertSummary(summaryResult.value);
  }
  return {
    ...capsule,
    status: "closed",
    closedAt
  };
}
function getCapsule(store, capsuleId) {
  return store.getCapsuleById(capsuleId);
}
function getOpenCapsule(store, sessionId) {
  return store.getOpenCapsuleForSession(sessionId);
}

// ../../packages/kindling-core/dist/capsule/manager.js
var CapsuleManager = class {
  store;
  activeCache;
  constructor(store) {
    this.store = store;
    this.activeCache = /* @__PURE__ */ new Map();
  }
  /**
   * Open a new capsule
   *
   * @param options - Capsule creation options
   * @returns The created capsule
   * @throws Error if validation fails or duplicate open capsule exists
   */
  open(options) {
    const capsule = openCapsule(this.store, options);
    this.activeCache.set(capsule.id, capsule);
    return capsule;
  }
  /**
   * Close an open capsule
   *
   * @param capsuleId - ID of capsule to close
   * @param signals - Closure signals/metadata
   * @returns The closed capsule
   * @throws Error if capsule not found or already closed
   */
  close(capsuleId, signals) {
    const capsule = closeCapsule(this.store, capsuleId, signals);
    this.activeCache.delete(capsuleId);
    return capsule;
  }
  /**
   * Get a capsule by ID
   *
   * Checks cache first, falls back to store.
   *
   * @param capsuleId - Capsule ID to lookup
   * @returns Capsule or undefined if not found
   */
  get(capsuleId) {
    const cached = this.activeCache.get(capsuleId);
    if (cached) {
      return cached;
    }
    return getCapsule(this.store, capsuleId);
  }
  /**
   * Get the open capsule for a scope (if any)
   *
   * Currently only supports session-scoped lookup.
   *
   * @param scopeIds - Partial scope to match
   * @returns Open capsule or undefined
   */
  getOpen(scopeIds) {
    if (!scopeIds.sessionId) {
      throw new Error("getOpen currently only supports sessionId lookup");
    }
    return getOpenCapsule(this.store, scopeIds.sessionId);
  }
  /**
   * Notify that an observation was attached to a capsule
   *
   * Updates the cached capsule's observationIds if the capsule is in the cache.
   *
   * @param capsuleId - Capsule that received the observation
   * @param observationId - Observation that was attached
   */
  notifyObservationAttached(capsuleId, observationId) {
    const cached = this.activeCache.get(capsuleId);
    if (cached) {
      cached.observationIds.push(observationId);
    }
  }
  /**
   * Clear the active capsule cache
   *
   * Useful for testing or manual cache invalidation.
   */
  clearCache() {
    this.activeCache.clear();
  }
  /**
   * Get count of cached active capsules
   *
   * Useful for debugging and monitoring.
   */
  getCacheSize() {
    return this.activeCache.size;
  }
};

// ../../packages/kindling-core/dist/retrieval/orchestrator.js
async function retrieve(store, provider, options) {
  const { query, scopeIds, maxCandidates = 10, includeRedacted = false } = options;
  const now = Date.now();
  const pins = store.listActivePins(scopeIds, now);
  const pinResults = [];
  const pinnedIds = /* @__PURE__ */ new Set();
  for (const pin of pins) {
    let target;
    if (pin.targetType === "observation") {
      target = store.getObservationById(pin.targetId);
    } else if (pin.targetType === "summary") {
      target = store.getSummaryById(pin.targetId);
    }
    if (target) {
      if ("redacted" in target && target.redacted && !includeRedacted) {
        continue;
      }
      pinResults.push({ pin, target });
      pinnedIds.add(target.id);
    }
  }
  let currentSummary;
  if (scopeIds.sessionId) {
    const capsule = store.getOpenCapsuleForSession(scopeIds.sessionId);
    if (capsule) {
      const summary = store.getLatestSummaryForCapsule(capsule.id);
      if (summary) {
        currentSummary = summary;
        pinnedIds.add(summary.id);
      }
    }
  }
  const providerResults = await provider.search({
    query,
    scopeIds,
    maxResults: maxCandidates,
    excludeIds: Array.from(pinnedIds),
    includeRedacted
  });
  const candidates = providerResults.map((result) => ({
    entity: result.entity,
    score: result.score,
    matchContext: result.matchContext
  }));
  const provenance = {
    query,
    scopeIds,
    totalCandidates: providerResults.length,
    returnedCandidates: candidates.length,
    truncatedDueToTokenBudget: false,
    // Token budgeting not implemented in v0.1
    providerUsed: provider.name
  };
  return {
    pins: pinResults,
    currentSummary,
    candidates,
    provenance
  };
}

// ../../packages/kindling-core/dist/retrieval/tiering.js
var Tier;
(function(Tier2) {
  Tier2[Tier2["PINNED"] = 0] = "PINNED";
  Tier2[Tier2["CANDIDATE"] = 1] = "CANDIDATE";
})(Tier || (Tier = {}));

// ../../packages/kindling-core/dist/export/bundle.js
function createExportBundle(store, options = {}) {
  const { scope, includeRedacted = false, limit, metadata } = options;
  const dataset = store.exportDatabase({
    scope,
    includeRedacted,
    limit
  });
  const bundle = {
    bundleVersion: "1.0",
    exportedAt: Date.now(),
    dataset
  };
  if (metadata) {
    bundle.metadata = metadata;
  }
  return bundle;
}
function getBundleStats(bundle) {
  const { dataset } = bundle;
  const totalSize = JSON.stringify(bundle).length;
  return {
    observations: dataset.observations.length,
    capsules: dataset.capsules.length,
    summaries: dataset.summaries.length,
    pins: dataset.pins.length,
    totalSize
  };
}
function validateBundle(bundle) {
  const errors = [];
  if (!bundle || typeof bundle !== "object") {
    return { valid: false, errors: ["Bundle must be an object"] };
  }
  const b = bundle;
  if (!b.bundleVersion || typeof b.bundleVersion !== "string") {
    errors.push("Missing or invalid bundleVersion");
  } else if (b.bundleVersion !== "1.0") {
    errors.push(`Unsupported bundle version: ${b.bundleVersion}`);
  }
  if (!b.exportedAt || typeof b.exportedAt !== "number") {
    errors.push("Missing or invalid exportedAt");
  }
  if (!b.dataset || typeof b.dataset !== "object") {
    errors.push("Missing or invalid dataset");
  } else {
    const ds = b.dataset;
    if (!ds.version || typeof ds.version !== "string") {
      errors.push("Missing or invalid dataset.version");
    }
    if (!Array.isArray(ds.observations)) {
      errors.push("dataset.observations must be an array");
    }
    if (!Array.isArray(ds.capsules)) {
      errors.push("dataset.capsules must be an array");
    }
    if (!Array.isArray(ds.summaries)) {
      errors.push("dataset.summaries must be an array");
    }
    if (!Array.isArray(ds.pins)) {
      errors.push("dataset.pins must be an array");
    }
  }
  return {
    valid: errors.length === 0,
    errors
  };
}
function serializeBundle(bundle, pretty = false) {
  return JSON.stringify(bundle, null, pretty ? 2 : 0);
}
function deserializeBundle(json) {
  try {
    const bundle = JSON.parse(json);
    const validation = validateBundle(bundle);
    if (!validation.valid) {
      throw new Error(`Invalid bundle: ${validation.errors.join(", ")}`);
    }
    return bundle;
  } catch (err2) {
    if (err2 instanceof SyntaxError) {
      throw new Error(`Invalid JSON: ${err2.message}`);
    }
    throw err2;
  }
}

// ../../packages/kindling-core/dist/export/restore.js
function restoreFromBundle(store, bundle, options = {}) {
  const { skipValidation = false, dryRun = false } = options;
  if (!skipValidation) {
    const validation = validateBundle(bundle);
    if (!validation.valid) {
      return {
        observations: 0,
        capsules: 0,
        summaries: 0,
        pins: 0,
        errors: validation.errors,
        dryRun
      };
    }
  }
  if (dryRun) {
    return {
      observations: bundle.dataset.observations.length,
      capsules: bundle.dataset.capsules.length,
      summaries: bundle.dataset.summaries.length,
      pins: bundle.dataset.pins.length,
      errors: [],
      dryRun: true
    };
  }
  const result = store.importDatabase(bundle.dataset);
  return {
    ...result,
    dryRun: false
  };
}
function mergeBundles(bundles, metadata) {
  if (bundles.length === 0) {
    throw new Error("At least one bundle required for merge");
  }
  for (const bundle of bundles) {
    const validation = validateBundle(bundle);
    if (!validation.valid) {
      throw new Error(`Invalid bundle: ${validation.errors.join(", ")}`);
    }
  }
  const merged = {
    observations: [],
    capsules: [],
    summaries: [],
    pins: []
  };
  for (const bundle of bundles) {
    merged.observations.push(...bundle.dataset.observations);
    merged.capsules.push(...bundle.dataset.capsules);
    merged.summaries.push(...bundle.dataset.summaries);
    merged.pins.push(...bundle.dataset.pins);
  }
  const deduped = {
    observations: deduplicateById(merged.observations),
    capsules: deduplicateById(merged.capsules),
    summaries: deduplicateById(merged.summaries),
    pins: deduplicateById(merged.pins)
  };
  return {
    bundleVersion: "1.0",
    exportedAt: Date.now(),
    metadata: metadata || {
      description: `Merged from ${bundles.length} bundles`
    },
    dataset: {
      version: "1.0",
      exportedAt: Date.now(),
      ...deduped
    }
  };
}
function deduplicateById(entities) {
  const seen = /* @__PURE__ */ new Set();
  const deduped = [];
  for (const entity of entities) {
    if (!entity.id)
      continue;
    if (!seen.has(entity.id)) {
      seen.add(entity.id);
      deduped.push(entity);
    }
  }
  return deduped;
}

// ../../packages/kindling-core/dist/service/kindling-service.js
var KindlingService = class {
  store;
  provider;
  capsuleManager;
  constructor(config) {
    this.store = config.store;
    this.provider = config.provider;
    this.capsuleManager = new CapsuleManager({
      createCapsule: (capsule) => this.store.createCapsule(capsule),
      closeCapsule: (capsuleId, closedAt) => this.store.closeCapsule(capsuleId, closedAt),
      getCapsuleById: (capsuleId) => this.store.getCapsule(capsuleId),
      getOpenCapsuleForSession: (sessionId) => this.store.getOpenCapsuleForSession(sessionId),
      insertSummary: (summary) => this.store.createSummary(summary)
    });
  }
  /**
   * Open a new capsule
   *
   * @param options - Capsule creation options
   * @returns The created capsule
   */
  openCapsule(options) {
    return this.capsuleManager.open(options);
  }
  /**
   * Close a capsule
   *
   * @param capsuleId - ID of capsule to close
   * @param options - Closure options
   * @returns The closed capsule
   */
  closeCapsule(capsuleId, options) {
    const signals = {};
    if (options?.generateSummary && options.summaryContent) {
      const summary = {
        id: `sum_${crypto.randomUUID()}`,
        capsuleId,
        content: options.summaryContent,
        confidence: options.confidence ?? 1,
        createdAt: Date.now(),
        evidenceRefs: []
      };
      const validation = validateSummary(summary);
      if (!validation.ok) {
        const errorMessages = validation.error.map((e) => e.message).join(", ");
        throw new Error(`Invalid summary: ${errorMessages}`);
      }
      this.store.createSummary(validation.value);
    }
    return this.capsuleManager.close(capsuleId, signals);
  }
  /**
   * Append an observation
   *
   * @param observation - Observation to append
   * @param options - Append options
   */
  appendObservation(observation, options) {
    let obsToStore = observation;
    if (options?.validate !== false) {
      const validation = validateObservation(observation);
      if (!validation.ok) {
        const errorMessages = validation.error.map((e) => e.message).join(", ");
        throw new Error(`Invalid observation: ${errorMessages}`);
      }
      obsToStore = validation.value;
    }
    this.store.insertObservation(obsToStore);
    if (options?.capsuleId) {
      this.store.attachObservationToCapsule(options.capsuleId, obsToStore.id);
      this.capsuleManager.notifyObservationAttached(options.capsuleId, obsToStore.id);
    }
  }
  /**
   * Retrieve relevant context
   *
   * @param options - Retrieval options
   * @returns Retrieval result with pins, summary, and candidates
   */
  async retrieve(options) {
    return retrieve(this.store, this.provider, options);
  }
  /**
   * Create a pin
   *
   * @param options - Pin creation options
   * @returns The created pin
   */
  pin(options) {
    const pin = {
      id: `pin_${crypto.randomUUID()}`,
      targetType: options.targetType,
      targetId: options.targetId,
      reason: options.note,
      createdAt: Date.now(),
      expiresAt: options.ttlMs ? Date.now() + options.ttlMs : void 0,
      scopeIds: options.scopeIds ?? {}
    };
    const validation = validatePin(pin);
    if (!validation.ok) {
      const errorMessages = validation.error.map((e) => e.message).join(", ");
      throw new Error(`Invalid pin: ${errorMessages}`);
    }
    this.store.createPin(validation.value);
    return validation.value;
  }
  /**
   * Remove a pin
   *
   * @param pinId - ID of pin to remove
   */
  unpin(pinId) {
    this.store.removePin(pinId);
  }
  /**
   * Redact an observation
   *
   * Removes content but preserves structure for provenance.
   *
   * @param observationId - ID of observation to redact
   */
  forget(observationId) {
    this.store.redactObservation(observationId);
  }
  /**
   * Get a capsule by ID
   *
   * @param capsuleId - Capsule ID
   * @returns Capsule or undefined if not found
   */
  getCapsule(capsuleId) {
    return this.capsuleManager.get(capsuleId);
  }
  /**
   * Get open capsule for a session
   *
   * @param sessionId - Session ID
   * @returns Open capsule or undefined
   */
  getOpenCapsule(sessionId) {
    return this.capsuleManager.getOpen({ sessionId });
  }
  /**
   * Get observation by ID
   *
   * @param observationId - Observation ID
   * @returns Observation or undefined if not found
   */
  getObservation(observationId) {
    return this.store.getObservationById(observationId);
  }
  /**
   * Get summary by ID
   *
   * @param summaryId - Summary ID
   * @returns Summary or undefined if not found
   */
  getSummary(summaryId) {
    return this.store.getSummaryById(summaryId);
  }
  /**
   * List active pins for a scope
   *
   * @param scopeIds - Scope to filter by
   * @returns Array of active pins
   */
  listPins(scopeIds) {
    return this.store.listActivePins(scopeIds, Date.now());
  }
  /**
   * Export database to a portable bundle
   *
   * @param options - Export options
   * @returns Export bundle with dataset and metadata
   */
  export(options) {
    return createExportBundle(this.store, options);
  }
  /**
   * Export database to JSON string
   *
   * @param options - Export options
   * @param pretty - Use pretty formatting (default: false)
   * @returns JSON string
   */
  exportToJson(options, pretty = false) {
    const bundle = this.export(options);
    return serializeBundle(bundle, pretty);
  }
  /**
   * Import from export bundle
   *
   * @param bundle - Export bundle to import
   * @param options - Import options
   * @returns Import result with counts and errors
   */
  import(bundle, options) {
    return restoreFromBundle(this.store, bundle, options);
  }
  /**
   * Import from JSON string
   *
   * @param json - JSON string containing export bundle
   * @param options - Import options
   * @returns Import result with counts and errors
   */
  importFromJson(json, options) {
    const bundle = deserializeBundle(json);
    return this.import(bundle, options);
  }
  /**
   * Get statistics for an export bundle
   *
   * @param bundle - Export bundle
   * @returns Statistics about bundle contents
   */
  getBundleStats(bundle) {
    return getBundleStats(bundle);
  }
  /**
   * Merge multiple export bundles
   *
   * @param bundles - Bundles to merge
   * @param metadata - Optional metadata for merged bundle
   * @returns Merged bundle
   */
  mergeBundles(bundles, metadata) {
    return mergeBundles(bundles, metadata);
  }
};

// ../../packages/kindling-adapter-claude-code/dist/claude-code/events.js
function createPostToolUseEvent(ctx) {
  return {
    type: "post_tool_use",
    timestamp: Date.now(),
    sessionId: ctx.sessionId,
    cwd: ctx.cwd,
    toolName: ctx.toolName,
    toolInput: ctx.toolInput,
    toolResult: ctx.toolResult,
    toolError: ctx.toolError
  };
}
function createStopEvent(ctx) {
  return {
    type: "stop",
    timestamp: Date.now(),
    sessionId: ctx.sessionId,
    cwd: ctx.cwd,
    stopReason: ctx.reason
  };
}
function createSubagentStopEvent(ctx) {
  return {
    type: "subagent_stop",
    timestamp: Date.now(),
    sessionId: ctx.sessionId,
    cwd: ctx.cwd,
    agentType: ctx.agentType,
    agentOutput: ctx.output
  };
}
function createUserPromptEvent(ctx) {
  return {
    type: "user_prompt",
    timestamp: Date.now(),
    sessionId: ctx.sessionId,
    cwd: ctx.cwd,
    userContent: ctx.content
  };
}

// ../../packages/kindling-adapter-claude-code/dist/claude-code/provenance.js
function extractToolUseProvenance(event) {
  if (event.type !== "post_tool_use") {
    return {};
  }
  const provenance = {
    toolName: event.toolName,
    hasError: !!event.toolError
  };
  switch (event.toolName) {
    case "Read":
      provenance.filePath = event.toolInput?.file_path;
      break;
    case "Write":
      provenance.filePath = event.toolInput?.file_path;
      break;
    case "Edit":
      provenance.filePath = event.toolInput?.file_path;
      provenance.hasOldString = !!event.toolInput?.old_string;
      break;
    case "Bash":
      provenance.command = extractCommandName(event.toolInput?.command);
      provenance.exitCode = extractExitCode(event.toolResult);
      break;
    case "Glob":
      provenance.pattern = event.toolInput?.pattern;
      provenance.path = event.toolInput?.path;
      break;
    case "Grep":
      provenance.pattern = event.toolInput?.pattern;
      provenance.path = event.toolInput?.path;
      break;
    case "Task":
      provenance.subagentType = event.toolInput?.subagent_type;
      provenance.description = event.toolInput?.description;
      break;
    case "WebFetch":
      provenance.url = event.toolInput?.url;
      break;
    case "WebSearch":
      provenance.query = event.toolInput?.query;
      break;
    default:
      if (event.toolInput) {
        provenance.inputKeys = Object.keys(event.toolInput);
      }
  }
  return provenance;
}
function extractUserPromptProvenance(event) {
  if (event.type !== "user_prompt") {
    return {};
  }
  return {
    role: "user",
    length: event.userContent?.length ?? 0
  };
}
function extractSubagentProvenance(event) {
  if (event.type !== "subagent_stop") {
    return {};
  }
  return {
    agentType: event.agentType,
    hasOutput: !!event.agentOutput,
    outputLength: event.agentOutput?.length ?? 0
  };
}
function extractStopProvenance(event) {
  if (event.type !== "stop") {
    return {};
  }
  return {
    reason: event.stopReason ?? "unknown"
  };
}
function extractCommandName(command) {
  if (!command)
    return void 0;
  const parts = command.trim().split(/\s+/);
  return parts[0] || void 0;
}
function extractExitCode(result) {
  if (typeof result === "object" && result !== null) {
    const r = result;
    if (typeof r.exitCode === "number")
      return r.exitCode;
    if (typeof r.exit_code === "number")
      return r.exit_code;
  }
  return void 0;
}
function extractProvenance(event) {
  switch (event.type) {
    case "post_tool_use":
      return extractToolUseProvenance(event);
    case "user_prompt":
      return extractUserPromptProvenance(event);
    case "subagent_stop":
      return extractSubagentProvenance(event);
    case "stop":
      return extractStopProvenance(event);
    default:
      return {};
  }
}

// ../../packages/kindling-adapter-claude-code/dist/claude-code/filter.js
var MAX_CONTENT_LENGTH = 5e4;
var MAX_RESULT_LENGTH = 1e4;
var SECRET_PATTERNS = [
  // API keys and tokens
  /['"]?(?:api[-_]?key|apikey|token|secret|password|passwd|pwd)['"]?\s*[:=]\s*['"]?([^\s'"]+)['"]?/gi,
  // AWS keys
  /(?:AWS|aws)[-_]?(?:SECRET|secret)[-_]?(?:ACCESS|access)[-_]?(?:KEY|key)\s*[:=]\s*['"]?([A-Za-z0-9/+=]{40})['"]?/g,
  // Generic API tokens (long alphanumeric strings)
  /\b(?=.*[0-9])(?=.*[A-Za-z])[A-Za-z0-9]{32,}\b/g,
  // Bearer tokens
  /Bearer\s+([A-Za-z0-9\-._~+/]+=*)/gi,
  // Basic auth
  /Basic\s+([A-Za-z0-9+/]+=*)/gi,
  // Anthropic API keys
  /sk-ant-[A-Za-z0-9\-_]{90,}/g,
  // OpenAI API keys
  /sk-[A-Za-z0-9]{48,}/g
];
var SKIP_RESULT_TOOLS = [
  "WebSearch"
  // Full search results are noisy
];
function truncateContent(content, options = {}) {
  const { maxLength = MAX_CONTENT_LENGTH, showTruncationNotice = true } = options;
  if (content.length <= maxLength) {
    return content;
  }
  const truncated = content.substring(0, maxLength);
  if (showTruncationNotice) {
    const remaining = content.length - maxLength;
    return `${truncated}

[Truncated ${remaining} characters]`;
  }
  return truncated;
}
function containsSecrets(content) {
  return SECRET_PATTERNS.some((pattern) => {
    pattern.lastIndex = 0;
    return pattern.test(content);
  });
}
function maskSecrets(content) {
  let masked = content;
  for (const pattern of SECRET_PATTERNS) {
    pattern.lastIndex = 0;
    masked = masked.replace(pattern, (match) => {
      if (match.includes(":") || match.includes("=")) {
        const parts = match.split(/[:=]/);
        return `${parts[0]}=[REDACTED]`;
      }
      return "[REDACTED]";
    });
  }
  return masked;
}
function filterContent(content, options = {}) {
  const { maskSecrets: shouldMask = true } = options;
  let filtered = content;
  if (shouldMask && containsSecrets(filtered)) {
    filtered = maskSecrets(filtered);
  }
  filtered = truncateContent(filtered, options);
  return filtered;
}
function shouldCaptureToolResult(toolName) {
  return !SKIP_RESULT_TOOLS.includes(toolName);
}
function filterToolResult(toolName, result, maxLength = MAX_RESULT_LENGTH) {
  if (!shouldCaptureToolResult(toolName)) {
    return "[Result not captured]";
  }
  if (result === void 0 || result === null) {
    return null;
  }
  let resultStr;
  if (typeof result === "string") {
    resultStr = result;
  } else {
    try {
      resultStr = JSON.stringify(result, null, 2);
    } catch {
      resultStr = String(result);
    }
  }
  return filterContent(resultStr, { maxLength, maskSecrets: true });
}

// ../../packages/kindling-adapter-claude-code/dist/claude-code/mapping.js
var TOOL_TO_KIND_MAP = {
  // File operations -> file_diff
  Write: "file_diff",
  Edit: "file_diff",
  // Shell commands -> command
  Bash: "command",
  // Everything else -> tool_call
  Read: "tool_call",
  Glob: "tool_call",
  Grep: "tool_call",
  Task: "tool_call",
  WebFetch: "tool_call",
  WebSearch: "tool_call",
  AskUserQuestion: "tool_call",
  Skill: "tool_call"
};
function mapEvent(event) {
  switch (event.type) {
    case "session_start":
    case "pre_compact":
      return { skip: true };
    case "post_tool_use":
      return mapToolUseEvent(event);
    case "user_prompt":
      return mapUserPromptEvent(event);
    case "subagent_stop":
      return mapSubagentStopEvent(event);
    case "stop":
      return { skip: true };
    default:
      return { error: `Unknown event type: ${event.type}` };
  }
}
function mapToolUseEvent(event) {
  if (!event.toolName) {
    return { error: "Tool use event missing toolName" };
  }
  const kind = TOOL_TO_KIND_MAP[event.toolName] ?? "tool_call";
  const content = formatToolContent(event);
  const provenance = extractProvenance(event);
  const scopeIds = {
    sessionId: event.sessionId,
    repoId: event.cwd
  };
  return {
    observation: {
      kind,
      content,
      provenance,
      scopeIds
    }
  };
}
function mapUserPromptEvent(event) {
  if (!event.userContent) {
    return { error: "User prompt event missing content" };
  }
  const content = filterContent(event.userContent, { maxLength: 1e4 });
  const provenance = extractProvenance(event);
  const scopeIds = {
    sessionId: event.sessionId,
    repoId: event.cwd
  };
  return {
    observation: {
      kind: "message",
      content,
      provenance,
      scopeIds
    }
  };
}
function mapSubagentStopEvent(event) {
  const content = formatSubagentContent(event);
  const provenance = extractProvenance(event);
  const scopeIds = {
    sessionId: event.sessionId,
    repoId: event.cwd
  };
  return {
    observation: {
      kind: "node_end",
      content,
      provenance,
      scopeIds
    }
  };
}
function formatToolContent(event) {
  const toolName = event.toolName ?? "unknown";
  const parts = [`Tool: ${toolName}`];
  switch (toolName) {
    case "Read": {
      const filePath = event.toolInput?.file_path;
      if (filePath)
        parts.push(`File: ${filePath}`);
      break;
    }
    case "Write": {
      const filePath = event.toolInput?.file_path;
      if (filePath)
        parts.push(`File: ${filePath}`);
      parts.push("Action: Created/overwrote file");
      break;
    }
    case "Edit": {
      const filePath = event.toolInput?.file_path;
      if (filePath)
        parts.push(`File: ${filePath}`);
      parts.push("Action: Edited file");
      break;
    }
    case "Bash": {
      const command = event.toolInput?.command;
      if (command)
        parts.push(`$ ${command}`);
      const resultStr = filterToolResult(toolName, event.toolResult);
      if (resultStr)
        parts.push(resultStr);
      break;
    }
    case "Glob": {
      const pattern = event.toolInput?.pattern;
      const path = event.toolInput?.path;
      if (pattern)
        parts.push(`Pattern: ${pattern}`);
      if (path)
        parts.push(`Path: ${path}`);
      break;
    }
    case "Grep": {
      const pattern = event.toolInput?.pattern;
      const path = event.toolInput?.path;
      if (pattern)
        parts.push(`Pattern: ${pattern}`);
      if (path)
        parts.push(`Path: ${path}`);
      break;
    }
    case "Task": {
      const agentType = event.toolInput?.subagent_type;
      const description = event.toolInput?.description;
      if (agentType)
        parts.push(`Agent: ${agentType}`);
      if (description)
        parts.push(`Task: ${description}`);
      break;
    }
    case "WebFetch": {
      const url = event.toolInput?.url;
      if (url)
        parts.push(`URL: ${url}`);
      break;
    }
    case "WebSearch": {
      const query = event.toolInput?.query;
      if (query)
        parts.push(`Query: ${query}`);
      break;
    }
    default: {
      if (event.toolInput) {
        const keys = Object.keys(event.toolInput).join(", ");
        parts.push(`Input keys: ${keys}`);
      }
    }
  }
  if (event.toolError) {
    parts.push(`Error: ${event.toolError}`);
  }
  return parts.join("\n\n");
}
function formatSubagentContent(event) {
  const parts = [`Subagent: ${event.agentType ?? "unknown"}`];
  if (event.agentOutput) {
    const output = filterContent(event.agentOutput, { maxLength: 5e3 });
    parts.push(`Output:
${output}`);
  }
  return parts.join("\n\n");
}

// ../../packages/kindling-adapter-claude-code/dist/claude-code/session.js
var import_crypto5 = require("crypto");
var SessionManager = class {
  store;
  activeSessions = /* @__PURE__ */ new Map();
  constructor(store) {
    this.store = store;
  }
  /**
   * Start a new session
   *
   * Opens a capsule for the session. If a session already has an open capsule,
   * returns the existing context.
   */
  onSessionStart(options) {
    const { sessionId, cwd, intent = "Claude Code session" } = options;
    const existing = this.activeSessions.get(sessionId);
    if (existing) {
      return existing;
    }
    const existingCapsule = this.store.getOpenCapsuleForSession(sessionId);
    if (existingCapsule) {
      const context2 = {
        sessionId,
        cwd,
        activeCapsuleId: existingCapsule.id,
        eventCount: existingCapsule.observationIds.length,
        startedAt: existingCapsule.openedAt
      };
      this.activeSessions.set(sessionId, context2);
      return context2;
    }
    const capsuleId = (0, import_crypto5.randomUUID)();
    const now = Date.now();
    const capsule = {
      id: capsuleId,
      type: "session",
      intent,
      status: "open",
      openedAt: now,
      scopeIds: {
        sessionId,
        repoId: cwd
      },
      observationIds: []
    };
    this.store.createCapsule(capsule);
    const context = {
      sessionId,
      cwd,
      activeCapsuleId: capsuleId,
      eventCount: 0,
      startedAt: now
    };
    this.activeSessions.set(sessionId, context);
    return context;
  }
  /**
   * Process an event from the session
   *
   * Maps the event to an observation and attaches it to the active capsule.
   */
  onEvent(event) {
    const context = this.activeSessions.get(event.sessionId);
    if (!context) {
      return {
        error: `No active session found for sessionId: ${event.sessionId}`
      };
    }
    const mapResult = mapEvent(event);
    if (mapResult.skip) {
      return { skipped: true };
    }
    if (mapResult.error) {
      return { error: mapResult.error };
    }
    if (!mapResult.observation) {
      return { error: "Mapping produced no observation" };
    }
    const observation = {
      id: (0, import_crypto5.randomUUID)(),
      ts: event.timestamp,
      redacted: false,
      provenance: {},
      ...mapResult.observation
    };
    this.store.insertObservation(observation);
    this.store.attachObservationToCapsule(context.activeCapsuleId, observation.id);
    context.eventCount += 1;
    return { observation };
  }
  /**
   * End a session (called on Stop hook)
   *
   * Closes the active capsule for the session.
   */
  onStop(sessionId, signals) {
    const context = this.activeSessions.get(sessionId);
    if (!context) {
      throw new Error(`No active session found for sessionId: ${sessionId}`);
    }
    const closedAt = Date.now();
    this.store.closeCapsule(context.activeCapsuleId, closedAt);
    if (signals?.summaryContent) {
      const summary = {
        id: (0, import_crypto5.randomUUID)(),
        capsuleId: context.activeCapsuleId,
        content: signals.summaryContent,
        confidence: signals.summaryConfidence ?? 0.8,
        createdAt: closedAt,
        evidenceRefs: signals.evidenceRefs ?? []
      };
      this.store.insertSummary(summary);
    }
    this.activeSessions.delete(sessionId);
    const capsule = {
      id: context.activeCapsuleId,
      type: "session",
      intent: "Claude Code session",
      status: "closed",
      openedAt: context.startedAt,
      closedAt,
      scopeIds: {
        sessionId,
        repoId: context.cwd
      },
      observationIds: []
    };
    return capsule;
  }
  /**
   * Get active session context
   */
  getSession(sessionId) {
    return this.activeSessions.get(sessionId);
  }
  /**
   * Check if session is active
   */
  isSessionActive(sessionId) {
    return this.activeSessions.has(sessionId);
  }
  /**
   * Get all active session IDs
   */
  getActiveSessions() {
    return Array.from(this.activeSessions.keys());
  }
  /**
   * Get session statistics
   */
  getSessionStats(sessionId) {
    const context = this.activeSessions.get(sessionId);
    if (!context)
      return void 0;
    return {
      eventCount: context.eventCount,
      duration: Date.now() - context.startedAt
    };
  }
};

// ../../packages/kindling-adapter-claude-code/dist/claude-code/hooks.js
function createHookHandlers(store, config = {}) {
  const { captureResults = true, captureUserMessages = true, captureSubagents = true, defaultIntent = "Claude Code session" } = config;
  const sessionManager = new SessionManager(store);
  return {
    /**
     * SessionStart hook handler
     *
     * Opens a new capsule for the session.
     */
    onSessionStart: (ctx) => {
      sessionManager.onSessionStart({
        sessionId: ctx.sessionId,
        cwd: ctx.cwd,
        intent: defaultIntent
      });
      return { continue: true };
    },
    /**
     * PostToolUse hook handler
     *
     * Captures tool calls as observations.
     */
    onPostToolUse: (ctx) => {
      if (!captureResults) {
        return { continue: true };
      }
      const event = createPostToolUseEvent(ctx);
      sessionManager.onEvent(event);
      return { continue: true };
    },
    /**
     * Stop hook handler
     *
     * Closes the session capsule.
     */
    onStop: (ctx) => {
      const event = createStopEvent(ctx);
      try {
        sessionManager.onStop(event.sessionId, {
          reason: ctx.reason,
          summaryContent: ctx.summary
        });
      } catch {
        console.warn(`Could not close session ${event.sessionId}: session not found`);
      }
      return { continue: true };
    },
    /**
     * SubagentStop hook handler
     *
     * Captures subagent completions as observations.
     */
    onSubagentStop: (ctx) => {
      if (!captureSubagents) {
        return { continue: true };
      }
      const event = createSubagentStopEvent(ctx);
      sessionManager.onEvent(event);
      return { continue: true };
    },
    /**
     * UserPromptSubmit hook handler
     *
     * Captures user messages as observations.
     */
    onUserPromptSubmit: (ctx) => {
      if (!captureUserMessages) {
        return { continue: true };
      }
      const event = createUserPromptEvent(ctx);
      sessionManager.onEvent(event);
      return { continue: true };
    },
    /**
     * Get the session manager for advanced usage
     */
    getSessionManager: () => sessionManager,
    /**
     * Check if a session is active
     */
    isSessionActive: (sessionId) => sessionManager.isSessionActive(sessionId),
    /**
     * Get session statistics
     */
    getSessionStats: (sessionId) => sessionManager.getSessionStats(sessionId)
  };
}
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {
  KindlingService,
  LocalFtsProvider,
  SqliteKindlingStore,
  closeDatabase,
  createHookHandlers,
  extractProvenance,
  filterContent,
  filterToolResult,
  mapEvent,
  maskSecrets,
  openDatabase,
  runMigrations,
  truncateContent,
  validateCapsule,
  validateObservation,
  validateSummary
});
