# Data Model

## Overview

kindling's data model is designed for **local-first memory capture** with **deterministic retrieval**. Rust is the canonical implementation: the domain types live in the `kindling-types` crate and are serialized as **camelCase JSON**. The optional `ts-rs` feature emits checked-in TypeScript bindings under `crates/kindling-types/bindings/`, and CI fails if those bindings drift from the Rust definitions — this keeps cross-language type parity exact.

The model consists of five core entities:

1. **Observation** — atomic record of something that happened
2. **Capsule** — bounded unit of meaning (session, workflow node)
3. **Summary** — high-level description of a capsule's content
4. **Pin** — user-marked important item
5. **ScopeIds** — multi-dimensional isolation (session, repo, agent, user, task)

All entities are immutable once created (except for status transitions and redaction).

## Core Entities

### Observation

An **Observation** is an atomic, immutable record of an event that occurred during development.

```rust
pub struct Observation {
    pub id: String,                  // Unique identifier (UUID or similar)
    pub kind: ObservationKind,       // Type of observation
    pub content: String,             // The actual content (text, JSON, etc.)
    pub provenance: serde_json::Value, // Source-specific metadata (JSON object)
    pub ts: i64,                     // Timestamp (epoch milliseconds)
    pub scope_ids: ScopeIds,         // Isolation dimensions (camelCase: scopeIds)
    pub redacted: bool,              // Privacy flag
}

pub enum ObservationKind {
    ToolCall,   // "tool_call"   — tool invocation (e.g., grep, read file)
    Command,    // "command"     — shell command execution
    FileDiff,   // "file_diff"   — file change
    Error,      // "error"       — error or exception
    Message,    // "message"     — user or agent message
    NodeStart,  // "node_start"  — workflow node started
    NodeEnd,    // "node_end"    — workflow node ended
    NodeOutput, // "node_output" — workflow node output
    NodeError,  // "node_error"  — workflow node error
}
```

`ObservationKind` serializes to the snake_case string values shown in the comments (matching the `CHECK` constraint in the canonical schema).

**Properties:**

- `id` — Globally unique; used for referencing and provenance
- `kind` — Enables filtering by observation type
- `content` — Stored as text; adapters serialize structured data to JSON
- `provenance` — Adapter-specific JSON object (e.g., `toolName`, `exitCode`, `nodeId`)
- `ts` — Used for ordering and time-based queries
- `scopeIds` — Enables scoped retrieval (isolate by session, repo, etc.)
- `redacted` — If true, content is redacted and excluded from FTS

**Immutability:**

- Observations are append-only
- Only the `redacted` flag (and the associated content removal) can change, via `forget` — see Redaction below

### Capsule

A **Capsule** is a bounded unit of meaning that groups related observations.

```rust
pub struct Capsule {
    pub id: String,             // Unique identifier
    pub type_: CapsuleType,     // Type of capsule (serialized as "type")
    pub intent: String,         // Human-readable description of capsule purpose
    pub status: CapsuleStatus,  // Lifecycle state
    pub opened_at: i64,         // Timestamp when capsule opened (epoch ms)
    pub closed_at: Option<i64>, // Timestamp when capsule closed (epoch ms)
    pub scope_ids: ScopeIds,    // Isolation dimensions
}

pub enum CapsuleType {
    Session,        // "session"         — agent/editor session
    PocketflowNode, // "pocketflow_node" — PocketFlow workflow node
}

pub enum CapsuleStatus {
    Open,   // "open"   — accepting observations
    Closed, // "closed" — finalized
}
```

Note: capsule membership is **not** stored on the capsule itself. The ordered set of observations belonging to a capsule is tracked via the `capsule_observations` join table, which carries a `seq` column for deterministic ordering. Summaries reference their capsule via `summary.capsuleId` (one summary per capsule).

**Properties:**

- `type` — Determines capsule semantics (session vs. node)
- `intent` — Stored for context (e.g., "Fix authentication bug")
- `status` — Transitions: open → closed (one-way)
- Observation membership — Deterministic order via `capsule_observations.seq`

**Lifecycle:**

1. **Open** — Capsule created with `status = open`
2. **Accumulate** — Observations attached via the `capsule_observations` join table
3. **Close** — Status transitions to `closed`, `closedAt` set, optional summary generated

**Scoping:**

- Capsules inherit `scopeIds` from the opening context
- All attached observations share the same scope

### Summary

A **Summary** is a high-level description of a capsule's content.

```rust
pub struct Summary {
    pub id: String,                // Unique identifier
    pub capsule_id: String,        // Reference to parent capsule (camelCase: capsuleId)
    pub content: String,           // Summary text
    pub confidence: f64,           // 0.0..=1.0 (quality/confidence score)
    pub created_at: i64,           // Timestamp (epoch ms)
    pub evidence_refs: Vec<String>, // Observation IDs that support this summary
}
```

**Properties:**

- `capsuleId` — One-to-one relationship (one summary per capsule)
- `content` — Human-readable summary (typically LLM-generated)
- `confidence` — Quality indicator in `[0.0, 1.0]` (enforced by a schema `CHECK`)
- `evidenceRefs` — Provenance: which observations informed this summary

**Generation:**

- Summaries are typically generated on capsule close
- Mid-capsule rollups are optional (triggered by size/noise thresholds)
- Conservative summarization by default (raw observations retained)

### Pin

A **Pin** marks an observation or summary as important.

```rust
pub struct Pin {
    pub id: String,               // Unique identifier
    pub target_type: PinTarget,   // What is pinned: observation | summary
    pub target_id: String,        // ID of pinned entity
    pub reason: Option<String>,   // Optional explanation
    pub created_at: i64,          // Timestamp (epoch ms)
    pub expires_at: Option<i64>,  // Optional TTL (epoch ms)
    pub scope_ids: ScopeIds,      // Isolation dimensions
}

pub enum PinTarget {
    Observation, // "observation"
    Summary,     // "summary"
}
```

**Properties:**

- `targetType` + `targetId` — References the pinned entity
- `reason` — User-provided context (e.g., "Critical context for auth flow")
- `expiresAt` — Optional TTL for time-bound pins (e.g., session-only)

**Retrieval Semantics:**

- Pins are **non-evictable** in retrieval results
- Active pins (not expired) always appear first in retrieval response
- TTL-aware: pins with `expiresAt <= now` are excluded

### ScopeIds

**ScopeIds** enable multi-dimensional isolation for queries and retrieval. All five fields are optional.

```rust
pub struct ScopeIds {
    pub session_id: Option<String>, // Session isolation (FILTERABLE)
    pub repo_id: Option<String>,    // Repository isolation (FILTERABLE)
    pub agent_id: Option<String>,   // Agent isolation (FILTERABLE)
    pub user_id: Option<String>,    // User isolation (FILTERABLE)
    pub task_id: Option<String>,    // Task grouping (NOT filterable — see note)
}
```

**Filterability:**

| Field       | Denormalized column | Retrieval-filterable |
| ----------- | ------------------- | -------------------- |
| `sessionId` | `session_id`        | Yes                  |
| `repoId`    | `repo_id`           | Yes                  |
| `agentId`   | `agent_id`          | Yes                  |
| `userId`    | `user_id`           | Yes                  |
| `taskId`    | _(none)_            | **No**               |

> **`taskId` is intentionally NOT filterable.** It has no denormalized column and is never used as a retrieval scope filter. It is carried purely for provenance and grouping — it travels with the entity and appears in serialized output, but supplying a `taskId` in `RetrieveOptions.scopeIds` has no effect on the result set.

**Usage:**

- All entities carry `scopeIds`
- Queries can filter by one or more of the filterable scope dimensions (`sessionId`, `repoId`, `agentId`, `userId`)
- Default retrieval: scoped to current session + repo

**Examples (JSON, as serialized):**

```jsonc
// All observations from a specific session
{ "sessionId": "abc123" }

// All observations in a specific repo
{ "repoId": "/path/to/repo" }

// Specific session within a specific repo
{ "sessionId": "abc123", "repoId": "/path/to/repo" }
```

## Relationships

### Entity Relationships

```
Capsule (1) ----- (0..1) Summary
        |
        +------- (0..N) Observation  (via capsule_observations)

Pin (N) --------- (1) Observation | Summary
```

**Capsule — Observation:**

- One capsule has zero or more observations
- Relationship tracked via the `capsule_observations` join table, ordered by `seq`

**Capsule — Summary:**

- One capsule has zero or one summary
- One summary belongs to exactly one capsule (`capsuleId` is `UNIQUE`)

**Pin — Observation/Summary:**

- One pin targets exactly one observation or summary
- One observation/summary can have multiple pins

### Database Schema

The **canonical schema** is `schema/schema.sql` (the DDL reflecting the state after all migrations) together with `schema/version.json` (currently **version 5**). Both the Rust store and the deprecated TypeScript store MUST produce an identical structure from a fresh database. The runtime version is also readable from any SQLite client via `PRAGMA user_version;`.

Key facts (authoritative source is `schema/schema.sql`):

- **Tables:** `observations`, `capsules`, `capsule_observations`, `summaries`, `pins`, `observations_fts`, `summaries_fts`, `schema_migrations`.
- **FTS tokenizer:** `porter unicode61` on both FTS tables. Changing it is a **breaking change** — it invalidates existing indexes and changes search results.
- **Scope ids are denormalized into columns** (`session_id`, `repo_id`, `agent_id`, `user_id`) on `observations`, `capsules`, and `pins` — added by migration `004_denormalize_scopes`. Retrieval filters on these columns directly; it does **not** use `json_extract` against the legacy `scope_ids` JSON blob. There is no `task_id` column.

The excerpt below is **illustrative only** — refer to `schema/schema.sql` for the exact, canonical DDL (constraints, triggers, and indexes included).

```sql
-- ILLUSTRATIVE EXCERPT — canonical source is schema/schema.sql
PRAGMA user_version = 5;

CREATE TABLE observations (
  id          TEXT    PRIMARY KEY,
  kind        TEXT    NOT NULL CHECK(kind IN (
                'tool_call','command','file_diff','error','message',
                'node_start','node_end','node_output','node_error')),
  content     TEXT    NOT NULL,
  provenance  TEXT    NOT NULL DEFAULT '{}',   -- JSON blob
  ts          INTEGER NOT NULL,                -- epoch milliseconds
  scope_ids   TEXT    NOT NULL DEFAULT '{}',   -- JSON blob (legacy, kept for compat)
  redacted    INTEGER NOT NULL DEFAULT 0 CHECK(redacted IN (0,1)),
  -- Denormalized scope columns (migration 004) — preferred for filtering
  session_id  TEXT,
  repo_id     TEXT,
  agent_id    TEXT,
  user_id     TEXT
);

CREATE TABLE capsule_observations (
  capsule_id     TEXT    NOT NULL,
  observation_id TEXT    NOT NULL,
  seq            INTEGER NOT NULL,             -- ordering within capsule
  PRIMARY KEY (capsule_id, observation_id),
  FOREIGN KEY (capsule_id)     REFERENCES capsules(id)     ON DELETE CASCADE,
  FOREIGN KEY (observation_id) REFERENCES observations(id) ON DELETE CASCADE
);

-- FTS index over observations.content (external content table)
CREATE VIRTUAL TABLE observations_fts USING fts5(
  content,
  content='observations',
  content_rowid='rowid',
  tokenize='porter unicode61'
);
```

## Data Flow Examples

These examples use the embedded Rust API from the `kindling-service` crate
(`KindlingService`). The equivalent daemon-backed flow uses `kindling-client`'s `Client`.

### Example 1: Session Capture

```rust
use kindling_service::KindlingService;

// 0. Open the local store
let svc = KindlingService::open("./.kindling/kindling.db")?;

// 1. Session starts
let capsule = svc.open_capsule(open_capsule_options)?; // type=session, intent, scopeIds

// 2. Tool call happens
svc.append_observation(tool_call_input, append_options)?;

// 3. File edit happens
svc.append_observation(file_diff_input, append_options)?;

// 4. Session ends (optionally generating a summary)
svc.close_capsule(capsule.id, close_options)?;
```

Each `append_observation` input carries `kind`, `content`, `provenance`, `ts`,
`scopeIds`, and `redacted`; the append options bind the observation to the open
capsule. `close_capsule` options may include the summary `content`, `confidence`,
and `evidenceRefs`.

### Example 2: Retrieval

```rust
// Query: "authentication" within a repo scope
let result = svc.retrieve(retrieve_options)?; // query, scopeIds { repoId }, max_candidates
```

The `RetrieveResult` has three tiers plus provenance:

```jsonc
{
  "pins": [
    {
      "pin": { "targetType": "observation", "targetId": "obs1" /* … */ },
      "target": { "id": "obs1", "kind": "file_diff" /* … */ },
    },
  ],
  "currentSummary": {
    "capsuleId": "cap1",
    "content": "Working on authentication bug…",
    "confidence": 0.8,
  },
  "candidates": [
    { "entity": { "id": "obs2", "kind": "file_diff" /* … */ }, "score": 0.95 },
    { "entity": { "id": "obs3", "kind": "tool_call" /* … */ }, "score": 0.87 },
  ],
  "provenance": {
    "query": "authentication",
    "scopeIds": { "repoId": "/repo" },
    "totalCandidates": 15,
    "returnedCandidates": 10,
    "truncatedDueToTokenBudget": false,
    "providerUsed": "local-fts",
  },
}
```

See [Retrieval Contract](./retrieval-contract.md) for the full tiering, scoring, and determinism rules.

### Example 3: Redaction

Redaction is performed via `forget`. It moves the observation's content to a
redacted state, removes it from the FTS index, and **preserves provenance and
references** (the id still exists, and `capsule_observations`/`evidenceRefs`
entries remain).

```rust
// Redact (forget) a sensitive observation
svc.forget("obs5")?;
```

After `forget("obs5")`:

```jsonc
{
  "id": "obs5",
  "kind": "tool_call",
  "content": "[redacted]",
  "provenance": { "toolName": "read_file" },
  "ts": 1234567890,
  "scopeIds": { "sessionId": "s1" },
  "redacted": true,
}
```

FTS search no longer returns `obs5`, but the capsule still references it via
`capsule_observations` (provenance preserved). It is excluded from retrieval
unless `include_redacted` is set.

## Constraints and Invariants

### Immutability

- Observations: Immutable except the `redacted` flag (and its content removal) via `forget`
- Capsules: Immutable except `status` and `closedAt`
- Summaries: Fully immutable
- Pins: Immutable (delete to remove via `unpin`)

### Ordering

- Capsule membership: `capsule_observations.seq` (insertion order, deterministic)
- Retrieval results: Pins → Current Summary → Candidates (tiered)
- Export: Deterministic ordering by timestamp

### Referential Integrity

- `summary.capsuleId` must reference an existing capsule (`UNIQUE`)
- Rows in `capsule_observations` must reference existing capsules and observations
- Pin targets must exist
- `forget` preserves references (redact content, do not delete the row)

### Scope Consistency

- All observations in a capsule share the capsule's `scopeIds`
- Pins inherit scope from creation context
- Retrieval filters apply to all tiers (pins, summaries, candidates) — using the filterable scope dimensions only (`taskId` is excluded; see ScopeIds)

## Evolution and Versioning

### Schema Versioning

- Schema version tracked in `schema/version.json` (and `PRAGMA user_version`), with applied migrations recorded in the `schema_migrations` table
- Current version is **5**; `minCompatible` is 1
- Migrations are additive only (no destructive changes)
- Export bundles include version for forward compatibility (see below)

### Export Bundle Compatibility

Export bundles (produced by `KindlingService::export`, JSON-compatible with the
TypeScript `ExportBundle`) are a **stable, portable interchange format**, decoupled
from the SQLite schema version above. Each bundle carries `bundleVersion` and a
`dataset.version`, both currently **`"1.0"`** (`BUNDLE_VERSION` in
`crates/kindling-service/src/export.rs`).

Stability guarantee:

- **`1.0` is a stable contract.** A bundle written by any kindling release that
  stamps `bundleVersion: "1.0"` will import into any other release that accepts
  `1.0`. Field shapes already present in `1.0` will not be removed or repurposed
  within the `1.0` line.
- **Additive evolution stays `1.0`.** New optional fields may be added to the
  bundle/dataset without bumping the version; older importers ignore unknown
  fields. The version is bumped only for a breaking change.
- **Forward-compatibility policy.** Importers validate `bundleVersion` /
  `dataset.version` and reject versions they do not recognise (`Unsupported
bundle version`) rather than silently mis-parsing. Pin on `bundleVersion` to
  detect format changes deterministically.
- Within a bundle, all entity lists use deterministic ordering (`ts`/`*_at` then
  `id`), so re-exporting unchanged data is byte-stable.

### Cross-Language Type Parity

- Rust is canonical; domain types live in `kindling-types`
- The optional `ts-rs` feature generates TypeScript bindings under `crates/kindling-types/bindings/`
- CI fails on drift, guaranteeing the JSON wire format stays identical across the Rust store/service/client and any TypeScript consumer

### Type Extensions (Future)

- Additional `ObservationKind` values can be added (extend the enum and the schema `CHECK`)
- Additional `CapsuleType` values can be added
- `provenance` is an unstructured JSON object (extensible by adapters)
- `ScopeIds` can add new dimensions (e.g., `organizationId`)

## Design Decisions

1. **Canonical Rust types** — `kindling-types`, serialized as camelCase JSON, with ts-rs bindings checked for drift in CI
2. **Denormalized scope columns** — scope ids live in real columns for fast, indexable filtering; the legacy `scope_ids` JSON blob is kept only for compatibility
3. **`taskId` is provenance-only** — carried for grouping, never a retrieval filter (no column)
4. **No cascading content loss** — `forget` (redaction) over deletion; preserve provenance
5. **Deterministic ordering** — all lists have a defined order (`seq`, timestamp)
