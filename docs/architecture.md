# Architecture

## Overview

kindling is a local-first memory and continuity engine for AI-assisted development. It captures observations from development sessions, organizes them into capsules (bounded units of meaning), and provides deterministic, explainable retrieval.

The architecture is deliberately layered to separate concerns:

- **Storage** - persistence and schema
- **Retrieval** - ranking and search
- **Core** - domain logic and orchestration
- **Adapters** - ingestion from external systems
- **CLI** - inspection and debugging tools

## Design Principles

1. **Local-first** - No external services; embedded SQLite
2. **Deterministic** - Same query, same context produces same results
3. **Explainable** - Every result has provenance
4. **Infrastructure, not truth** - kindling captures what happened; it does not assert organizational authority

## System Diagram

```
                         Adapters
   ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐
   │  OpenCode    │  │  Claude Code │  │  PocketFlow Nodes    │
   │  Adapter     │  │  Adapter     │  │  Adapter             │
   └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘
          │                 │                     │
          └─────────────────┴─────────────────────┘
                    ▼
     ┌──────────────────────────────────────────┐
     │  @eddacraft/kindling (Main Package)      │
     │                                          │
     │  ┌────────────────────────────────────┐  │
     │  │  kindling Core                     │  │
     │  │  - Observation ingestion           │  │
     │  │  - Capsule lifecycle               │  │
     │  │  - Retrieval orchestration         │  │
     │  │  - Export/import                   │  │
     │  └──────────┬─────────────────────────┘  │
     │             │                            │
     │  ┌──────────┴──────────────┐             │
     │  ▼                         ▼             │
     │  Storage (SQLite)   Provider (Local)     │
     │  - Observations     - FTS ranking        │
     │  - Capsules         - Recency scoring    │
     │  - Summaries        - Candidate          │
     │  - Pins               generation         │
     │  - FTS indexes                           │
     │  └──────────┬──────────┘                 │
     │             ▼                            │
     │  API Server (Fastify)                    │
     └──────────────────────────────────────────┘
                    │
                    ▼
     ┌──────────────────────────────────────────┐
     │                    CLI                   │
     │  Inspection - Debugging - Export/Import  │
     └──────────────────────────────────────────┘
```

## Packages

### `@eddacraft/kindling` (Main Package)

**Purpose:** All-in-one package for Node.js users

**Bundles:**

- `@eddacraft/kindling-core` (types, KindlingService)
- SQLite persistence (better-sqlite3, FTS5, WAL mode)
- Local FTS provider (retrieval with ranking)
- API server (Fastify)

**This is what most users install.** It re-exports core types and provides the complete stack.

**Key Exports:**

- `KindlingService` - orchestration (from core)
- `openDatabase`, `SqliteKindlingStore` - persistence
- `LocalFtsProvider` - retrieval
- API server entrypoint

**Configuration:**

- Default DB location: `~/.kindling/kindling.db`
- Overridable via environment or explicit path

### `@eddacraft/kindling-core`

**Purpose:** Lightweight domain model and orchestration

**Responsibilities:**

- Define core types (Observation, Capsule, Summary, Pin)
- Manage capsule lifecycle (open/close)
- Orchestrate retrieval (combine pins, summaries, provider results)
- Coordinate export/import at the service level

**Dependencies:** None (defines interfaces only)

**Use when:** Building adapters, or targeting browser environments where you pair this with `@eddacraft/kindling-store-sqljs`.

**Key Interfaces:**

- `appendObservation(obs)` - record an observation
- `openCapsule(type, intent, scopeIds)` - start a new capsule
- `closeCapsule(id, summary?)` - finalize a capsule
- `retrieve(query, scopeIds, opts)` - orchestrated retrieval

### `@eddacraft/kindling-store-sqljs`

**Purpose:** Browser/WASM-compatible store

**Responsibilities:**

- sql.js-backed persistence for browser environments
- Same store interface as the SQLite store bundled in the main package

**Dependencies:**

- `@eddacraft/kindling-core`

### `@eddacraft/kindling-adapter-opencode`

**Purpose:** OpenCode session integration

**Responsibilities:**

- Map OpenCode tool calls, diffs, messages to Observations
- Manage session-level capsules (open on session start, close on end)
- Provide `/memory` command surface for OpenCode

**Dependencies:**

- `@eddacraft/kindling-core`

**Key Interfaces:**

- `onSessionStart(sessionId, repoId)` - open capsule
- `onToolCall(tool, args, result)` - append observation
- `onSessionEnd(sessionId)` - close capsule

### `@eddacraft/kindling-adapter-pocketflow`

**Purpose:** PocketFlow workflow integration

**Responsibilities:**

- Map workflow nodes to Observations
- Manage node-level capsules (open on node start, close on node end)
- Capture structured evidence (inputs, outputs, errors)

**Dependencies:**

- `@eddacraft/kindling-core`

**Key Interfaces:**

- `onNodeStart(nodeId, intent)` - open capsule
- `onNodeOutput(nodeId, output)` - append observation
- `onNodeEnd(nodeId)` - close capsule

### `@eddacraft/kindling-adapter-claude-code`

**Purpose:** Claude Code hooks integration

**Responsibilities:**

- Capture Claude Code tool calls and session activity
- Manage session-level capsules via Claude Code hooks

**Dependencies:**

- `@eddacraft/kindling-core`

### `@eddacraft/kindling-cli`

**Purpose:** Inspection, debugging, and standalone use

**Responsibilities:**

- List capsules, observations, pins
- Query FTS directly
- Export/import for backup and portability
- Redact sensitive content

**Dependencies:**

- `@eddacraft/kindling` (main package)

**Key Commands:**

- `kindling list capsules [--scope sessionId] [--after ts]`
- `kindling search <query> [--scope repoId]`
- `kindling export [--scope] [--after] [--before] > backup.json`
- `kindling import < backup.json`
- `kindling redact <observation-id>`

## Data Flow

### Ingestion (Write Path)

```
1. Adapter receives event (e.g., tool call from OpenCode)
2. Adapter maps event to Observation
3. Adapter calls core.appendObservation(obs)
4. Core validates observation
5. Core writes to Store
6. Store persists to SQLite + FTS index
```

### Retrieval (Read Path)

```
1. Adapter requests memory (e.g., /memory command)
2. Adapter calls core.retrieve(query, scopeIds)
3. Core orchestrates:
   a. Fetch pins from Store (non-evictable)
   b. Fetch latest summary for open capsule (non-evictable)
   c. Query Provider for FTS candidates
   d. Combine + tier results (pins/summary at top)
   e. Truncate to token budget (if provided)
4. Core returns retrieval response with provenance
5. Adapter formats for end user (e.g., OpenCode context)
```

### Capsule Lifecycle

```
1. Session/workflow starts -> Adapter calls core.openCapsule()
2. Core creates Capsule record in Store (status=open)
3. Events occur -> Adapter calls core.appendObservation()
4. Core attaches observations to open capsule
5. Session/workflow ends -> Adapter calls core.closeCapsule()
6. Core optionally generates summary
7. Core marks Capsule as closed (status=closed)
```

## Scoping and Multi-tenancy

kindling supports scoped queries via `ScopeIds`:

- `sessionId` - isolate by session (e.g., OpenCode session)
- `repoId` - isolate by repository
- `agentId` - isolate by agent (future)
- `userId` - isolate by user (future)

All queries accept optional scope filters. Default behavior:

- Retrieval: scoped to current session + repo
- Export: can export by scope or global

## Redaction and Privacy

Users can redact sensitive observations:

```typescript
store.redactObservation(id);
```

Redaction:

- Clears content to `[redacted]`
- Sets `redacted=true` flag
- Removes from FTS index
- Preserves provenance (observation ID, capsule relationship)

Tombstones (optional):

- Mark observation as deleted
- Exclude from all queries except audit
- Preserve referential integrity

## Export and Import

Export produces JSON bundles with deterministic ordering:

```json
{
  "version": "0.1.0",
  "exported_at": 1234567890,
  "scope_filter": { "repoId": "abc" },
  "observations": [...],
  "capsules": [...],
  "summaries": [...],
  "pins": [...]
}
```

Import supports conflict strategies:

- `skip` - preserve existing
- `overwrite` - replace existing
- `error` - fail on conflict

Round-trip guarantees: `export -> import -> export` produces identical data.

## Migrations and Evolution

Schema changes are migration-based:

```
migrations/
  001_init.sql          # Core tables
  002_fts.sql           # FTS tables
  003_indexes.sql       # Query indexes
  schema_migrations     # Tracking table
```

**Migration rules:**

- Additive only (never destructive)
- Applied on DB open
- Idempotent (safe to re-run)

## Configuration

kindling is configured via:

1. **Environment variables**
   - `KINDLING_DB_PATH` - override default DB location
   - `KINDLING_LOG_LEVEL` - debug logging

2. **Explicit options** (per-package)
   - `openDatabase(path)` - explicit DB path
   - `retrieve(query, scopeIds, { tokenBudget })` - retrieval options

## Non-Goals (Explicit Out of Scope)

- **Organizational truth** - kindling does not promote or curate memory
- **Governance workflows** - No approval, conflict resolution, lifecycle management
- **Multi-user access control** - Local-first; single-user by default
- **Cloud/server modes** - Embedded only
- **Semantic retrieval** - FTS + recency only (no embeddings in v0.1)

These concerns belong to downstream systems (e.g., Edda).

## Future Considerations (Not M1)

- Embedding-based retrieval (provider plugins)
- Capsule hierarchy (parent/child relationships)
- Streaming export/import for large datasets
- Compression for long-term storage
- Multi-user sync (if ever required)
