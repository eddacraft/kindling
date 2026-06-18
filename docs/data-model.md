# Data Model

## Overview

kindling's data model is designed for **local-first memory capture** with **deterministic retrieval**. The model consists of five core entities:

1. **Observation** — atomic record of something that happened
2. **Capsule** — bounded unit of meaning (session, workflow node)
3. **Summary** — high-level description of a capsule's content
4. **Pin** — user-marked important item
5. **ScopeIds** — multi-dimensional isolation (session, repo, agent, user)

All entities are immutable once created (except for status transitions and redaction).

## Core Entities

### Observation

An **Observation** is an atomic, immutable record of an event that occurred during development.

```typescript
interface Observation {
  id: string; // Unique identifier (UUIDv4 or similar)
  kind: ObservationKind; // Type of observation
  content: string; // The actual content (text, JSON, etc.)
  provenance: Record<string, unknown>; // Source-specific metadata
  ts: number; // Timestamp (epoch milliseconds)
  scopeIds: ScopeIds; // Isolation dimensions
  redacted: boolean; // Privacy flag
}

type ObservationKind =
  | 'tool_call' // Tool invocation (e.g., grep, read file)
  | 'command' // Shell command execution
  | 'file_diff' // File change
  | 'error' // Error or exception
  | 'message' // User or agent message
  | 'node_start' // Workflow node started
  | 'node_end' // Workflow node ended
  | 'node_output' // Workflow node output
  | 'node_error'; // Workflow node error
```

**Properties:**

- `id` — Globally unique; used for referencing and provenance
- `kind` — Enables filtering by observation type
- `content` — Stored as text; adapters serialize structured data to JSON
- `provenance` — Adapter-specific metadata (e.g., `toolName`, `exitCode`, `nodeId`)
- `ts` — Used for ordering and time-based queries
- `scopeIds` — Enables scoped retrieval (isolate by session, repo, etc.)
- `redacted` — If true, content is `[redacted]` and excluded from FTS

**Immutability:**

- Observations are append-only
- Only `redacted` flag can change (via explicit redaction API)

### Capsule

A **Capsule** is a bounded unit of meaning that groups related observations.

```typescript
interface Capsule {
  id: string; // Unique identifier
  type: CapsuleType; // Type of capsule
  intent: string; // Human-readable description of capsule purpose
  status: CapsuleStatus; // Lifecycle state
  openedAt: number; // Timestamp when capsule opened (epoch ms)
  closedAt?: number; // Timestamp when capsule closed (epoch ms)
  scopeIds: ScopeIds; // Isolation dimensions
  observationIds: string[]; // Ordered list of observation IDs
  summaryId?: string; // Optional summary reference
}

type CapsuleType =
  | 'session' // OpenCode session
  | 'pocketflow_node'; // PocketFlow workflow node

type CapsuleStatus =
  | 'open' // Accepting observations
  | 'closed'; // Finalized
```

**Properties:**

- `type` — Determines capsule semantics (session vs. node)
- `intent` — Stored for context (e.g., "Fix authentication bug")
- `status` — Transitions: open → closed (one-way)
- `observationIds` — Deterministic order (insertion order)
- `summaryId` — Links to optional Summary entity

**Lifecycle:**

1. **Open** — Capsule created with `status=open`
2. **Accumulate** — Observations attached via `observationIds`
3. **Close** — Status transitions to `closed`, optional summary generated

**Scoping:**

- Capsules inherit `scopeIds` from the opening context
- All attached observations share the same scope

### Summary

A **Summary** is a high-level description of a capsule's content.

```typescript
interface Summary {
  id: string; // Unique identifier
  capsuleId: string; // Reference to parent capsule
  content: string; // Summary text
  confidence: number; // 0.0-1.0 (quality/confidence score)
  createdAt: number; // Timestamp (epoch ms)
  evidenceRefs: string[]; // Observation IDs that support this summary
}
```

**Properties:**

- `capsuleId` — One-to-one relationship (one summary per capsule)
- `content` — Human-readable summary (typically LLM-generated)
- `confidence` — Quality indicator (0.0 = low, 1.0 = high)
- `evidenceRefs` — Provenance: which observations informed this summary

**Generation:**

- Summaries are typically generated on capsule close
- Mid-capsule rollups are optional (triggered by size/noise thresholds)
- v0.1: Conservative summarization (raw observations retained by default)

### Pin

A **Pin** marks an observation or summary as important.

```typescript
interface Pin {
  id: string; // Unique identifier
  targetType: 'observation' | 'summary'; // What is pinned
  targetId: string; // ID of pinned entity
  reason?: string; // Optional explanation
  createdAt: number; // Timestamp (epoch ms)
  expiresAt?: number; // Optional TTL (epoch ms)
  scopeIds: ScopeIds; // Isolation dimensions
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

**ScopeIds** enable multi-dimensional isolation for queries and retrieval.

```typescript
interface ScopeIds {
  sessionId?: string; // Session isolation (e.g., OpenCode session)
  repoId?: string; // Repository isolation
  agentId?: string; // Agent isolation (future)
  userId?: string; // User isolation (future)
}
```

**Usage:**

- All entities have `scopeIds`
- Queries can filter by one or more scope dimensions
- Default retrieval: scoped to current session + repo

**Examples:**

```typescript
// All observations from a specific session
{ sessionId: "abc123" }

// All observations in a specific repo
{ repoId: "/path/to/repo" }

// Specific session within a specific repo
{ sessionId: "abc123", repoId: "/path/to/repo" }
```

## Relationships

### Entity Relationships

```
Capsule (1) ----- (0..1) Summary
        |
        +------- (0..N) Observation

Pin (N) --------- (1) Observation | Summary
```

**Capsule — Observation:**

- One capsule has zero or more observations
- One observation can belong to one capsule
- Relationship tracked via `capsule_observations` join table

**Capsule — Summary:**

- One capsule has zero or one summary
- One summary belongs to exactly one capsule

**Pin — Observation/Summary:**

- One pin targets exactly one observation or summary
- One observation/summary can have multiple pins

### Database Schema (Conceptual)

```sql
-- Core entities
CREATE TABLE observations (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  content TEXT NOT NULL,
  provenance TEXT NOT NULL,  -- JSON blob
  ts INTEGER NOT NULL,
  scope_ids TEXT NOT NULL,   -- JSON blob
  redacted INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE capsules (
  id TEXT PRIMARY KEY,
  type TEXT NOT NULL,
  intent TEXT NOT NULL,
  status TEXT NOT NULL,
  opened_at INTEGER NOT NULL,
  closed_at INTEGER,
  scope_ids TEXT NOT NULL    -- JSON blob
);

CREATE TABLE capsule_observations (
  capsule_id TEXT NOT NULL,
  observation_id TEXT NOT NULL,
  seq INTEGER NOT NULL,      -- Ordering
  PRIMARY KEY (capsule_id, observation_id),
  FOREIGN KEY (capsule_id) REFERENCES capsules(id),
  FOREIGN KEY (observation_id) REFERENCES observations(id)
);

CREATE TABLE summaries (
  id TEXT PRIMARY KEY,
  capsule_id TEXT NOT NULL UNIQUE,
  content TEXT NOT NULL,
  confidence REAL NOT NULL,
  created_at INTEGER NOT NULL,
  evidence_refs TEXT NOT NULL, -- JSON array of observation IDs
  FOREIGN KEY (capsule_id) REFERENCES capsules(id)
);

CREATE TABLE pins (
  id TEXT PRIMARY KEY,
  target_type TEXT NOT NULL,
  target_id TEXT NOT NULL,
  reason TEXT,
  created_at INTEGER NOT NULL,
  expires_at INTEGER,
  scope_ids TEXT NOT NULL     -- JSON blob
);

-- FTS indexes
CREATE VIRTUAL TABLE observations_fts USING fts5(
  content,
  content=observations,
  content_rowid=rowid
);

CREATE VIRTUAL TABLE summaries_fts USING fts5(
  content,
  content=summaries,
  content_rowid=rowid
);
```

## Data Flow Examples

### Example 1: Session Capture

```typescript
// 1. Session starts
const capsule = await core.openCapsule({
  type: 'session',
  intent: 'Fix authentication bug',
  scopeIds: { sessionId: 's1', repoId: '/repo' },
});

// 2. Tool call happens
await core.appendObservation({
  id: 'obs1',
  kind: 'tool_call',
  content: JSON.stringify({ tool: 'grep', pattern: 'auth' }),
  provenance: { toolName: 'grep' },
  ts: Date.now(),
  scopeIds: { sessionId: 's1', repoId: '/repo' },
  redacted: false,
});

// 3. File edit happens
await core.appendObservation({
  id: 'obs2',
  kind: 'file_diff',
  content: '+ fixed auth check',
  provenance: { filePath: '/src/auth.ts' },
  ts: Date.now(),
  scopeIds: { sessionId: 's1', repoId: '/repo' },
  redacted: false,
});

// 4. Session ends
await core.closeCapsule(capsule.id, {
  content: 'Fixed authentication bug by updating auth check',
  confidence: 0.9,
  evidenceRefs: ['obs1', 'obs2'],
});
```

### Example 2: Retrieval

```typescript
// Query: "authentication"
const results = await core.retrieve({
  query: 'authentication',
  scopeIds: { repoId: '/repo' },
  tokenBudget: 8000
});

// Result structure:
{
  pins: [
    // Active pins (non-evictable)
    { targetType: 'observation', targetId: 'obs1', ... }
  ],
  currentSummary: {
    // Latest summary for open capsule (if any)
    content: 'Working on authentication bug...',
    capsuleId: 'cap1',
    confidence: 0.8
  },
  candidates: [
    // FTS-ranked results
    {
      observation: { id: 'obs2', content: '...', kind: 'file_diff', ... },
      score: 0.95
    },
    {
      observation: { id: 'obs3', content: '...', kind: 'tool_call', ... },
      score: 0.87
    }
  ],
  provenance: {
    query: 'authentication',
    scopeIds: { repoId: '/repo' },
    totalCandidates: 15,
    returnedCandidates: 10,
    truncatedDueToTokenBudget: true
  }
}
```

### Example 3: Redaction

```typescript
// Redact sensitive observation
await store.redactObservation('obs5');

// After redaction:
{
  id: 'obs5',
  kind: 'tool_call',
  content: '[redacted]',
  provenance: { toolName: 'read_file' },
  ts: 1234567890,
  scopeIds: { sessionId: 's1' },
  redacted: true
}

// FTS search no longer returns obs5
// But capsule.observationIds still includes 'obs5' (provenance preserved)
```

## Constraints and Invariants

### Immutability

- Observations: Immutable except `redacted` flag
- Capsules: Immutable except `status` and `closedAt`
- Summaries: Fully immutable
- Pins: Immutable (delete to remove)

### Ordering

- `observationIds` in Capsule: Insertion order (deterministic)
- Retrieval results: Pins → Summary → Candidates (tiered)
- Export: Deterministic ordering by timestamp

### Referential Integrity

- Capsule references in Summary must exist
- Observation references in `capsule_observations` must exist
- Pin targets must exist
- Redaction preserves references (tombstone, don't delete)

### Scope Consistency

- All observations in a capsule share the capsule's `scopeIds`
- Pins inherit scope from creation context
- Retrieval filters apply to all tiers (pins, summaries, candidates)

## Evolution and Versioning

### Schema Versioning

- Schema version tracked in `schema_migrations` table
- Migrations are additive only (no destructive changes)
- Export bundles include version for forward compatibility

### Type Extensions (Future)

- Additional `ObservationKind` values can be added
- Additional `CapsuleType` values can be added
- `provenance` is unstructured (extensible by adapters)
- `ScopeIds` can add new dimensions (e.g., `organizationId`)

## Design Decisions

1. **JSON for nested data** — `scopeIds`, `provenance`, `evidenceRefs` stored as JSON blobs (SQLite TEXT)
2. **No cascading deletes** — Redaction over deletion; preserve provenance
3. **No polymorphism** — Flat types; discriminated unions (e.g., `ObservationKind`)
4. **No computed fields** — All fields explicitly stored; no magic
5. **Deterministic ordering** — All lists have defined order (insertion, timestamp)
