-- Kindling SQLite Schema Contract
-- =================================
-- Canonical DDL reflecting the state after all migrations (through 005).
-- Both the TypeScript store (better-sqlite3) and the Rust crate MUST produce
-- an identical structure when creating a fresh database.
--
-- See README.md in this directory for update rules and breaking-change policy.

-- Runtime schema version — readable from any SQLite client via:
--   PRAGMA user_version;
-- Must match the "version" field in version.json.
PRAGMA user_version = 5;

-- ============================================================================
-- Core tables
-- ============================================================================

-- Tracks which migrations have been applied (used by the TypeScript runner).
CREATE TABLE IF NOT EXISTS schema_migrations (
  version     INTEGER PRIMARY KEY,
  name        TEXT    NOT NULL,
  applied_at  INTEGER NOT NULL          -- epoch milliseconds
);

-- Seed migration history so the TypeScript runner (which checks
-- schema_migrations to determine the current version) does not attempt
-- to replay migrations against an already-complete schema.
INSERT OR IGNORE INTO schema_migrations (version, name, applied_at) VALUES
  (1, '001_init',                  0),
  (2, '002_fts',                   0),
  (3, '003_indexes',               0),
  (4, '004_denormalize_scopes',    0),
  (5, '005_pragma_user_version',   0);

-- Atomic units of captured context.
CREATE TABLE IF NOT EXISTS observations (
  id          TEXT    PRIMARY KEY,
  kind        TEXT    NOT NULL CHECK(kind IN (
                'tool_call',
                'command',
                'file_diff',
                'error',
                'message',
                'node_start',
                'node_end',
                'node_output',
                'node_error'
              )),
  content     TEXT    NOT NULL,
  provenance  TEXT    NOT NULL DEFAULT '{}',   -- JSON blob
  ts          INTEGER NOT NULL,                -- epoch milliseconds
  scope_ids   TEXT    NOT NULL DEFAULT '{}',   -- JSON blob (legacy, kept for compat)
  redacted    INTEGER NOT NULL DEFAULT 0 CHECK(redacted IN (0, 1)),
  -- Denormalized scope columns (migration 004) — prefer these over scope_ids
  session_id  TEXT,
  repo_id     TEXT,
  agent_id    TEXT,
  user_id     TEXT
);

-- Bounded units that group observations.
CREATE TABLE IF NOT EXISTS capsules (
  id          TEXT    PRIMARY KEY,
  type        TEXT    NOT NULL CHECK(type IN ('session', 'pocketflow_node')),
  intent      TEXT    NOT NULL,
  status      TEXT    NOT NULL CHECK(status IN ('open', 'closed')) DEFAULT 'open',
  opened_at   INTEGER NOT NULL,                -- epoch milliseconds
  closed_at   INTEGER,                         -- epoch milliseconds, NULL while open
  scope_ids   TEXT    NOT NULL DEFAULT '{}',   -- JSON blob (legacy, kept for compat)
  -- Denormalized scope columns (migration 004)
  session_id  TEXT,
  repo_id     TEXT,
  agent_id    TEXT,
  user_id     TEXT
);

-- Many-to-many relationship between capsules and observations (with ordering).
CREATE TABLE IF NOT EXISTS capsule_observations (
  capsule_id     TEXT    NOT NULL,
  observation_id TEXT    NOT NULL,
  seq            INTEGER NOT NULL,             -- ordering within capsule
  PRIMARY KEY (capsule_id, observation_id),
  FOREIGN KEY (capsule_id)     REFERENCES capsules(id)     ON DELETE CASCADE,
  FOREIGN KEY (observation_id) REFERENCES observations(id) ON DELETE CASCADE
);

-- AI-generated summaries of closed capsules.
CREATE TABLE IF NOT EXISTS summaries (
  id             TEXT    PRIMARY KEY,
  capsule_id     TEXT    NOT NULL UNIQUE,
  content        TEXT    NOT NULL,
  confidence     REAL    NOT NULL CHECK(confidence >= 0.0 AND confidence <= 1.0),
  created_at     INTEGER NOT NULL,             -- epoch milliseconds
  evidence_refs  TEXT    NOT NULL DEFAULT '[]', -- JSON array of observation IDs
  FOREIGN KEY (capsule_id) REFERENCES capsules(id) ON DELETE CASCADE
);

-- User-controlled priority content (non-evictable until expired/removed).
CREATE TABLE IF NOT EXISTS pins (
  id          TEXT    PRIMARY KEY,
  target_type TEXT    NOT NULL CHECK(target_type IN ('observation', 'summary')),
  target_id   TEXT    NOT NULL,
  reason      TEXT,
  created_at  INTEGER NOT NULL,                -- epoch milliseconds
  expires_at  INTEGER,                         -- epoch milliseconds, NULL = no expiry
  scope_ids   TEXT    NOT NULL DEFAULT '{}',   -- JSON blob (legacy, kept for compat)
  -- Denormalized scope columns (migration 004)
  session_id  TEXT,
  repo_id     TEXT,
  agent_id    TEXT,
  user_id     TEXT
);

-- ============================================================================
-- Full-Text Search (FTS5)
-- ============================================================================
-- IMPORTANT: The tokenizer MUST be 'porter unicode61' in both TypeScript and
-- Rust implementations. Changing the tokenizer is a BREAKING CHANGE — it
-- invalidates existing FTS indexes and produces different search results.
-- Porter stemming + Unicode normalization ensures consistent full-text search
-- across languages and implementations.

-- FTS index over observations.content (external content table).
CREATE VIRTUAL TABLE IF NOT EXISTS observations_fts USING fts5(
  content,
  content='observations',
  content_rowid='rowid',
  tokenize='porter unicode61'
);

-- FTS index over summaries.content (external content table).
CREATE VIRTUAL TABLE IF NOT EXISTS summaries_fts USING fts5(
  content,
  content='summaries',
  content_rowid='rowid',
  tokenize='porter unicode61'
);

-- ============================================================================
-- Triggers — keep FTS indexes in sync with content tables
-- ============================================================================

-- observations_fts sync
CREATE TRIGGER IF NOT EXISTS observations_fts_insert
AFTER INSERT ON observations
WHEN NEW.redacted = 0
BEGIN
  INSERT INTO observations_fts(rowid, content) VALUES (NEW.rowid, NEW.content);
END;

CREATE TRIGGER IF NOT EXISTS observations_fts_update
AFTER UPDATE ON observations
BEGIN
  INSERT INTO observations_fts(observations_fts, rowid, content) VALUES('delete', OLD.rowid, OLD.content);
  INSERT INTO observations_fts(rowid, content) SELECT NEW.rowid, NEW.content WHERE NEW.redacted = 0;
END;

CREATE TRIGGER IF NOT EXISTS observations_fts_delete
AFTER DELETE ON observations
BEGIN
  INSERT INTO observations_fts(observations_fts, rowid, content) VALUES('delete', OLD.rowid, OLD.content);
END;

-- summaries_fts sync
CREATE TRIGGER IF NOT EXISTS summaries_fts_insert
AFTER INSERT ON summaries
BEGIN
  INSERT INTO summaries_fts(rowid, content) VALUES (NEW.rowid, NEW.content);
END;

CREATE TRIGGER IF NOT EXISTS summaries_fts_update
AFTER UPDATE ON summaries
BEGIN
  INSERT INTO summaries_fts(summaries_fts, rowid, content) VALUES('delete', OLD.rowid, OLD.content);
  INSERT INTO summaries_fts(rowid, content) VALUES (NEW.rowid, NEW.content);
END;

CREATE TRIGGER IF NOT EXISTS summaries_fts_delete
AFTER DELETE ON summaries
BEGIN
  INSERT INTO summaries_fts(summaries_fts, rowid, content) VALUES('delete', OLD.rowid, OLD.content);
END;

-- ============================================================================
-- Indexes
-- ============================================================================

-- Observations
CREATE INDEX IF NOT EXISTS idx_observations_ts        ON observations(ts DESC);
CREATE INDEX IF NOT EXISTS idx_observations_kind       ON observations(kind);
CREATE INDEX IF NOT EXISTS idx_obs_session_ts          ON observations(session_id, ts DESC) WHERE session_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_obs_repo_ts             ON observations(repo_id, ts DESC)    WHERE repo_id IS NOT NULL;

-- Capsules
CREATE INDEX IF NOT EXISTS idx_capsules_opened_at      ON capsules(opened_at DESC);
CREATE INDEX IF NOT EXISTS idx_caps_status_session      ON capsules(status, session_id)      WHERE session_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_caps_repo                ON capsules(repo_id)                  WHERE repo_id IS NOT NULL;

-- Capsule-observations
CREATE INDEX IF NOT EXISTS idx_capsule_observations_capsule     ON capsule_observations(capsule_id, seq);
CREATE INDEX IF NOT EXISTS idx_capsule_observations_observation ON capsule_observations(observation_id);

-- Summaries
CREATE INDEX IF NOT EXISTS idx_summaries_capsule       ON summaries(capsule_id);
CREATE INDEX IF NOT EXISTS idx_summaries_created_at    ON summaries(created_at DESC);

-- Pins
CREATE INDEX IF NOT EXISTS idx_pins_expires_at         ON pins(expires_at)    WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_pins_target             ON pins(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_pins_session_id         ON pins(session_id)    WHERE session_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_pins_repo_id            ON pins(repo_id)       WHERE repo_id IS NOT NULL;
