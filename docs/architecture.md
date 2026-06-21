# Architecture

## Overview

kindling is a local-first memory and continuity engine for AI-assisted development. It captures observations from development sessions, organizes them into capsules (bounded units of meaning), and provides deterministic, explainable retrieval.

kindling is Rust-canonical: the engine is a workspace of seven Rust crates, fronted by a local daemon (`kindling serve`) that speaks HTTP/1 over a Unix domain socket. The daemon makes concurrent multi-tool access safe — many adapters, hooks, and CLI invocations can talk to one engine without fighting over the database.

> The canonical published docs live at **docs.eddacraft.ai/kindling**.
>
> The npm package `@eddacraft/kindling` is a **thin HTTP-over-UDS client** for the Rust `kindling` binary. It ships the binary via per-platform `optionalDependencies` (`@eddacraft/kindling-<os>-<arch>[-musl]`, one prebuilt binary each with `os`/`cpu`/`libc` fields), so a plain `npm install` lands a working binary with no postinstall; `$KINDLING_BIN` or a `kindling` on `PATH` (from `cargo install eddacraft-kindling` or the install script) override it. The client auto-spawns the resolved binary on first use. The legacy TypeScript implementation packages (`-core`, `-store-sqlite`, `-store-sqljs`, `-provider-local`, `-server`, `-cli`) are deprecated and will be removed at 1.0.0.

The architecture is deliberately layered to separate concerns:

- **Types** - shared domain model
- **Storage** - persistence and schema
- **Retrieval** - ranking and search
- **Service** - domain logic and orchestration
- **Server** - the daemon runtime (HTTP/1 over UDS)
- **Client** - daemon-backed SDK
- **Binary** - CLI, hooks, and `serve`

## Design Principles

1. **Local-first** - No external services; embedded SQLite
2. **Deterministic** - Same query, same context produces same results
3. **Explainable** - Every result has provenance
4. **Concurrency-safe** - A single daemon serializes access; concurrent tools share one engine
5. **Infrastructure, not truth** - kindling captures what happened; it does not assert organizational authority

## System Diagram

```
                         Producers
   ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  ┌──────────┐
   │  OpenCode    │  │  Claude Code │  │  PocketFlow Nodes    │  │  CLI /   │
   │  Adapter     │  │  hooks       │  │  Adapter             │  │  scripts │
   └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘  └────┬─────┘
          │                 │                     │                   │
          └─────────────────┴──────────┬──────────┴───────────────────┘
                                        │
                            kindling-client (Rust SDK)
                       HTTP/1 over UDS  +  X-Kindling-Project
                                        │
                                        ▼
     ┌──────────────────────────────────────────────────────────┐
     │  kindling serve  →  kindling-server (daemon runtime)       │
     │  - PID lock, socket mode 0600, idle shutdown               │
     │  - Per-project routing (one service cached per project)    │
     │  - v1 HTTP API (camelCase JSON)                            │
     │                                                            │
     │            ┌───────────────────────────────┐              │
     │            │  kindling-service             │◄──── embedded
     │            │  - Observation ingestion      │      (zero IPC)
     │            │  - Capsule lifecycle          │
     │            │  - Retrieval orchestration    │
     │            │  - Secret masking on append   │
     │            └───────┬───────────────┬───────┘              │
     │                    ▼               ▼                      │
     │          kindling-store     kindling-provider             │
     │          - Observations     - FTS5 BM25 ranking           │
     │          - Capsules         - Recency scoring             │
     │          - Summaries        - Deterministic combine       │
     │          - Pins                                           │
     │          - FTS5 + WAL                                     │
     │                    └───────┬───────┘                      │
     │                            ▼                              │
     │                    SQLite (per-project DB)                │
     └──────────────────────────────────────────────────────────┘
```

Two integration paths share the same core:

- **Daemon path** (default) — `kindling-client` auto-spawns the daemon on first use and talks to `kindling-server` over UDS. Use this when more than one tool may touch memory concurrently.
- **Embedded path** — `kindling-service` runs the engine in-process with zero IPC. Use this for single-owner, embedded scenarios.

## Crates

The workspace is versioned at `v0.1.0`.

| Crate                | Layer         | Depends on             | Use when                                            |
| -------------------- | ------------- | ---------------------- | --------------------------------------------------- |
| `kindling-types`     | domain        | —                      | always (shared types)                               |
| `kindling-store`     | storage       | types                  | direct persistence                                  |
| `kindling-provider`  | retrieval     | types, store           | direct retrieval                                    |
| `kindling-service`   | orchestration | types, store, provider | embedded, in-process                                |
| `kindling-server`    | daemon        | types, store, service  | running the daemon                                  |
| `kindling-client`    | SDK           | types                  | most integrations                                   |
| `eddacraft-kindling` | binary        | all                    | CLI, hooks, `serve` (installed command: `kindling`) |

### `kindling-types`

**Purpose:** Shared domain types.

**Responsibilities:**

- Define `Observation`, `ObservationKind`, `Capsule`, `CapsuleType`, `CapsuleStatus`, `ScopeIds`, summaries, and pins.
- Optional `ts-rs` feature generates checked-in TypeScript bindings under `crates/kindling-types/bindings/`. CI fails on drift.

**Dependencies:** None.

### `kindling-store`

**Purpose:** SQLite persistence.

**Responsibilities:**

- FTS5 indexing with WAL mode for concurrent reads.
- Schema v5, governed by the cross-language contract (`schema/schema.sql` + `schema/version.json`).
- Per-project database under `~/.kindling/projects/<hash>/`.

**Dependencies:** `kindling-types`.

### `kindling-provider`

**Purpose:** Deterministic local retrieval.

**Responsibilities:**

- FTS5 BM25 relevance, normalized to `[0,1]` across entity types.
- Recency scoring with a 30-day window.
- Final score: `fts_relevance * 0.7 + recency * 0.3`, where `recency = MAX(0, 1 - age_ms/max_age_ms)`.

**Dependencies:** `kindling-types`, `kindling-store`.

### `kindling-service`

**Purpose:** In-process / embedded orchestration (zero IPC).

**Responsibilities:**

- Manage capsule lifecycle (open/close).
- Append observations; **mask secrets at the service boundary on append**.
- Orchestrate retrieval (combine pins, summary, and provider hits).

**Dependencies:** `kindling-types`, `kindling-store`, `kindling-provider`.

**Use when:** Building a single-owner embedded integration that does not need cross-process concurrency.

### `kindling-server`

**Purpose:** The daemon runtime.

**Responsibilities:**

- HTTP/1 over UDS (TCP loopback fallback on Windows).
- Per-project routing via the `X-Kindling-Project` header; one service instance cached per project.
- PID lock, socket mode `0600`, idle shutdown (default 30 minutes).

**Dependencies:** `kindling-types`, `kindling-store`, `kindling-service`.

**v1 endpoints** (camelCase JSON; `X-Kindling-Project` required on all data endpoints, not on health):

| Method | Path                          | Purpose                              |
| ------ | ----------------------------- | ------------------------------------ |
| GET    | `/v1/health`                  | liveness (no project header)         |
| POST   | `/v1/capsules`                | open a capsule                       |
| GET    | `/v1/capsules/open?sessionId` | fetch the open capsule for a session |
| PATCH  | `/v1/capsules/:id/close`      | close a capsule                      |
| POST   | `/v1/observations`            | append an observation                |
| POST   | `/v1/observations/:id/forget` | forget an observation                |
| POST   | `/v1/retrieve`                | orchestrated retrieval               |
| POST   | `/v1/pins`                    | create a pin                         |
| DELETE | `/v1/pins/:id`                | remove a pin                         |
| POST   | `/v1/context/session-start`   | session-start context                |
| POST   | `/v1/context/pre-compact`     | pre-compact context                  |

### `kindling-client`

**Purpose:** Daemon-backed Rust SDK (the default for integrations).

**Responsibilities:**

- Thin async HTTP/1-over-UDS client; auto-spawns the daemon on first use.
- TCP loopback fallback on Windows.
- Sends the `X-Kindling-Project` header for per-project routing.
- Checks the daemon schema version on connect.
- Re-exports domain types from `kindling-types`.

**Dependencies:** `kindling-types`.

**Use when:** Most integrations — anywhere concurrent multi-tool access is possible.

### `eddacraft-kindling` (binary)

**Purpose:** CLI binary plus a thin lib surface. Published as the
`eddacraft-kindling` crate (the name `kindling` is taken on crates.io by an
unrelated project); the installed command and library are still `kindling`.

**Responsibilities:**

- Inspection, debugging, export/import, and running the daemon.
- Hook entrypoints for Claude Code lifecycle events.

**Dependencies:** All other crates.

**Key subcommands:**

```
kindling init
kindling log
kindling search <query>
kindling capsule open | close
kindling status
kindling list
kindling pin    | unpin
kindling forget <observation-id>
kindling export | import
kindling serve  [--socket ~/.kindling/kindling.sock]
                [--idle-timeout 1800] [--daemonize] [--kindling-home <dir>]
kindling hook   <type>
```

**Hook types:** `session-start`, `post-tool-use`, `post-tool-use-failure`, `user-prompt-submit`, `subagent-stop`, `stop`, `pre-compact`.

## Data Flow

### Ingestion (Write Path)

```
1. Producer emits an event (e.g., tool call from a Claude Code hook)
2. Producer maps the event to an Observation
3. Producer appends it:
     - daemon path:    kindling-client → POST /v1/observations
     - embedded path:  kindling-service append
4. Service validates the observation and masks secrets on append
5. Service writes to the store
6. Store persists to SQLite + FTS5 index
```

### Retrieval (Read Path)

```
1. Producer requests memory (e.g., session-start context)
2. Producer retrieves:
     - daemon path:    kindling-client → POST /v1/retrieve
     - embedded path:  kindling-service retrieve
3. Service orchestrates:
   a. Fetch pins from the store (non-evictable)
   b. Fetch the latest summary for the open capsule (non-evictable)
   c. Query the provider for FTS5 + recency candidates
   d. Combine + tier results (pins/summary at top)
   e. Bound the candidate set with `max_candidates` (token-budget assembly is a downstream responsibility)
4. Service returns the retrieval response with provenance
5. Producer formats it for the end user
```

### Capsule Lifecycle

```
1. Session/workflow starts -> open a capsule (POST /v1/capsules or service open)
2. Service creates the Capsule record in the store (status=open)
3. Events occur -> append observations (POST /v1/observations or service append)
4. Service attaches observations to the open capsule
5. Session/workflow ends -> close the capsule (PATCH /v1/capsules/:id/close)
6. Service optionally generates a summary
7. Service marks the Capsule as closed (status=closed)
```

## Domain Model

**ObservationKind** (9): `tool_call`, `command`, `file_diff`, `error`, `message`, `node_start`, `node_end`, `node_output`, `node_error`.

**CapsuleType:** `session`, `pocketflow_node`.

**CapsuleStatus:** `open`, `closed`.

## Scoping and Multi-tenancy

kindling supports scoped queries via `ScopeIds` (all five optional):

- `sessionId` - isolate by session (e.g., an OpenCode or Claude Code session)
- `repoId` - isolate by repository
- `agentId` - isolate by agent
- `userId` - isolate by user
- `taskId` - tag observations with a task

Scope IDs are denormalized into store columns for filtering — **except `taskId`**, which has no denormalized column and is intentionally **not retrieval-filterable**. It is carried for provenance only.

All queries accept optional scope filters. Default behavior:

- Retrieval: scoped to the current session + repo
- Export: can export by scope or global

The daemon adds a project axis on top of scoping: every data request carries `X-Kindling-Project`, and the server caches one service per project, each backed by its own per-project DB.

## Redaction and Privacy

There are two layers of protection:

1. **Automatic masking** — `kindling-service` masks secrets at the service boundary on every append, so secrets never reach storage.
2. **Explicit forget** — users can forget a specific observation after the fact:

   ```bash
   kindling forget <observation-id>
   ```

   over the daemon, this maps to:

   ```
   POST /v1/observations/:id/forget
   ```

Forgetting an observation:

- Clears content to `[redacted]`
- Removes it from the FTS index
- Preserves provenance (observation ID, capsule relationship)

## Export and Import

Export produces JSON bundles with deterministic ordering:

```json
{
  "version": "0.1.0",
  "exportedAt": 1234567890,
  "scopeFilter": { "repoId": "abc" },
  "observations": [],
  "capsules": [],
  "summaries": [],
  "pins": []
}
```

Import supports conflict strategies:

- `skip` - preserve existing
- `overwrite` - replace existing
- `error` - fail on conflict

Round-trip guarantees: `export -> import -> export` produces identical data.

## Schema and Migrations

kindling is at **schema v5**, governed by a **cross-language schema contract** so the Rust engine and any language bindings agree on the layout:

- `schema/schema.sql` - canonical schema
- `schema/version.json` - schema version (`5`)
- FTS5 tokenizer: `porter unicode61`

**Tables:** `observations`, `capsules`, `capsule_observations`, `summaries`, `pins`, `observations_fts`, `summaries_fts`, `schema_migrations`.

Scope IDs are denormalized into columns on the relevant tables (see Scoping). Migrations are applied on database open, additive, and idempotent; the `schema_migrations` table tracks applied versions.

## Configuration

kindling reads its home directory and daemon settings from the environment and the `serve` flags:

| Setting        | Default                        | Source                              |
| -------------- | ------------------------------ | ----------------------------------- |
| Home directory | `~/.kindling`                  | `KINDLING_HOME` / `--kindling-home` |
| Per-project DB | `~/.kindling/projects/<hash>/` | derived from project                |
| Socket         | `~/.kindling/kindling.sock`    | `kindling serve --socket`           |
| Idle timeout   | `1800` seconds (30 min)        | `kindling serve --idle-timeout`     |
| Daemonize      | off                            | `kindling serve --daemonize`        |

On Windows the daemon falls back to TCP loopback in place of the UDS.

## Non-Goals (Explicit Out of Scope)

- **Organizational truth** - kindling does not promote or curate memory
- **Governance workflows** - No approval, conflict resolution, lifecycle management
- **Multi-user access control** - Local-first; single-user by default
- **Cloud/remote modes** - The daemon is local-only (UDS, or loopback on Windows)
- **Semantic retrieval** - FTS5 + recency only (no embeddings in v0.1)

These concerns belong to downstream systems (e.g., Edda).

## Future Considerations (Not M1)

- Embedding-based retrieval (provider plugins)
- Capsule hierarchy (parent/child relationships)
- Streaming export/import for large datasets
- Compression for long-term storage
- Multi-user sync (if ever required)
