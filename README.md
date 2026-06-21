# kindling

**Local-first memory and continuity for AI-assisted development**

kindling gives AI coding tools a memory of what happened: tool calls, file edits, commands, errors, decisions, summaries and pinned findings. It stores that context locally, organises it into meaningful sessions, and retrieves it later with deterministic, explainable results.

Use kindling from the CLI, as a daemon-backed Rust SDK, or as an embedded in-process service.

- **Local-first:** project memory is stored locally in SQLite.
- **Deterministic retrieval:** pins, current summaries and ranked provider hits are returned in a predictable order.
- **Built for AI coding workflows:** Claude Code hooks today, with a crate-level API for other tools and agents.
- **Public docs:** [docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)

[![crates.io](https://img.shields.io/crates/v/eddacraft-kindling.svg)](https://crates.io/crates/eddacraft-kindling)
[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling.svg)](https://www.npmjs.com/package/@eddacraft/kindling)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

## Documentation

The full guide lives at **[docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)**.

Start here:

- [Install kindling](https://docs.eddacraft.ai/kindling/quickstart/install)
- [Core concepts](https://docs.eddacraft.ai/kindling/concepts/capsules)
- [CLI reference](https://docs.eddacraft.ai/kindling/reference/cli)
- [Configuration](https://docs.eddacraft.ai/kindling/reference/config)
- [Writing adapters](https://docs.eddacraft.ai/kindling/adapters/custom)

## Quick Start: Claude Code

The fastest way to use kindling — automatic memory for every Claude Code session.

```bash
# Install and set up in one step (downloads the prebuilt binary, no toolchain needed)
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh

# Or, if you have Rust:
cargo install eddacraft-kindling && kindling init --claude-code
```

That's it. kindling now captures your Claude Code sessions automatically — tool calls, file edits, commands, errors — all searchable across sessions.

**Manual setup:** If you prefer to configure hooks yourself, add to `.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [{ "type": "command", "command": "kindling-hook session-start" }],
    "PostToolUse": [{ "type": "command", "command": "kindling-hook post-tool-use" }],
    "Stop": [{ "type": "command", "command": "kindling-hook stop" }]
  }
}
```

## Install

### Prebuilt binary (recommended)

The one-line installer downloads the prebuilt `kindling` binary for your platform —
no Node.js or Rust toolchain required:

```bash
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
```

### Rust / Cargo

The CLI is published as the [`eddacraft-kindling`](https://crates.io/crates/eddacraft-kindling)
crate (the installed binary is `kindling`):

```bash
cargo install eddacraft-kindling
```

Then:

```bash
kindling init
kindling log "JWT tokens expire after 15 minutes, not 1 hour"
kindling search "JWT"
kindling serve
```

For Rust applications, prefer the daemon-backed client:

```toml
[dependencies]
kindling-client = "0.1"
```

Or use the in-process service when you explicitly want embedded, single-process access:

```toml
[dependencies]
kindling-service = "0.1"
```

Full setup guide: [docs.eddacraft.ai/kindling/quickstart/install](https://docs.eddacraft.ai/kindling/quickstart/install)

### Node.js / npm

The canonical CLI is the Rust binary. The one-line installer above downloads the
prebuilt binary and sets up the Claude Code integration — no Node.js or Rust
toolchain required.

For Node.js applications, install the thin client library — it talks to the same
Rust daemon over a Unix domain socket (Node.js >= 20):

```bash
npm install @eddacraft/kindling     # thin client library
pnpm add @eddacraft/kindling
yarn add @eddacraft/kindling
bun add @eddacraft/kindling
```

> `@eddacraft/kindling` is a thin TypeScript client over the Rust binary. The
> older implementation packages — including the standalone CLI
> `@eddacraft/kindling-cli` — are **deprecated** and will be removed at 1.0.0;
> prefer `cargo install eddacraft-kindling` (or the installer above) for the CLI.

## Which crate should I use?

| Crate                                                               | Use it when                                                                                                                                                 |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`eddacraft-kindling`](https://crates.io/crates/eddacraft-kindling) | You want the CLI binary (installed command: `kindling`): `kindling init`, `kindling log`, `kindling search`, `kindling serve`, or Claude Code hook support. |
| [`kindling-client`](https://crates.io/crates/kindling-client)       | You are building a Rust integration that should talk to the kindling daemon safely across concurrent tools. This is the default SDK choice.                 |
| [`kindling-service`](https://crates.io/crates/kindling-service)     | You need embedded, in-process access to capsule lifecycle, observation capture, retrieval and pins.                                                         |
| [`kindling-server`](https://crates.io/crates/kindling-server)       | You are extending or embedding the daemon/runtime layer. Most users should run `kindling serve` instead.                                                    |
| [`kindling-store`](https://crates.io/crates/kindling-store)         | You are working directly with the SQLite persistence layer. Most applications should use `kindling-client` or `kindling-service`.                           |
| [`kindling-provider`](https://crates.io/crates/kindling-provider)   | You are working on deterministic local retrieval and ranking.                                                                                               |
| [`kindling-types`](https://crates.io/crates/kindling-types)         | You need shared domain types directly. Most client users get these re-exported from `kindling-client`.                                                      |

## CLI Usage

The CLI is both a reader and a writer and you can capture observations manually, not just search for them.

```bash
# Initialize kindling (creates database, optionally sets up Claude Code)
kindling init
kindling init --claude-code

# --- Write: capture context from the command line ---

# Log an observation directly
kindling log "JWT tokens expire after 15 minutes, not 1 hour"
kindling log --kind error "segfault in auth middleware after upgrade"

# Open/close capsules for manual sessions
kindling capsule open --intent "investigating memory leak" --repo ./my-project
kindling capsule close cap_abc123 --summary "root cause: unbounded cache in SessionStore"

# --- Read: search and inspect your memory ---

# Search across all captured context
kindling search "authentication error"
kindling search "auth" --session session-123 --repo ./my-project

# List entities
kindling list capsules
kindling list capsules --status open
kindling list observations --kind error

# Show database status
kindling status

# Pin important findings (always returned first in searches)
kindling pin observation obs_abc123 --note "Root cause identified"
kindling pin observation obs_abc123 --ttl 7d

# Inspect details
kindling inspect observation obs_abc123
kindling inspect capsule cap_xyz789

# Export / import
kindling export ./backup.json
kindling import ./backup.json

# Start API server (for multi-agent access)
kindling serve --port 3000
```

## How It Works

kindling organises memory into two layers:

**Observations** — atomic units of captured context (tool calls, commands, file diffs, errors, messages). These flow in automatically from adapters or manually via the CLI.

**Capsules** — bounded groups of observations (a session, a workflow run). Each capsule has an intent, a lifecycle (open/close), and a summary.

When you search, kindling returns results in three tiers:

1. **Pins** — user-marked priority items (always first, non-evictable)
2. **Current Summary** — active session context
3. **Provider Hits** — ranked FTS results with provenance ("why was this returned?")

## Adapters

kindling captures context automatically through adapters:

| Adapter                                              | What it captures                                                      |
| ---------------------------------------------------- | --------------------------------------------------------------------- |
| [Claude Code](#quick-start-claude-code)              | Tool calls, file edits, commands, user messages, subagent completions |
| [OpenCode](./packages/kindling-adapter-opencode)     | Session events and tool activity                                      |
| [PocketFlow](./packages/kindling-adapter-pocketflow) | Workflow node lifecycle and outputs                                   |

Claude Code support is built into the `kindling` binary (hooks — see
[Quick Start](#quick-start-claude-code) above). OpenCode and PocketFlow are thin
TypeScript adapters that talk to the kindling daemon through the
[`@eddacraft/kindling`](./packages/kindling) client (published to npm).

Or capture manually with the CLI (`kindling log`, `kindling capsule open/close`).

## Programmatic Usage

kindling is Rust-canonical. For most integrations, use the daemon-backed client
([`kindling-client`](https://crates.io/crates/kindling-client)) — it speaks to
the daemon started by `kindling serve`, so several tools can share the same
project memory safely:

```rust
use kindling_client::{Client, CapsuleType, ObservationInput, ObservationKind, RetrieveOptions, ScopeIds};

#[tokio::main]
async fn main() -> Result<(), kindling_client::ClientError> {
    // Auto-spawns the daemon on first use.
    let client = Client::new()?;

    let scope = ScopeIds { session_id: Some("session-1".into()), ..Default::default() };

    // Open a session capsule.
    let capsule = client
        .open_capsule(CapsuleType::Session, "debug authentication issue", scope.clone(), None)
        .await?;

    // Capture an observation.
    client
        .append_observation(
            ObservationInput {
                id: None,
                kind: ObservationKind::Error,
                content: "JWT validation failed: token expired".into(),
                provenance: None,
                ts: None,
                scope_ids: scope.clone(),
                redacted: None,
            },
            Some(capsule.id.clone()),
            Some(true),
        )
        .await?;

    // Search.
    let results = client
        .retrieve(RetrieveOptions {
            query: "authentication token".into(),
            scope_ids: scope,
            token_budget: None,
            max_candidates: None,
            include_redacted: None,
        })
        .await?;
    println!("{} candidates", results.candidates.len());

    // Close with a summary.
    client.close_capsule(&capsule.id, Default::default()).await?;
    Ok(())
}
```

For embedded, single-process use (no daemon, zero IPC), use
[`kindling-service`](https://crates.io/crates/kindling-service) instead — it
exposes the same capsule/observation/retrieval/pin operations in-process.

> **Node.js:** `@eddacraft/kindling` is a thin client over the same Rust binary.
> The older TypeScript implementation packages (`-core`, `-store-sqlite`,
> `-store-sqljs`, `-provider-local`, `-server`, `-cli`) are **deprecated** and
> will be removed at 1.0.0.

## Architecture

Rust is the engine. Adapters, the CLI, and Claude Code hooks reach project
memory through the daemon (`kindling serve`, HTTP/1 over a Unix domain socket),
which owns the local SQLite store and keeps concurrent access safe. Integrations
that want embedded access skip the daemon and link `kindling-service` directly.

```diagram
        Adapters / hooks                CLI
  ┌──────────────────────────┐   ┌──────────────┐
  │ OpenCode · Claude Code · │   │  kindling    │
  │ PocketFlow               │   │  <command>   │
  └────────────┬─────────────┘   └──────┬───────┘
               │  kindling-client       │  in-process
               │  (HTTP/1 over UDS)     │  (kindling-service)
               ▼                        │
   ┌──────────────────────────┐         │
   │  kindling-server          │         │
   │  (daemon: kindling serve) │         │
   │  per-project routing      │         │
   └────────────┬─────────────┘         │
                ▼                        ▼
        ┌──────────────────────────────────┐
        │  kindling-service                 │
        │  capsule lifecycle · capture ·    │
        │  retrieval · pins · redaction     │
        └───────┬──────────────────┬────────┘
                ▼                  ▼
         kindling-store     kindling-provider
         (SQLite,FTS5,WAL)  (FTS5 BM25 + recency)
                └────────┬─────────┘
                         ▼
              ┌────────────────────┐
              │  SQLite database   │
              │  (WAL + FTS5,      │
              │   schema v5)       │
              └────────────────────┘
```

See [`docs/architecture.md`](./docs/architecture.md) for the full topology.

## Core Concepts

### Observations

Atomic units of captured context:

| Kind                         | Description                                  |
| ---------------------------- | -------------------------------------------- |
| `tool_call`                  | AI tool invocations (Read, Edit, Bash, etc.) |
| `command`                    | Shell commands with exit codes and output    |
| `file_diff`                  | File changes with paths                      |
| `error`                      | Errors with stack traces                     |
| `message`                    | User/assistant messages                      |
| `node_start` / `node_end`    | Workflow node lifecycle                      |
| `node_output` / `node_error` | Workflow node results                        |

### Capsules

Bounded units of meaning that group observations:

- **`session`** - Interactive development session
- **`pocketflow_node`** - Single workflow node execution

Each capsule has:

- Type and intent (debug, implement, test, etc.)
- Open/close lifecycle with optional summary generation
- Scope (`sessionId`, `repoId`, `agentId`, `userId`, `taskId`)

### Retrieval Tiers

Deterministic, explainable retrieval with 3 tiers:

1. **Pins** - Non-evictable, user-controlled priority content
2. **Current Summary** - Active session/capsule context
3. **Provider Hits** - Ranked FTS results with explainability

## Use Cases

### Session Continuity

Resume work without re-explaining context. Install the Claude Code plugin (or any
adapter) and sessions are captured automatically; later, retrieve what you were
doing:

```bash
kindling search "authentication bug" --session session-1
```

The same retrieval is available from Rust via `kindling-client`:

```rust
let results = client
    .retrieve(RetrieveOptions {
        query: "authentication bug".into(),
        scope_ids: ScopeIds { session_id: Some("session-1".into()), ..Default::default() },
        ..Default::default()
    })
    .await?;
```

### Workflow Memory

Capture high-signal workflow executions. The PocketFlow adapter opens a
`pocketflow_node` capsule per node and records `node_start` / `node_output` /
`node_error` / `node_end` observations through the thin client to the daemon —
see [`docs/pocketflow-capabilities.md`](./docs/pocketflow-capabilities.md).

### Pin Critical Findings

Mark important discoveries for non-evictable retrieval. Pins always appear first,
ahead of ranked results:

```bash
kindling pin observation obs_abc123 --note "Root cause of production outage" --ttl 7d
kindling search "outage"   # the pinned error is returned first
```

From Rust:

```rust
let mut pin = CreatePinBody::new(PinTargetType::Observation, error_obs_id);
pin.note = Some("Root cause of production outage".into());
client.pin(pin).await?;
```

## Design Principles

1. **Capture, Don't Judge** — preserves what happened without asserting truth
2. **Deterministic & Explainable** — retrieval results include "why" explanations
3. **Local-First** — no external services, embedded SQLite
4. **Privacy-Aware** — automatic redaction of secrets, bounded output capture
5. **Provenance Always** — every piece of context points to concrete evidence

## kindling and anvil

kindling captures what happened. anvil governs what should happen.

kindling provides local memory, session continuity and explainable retrieval. anvil builds on that foundation with governed plans, quality gates, provenance and policy enforcement for software teams.

Learn more at [eddacraft.ai](https://eddacraft.ai).

## Development

kindling is Rust-canonical; the Rust workspace is the primary build. The npm
packages (thin client + adapters) live alongside it.

```bash
git clone https://github.com/eddacraft/kindling.git
cd kindling

# Rust workspace (canonical engine)
cargo build
cargo test

# npm packages (thin client + adapters)
pnpm install
pnpm run build
pnpm run test
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full workflow. This project uses
[anvil Plan Spec (APS)](https://github.com/eddacraft/anvil-plan-spec) for planning.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines and [SECURITY.md](SECURITY.md) for responsible disclosure.

## License

Apache 2.0 — See [LICENSE](LICENSE) for details.

---

**Built by the [eddacraft](https://eddacraft.ai) team**
