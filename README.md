# kindling

**Local memory and continuity engine for AI-assisted development**

kindling captures what happens during AI-assisted development — tool calls, diffs, commands, errors — and makes it retrievable across sessions. All data stays local in embedded SQLite.

[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling.svg)](https://www.npmjs.com/package/@eddacraft/kindling)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.3-blue)](https://www.typescriptlang.org/)
[![Node.js](https://img.shields.io/badge/node-%3E%3D20.0.0-brightgreen)](https://nodejs.org/)

## Quick Start: Claude Code

The fastest way to use kindling — automatic memory for every Claude Code session.

```bash
# Install and set up in one step
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh

# Or with npx (no global install)
npx @eddacraft/kindling-cli init --claude-code
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

Node.js >= 20 required. Prebuilt binaries ship for Linux (glibc), macOS (Intel + Apple Silicon), and Windows (x64).

```bash
# Recommended: one-line installer (installs CLI + Claude Code plugin)
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh

# Or install with your preferred package manager
npm install -g @eddacraft/kindling-cli    # CLI (global)
npm install @eddacraft/kindling            # Library (project-local)

pnpm add -g @eddacraft/kindling-cli
pnpm add @eddacraft/kindling

yarn global add @eddacraft/kindling-cli
yarn add @eddacraft/kindling

bun add -g @eddacraft/kindling-cli
bun add @eddacraft/kindling
```

### Platform Notes

<details>
<summary>If prebuilt binaries aren't available for your platform</summary>

kindling uses [better-sqlite3](https://github.com/WiseLibs/better-sqlite3) which needs a C++ compiler to build from source:

- **Debian/Ubuntu:** `sudo apt-get install build-essential python3`
- **Fedora/RHEL:** `sudo dnf groupinstall "Development Tools"`
- **Alpine (musl):** `apk add build-base python3`
- **macOS:** `xcode-select --install`
- **Windows (Admin):** `npm install -g windows-build-tools`

</details>

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

| Adapter                                                | What it captures                                                      |
| ------------------------------------------------------ | --------------------------------------------------------------------- |
| [Claude Code](./packages/kindling-adapter-claude-code) | Tool calls, file edits, commands, user messages, subagent completions |
| [OpenCode](./packages/kindling-adapter-opencode)       | Session events and tool activity                                      |
| [PocketFlow](./packages/kindling-adapter-pocketflow)   | Workflow node lifecycle and outputs                                   |

Or capture manually with the CLI (`kindling log`, `kindling capsule open/close`).

## Packages

| Package                                                                              | Description                                                              |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| [`@eddacraft/kindling`](./packages/kindling)                                         | **Main package**: re-exports core + SQLite store + local FTS provider    |
| [`@eddacraft/kindling-core`](./packages/kindling-core)                               | Domain types, KindlingService, validation (for adapter authors, browser) |
| [`@eddacraft/kindling-store-sqlite`](./packages/kindling-store-sqlite)               | SQLite persistence with FTS5 and WAL mode                                |
| [`@eddacraft/kindling-store-sqljs`](./packages/kindling-store-sqljs)                 | sql.js WASM store for browser compatibility                              |
| [`@eddacraft/kindling-provider-local`](./packages/kindling-provider-local)           | Local FTS-based retrieval provider with deterministic ranking            |
| [`@eddacraft/kindling-server`](./packages/kindling-server)                           | HTTP API server for multi-agent concurrency (Fastify)                    |
| [`@eddacraft/kindling-cli`](./packages/kindling-cli)                                 | CLI tools for inspection, search, and management                         |
| [`@eddacraft/kindling-adapter-opencode`](./packages/kindling-adapter-opencode)       | OpenCode session integration                                             |
| [`@eddacraft/kindling-adapter-pocketflow`](./packages/kindling-adapter-pocketflow)   | PocketFlow workflow integration with intent and confidence tracking      |
| [`@eddacraft/kindling-adapter-claude-code`](./packages/kindling-adapter-claude-code) | Claude Code hooks integration                                            |

## Programmatic Usage

For building on kindling as a library:

```typescript
import { randomUUID } from 'node:crypto';
import {
  KindlingService,
  openDatabase,
  SqliteKindlingStore,
  LocalFtsProvider,
} from '@eddacraft/kindling';

const db = openDatabase({ path: './my-memory.db' });
const store = new SqliteKindlingStore(db);
const provider = new LocalFtsProvider(db);
const service = new KindlingService({ store, provider });

// Open a session capsule
const capsule = service.openCapsule({
  type: 'session',
  intent: 'debug authentication issue',
  scopeIds: { sessionId: 'session-1', repoId: 'my-project' },
});

// Capture observations
service.appendObservation(
  {
    id: randomUUID(),
    kind: 'error',
    content: 'JWT validation failed: token expired',
    provenance: { stack: 'Error: Token expired\n  at validateToken.ts:42' },
    scopeIds: { sessionId: 'session-1' },
    ts: Date.now(),
    redacted: false,
  },
  { capsuleId: capsule.id },
);

// Search
const results = await service.retrieve({
  query: 'authentication token',
  scopeIds: { sessionId: 'session-1' },
});

// Close with summary
service.closeCapsule(capsule.id, {
  generateSummary: true,
  summaryContent: 'Fixed JWT expiration check in token validation middleware',
});

db.close();
```

## Architecture

```diagram
                           Adapters
  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐
  │  OpenCode    │  │  Claude Code │  │  PocketFlow Nodes    │
  │  Sessions    │  │  (Hooks)     │  │  (Workflows)         │
  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘
         │                 │                     │
         └─────────────────┴─────────────────────┘
                   ▼
     ┌──────────────────────────────┐
     │  @eddacraft/kindling         │  ← Main package
     │  ┌────────────────────────┐  │
     │  │  KindlingService       │  │
     │  │  (kindling-core)       │  │
     │  └──────────┬─────────────┘  │
     │             │                │
     │  ┌──────────┴────────────┐   │
     │  ▼                       ▼   │
     │  SqliteStore    LocalFts     │
     │  (persistence)  Provider     │
     │  └──────┬───────┴──────┘     │
     │         ▼                    │
     │  ┌─────────────────────┐     │
     │  │  SQLite Database    │     │
     │  │  (WAL + FTS5)       │     │
     │  └─────────────────────┘     │
     │                              │
     │  API Server (Fastify)        │
     └──────────────────────────────┘
```

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

- **Session** - Interactive development session
- **PocketFlowNode** - Single workflow node execution

Each capsule has:

- Type and intent (debug, implement, test, etc.)
- Open/close lifecycle with automatic summary generation
- Scope (sessionId, repoId, agentId, userId)

### Retrieval Tiers

Deterministic, explainable retrieval with 3 tiers:

1. **Pins** - Non-evictable, user-controlled priority content
2. **Current Summary** - Active session/capsule context
3. **Provider Hits** - Ranked FTS results with explainability

## Use Cases

### Session Continuity

Resume work without re-explaining context:

```typescript
import { SessionManager } from '@eddacraft/kindling-adapter-opencode';

const manager = new SessionManager(store);

// Start session
manager.onSessionStart({
  sessionId: 'session-1',
  intent: 'Fix authentication bug',
  repoId: '/home/user/my-project',
});

// Events flow in automatically...

// Later: retrieve session context
const context = service.retrieve({
  scopeIds: { sessionId: 'session-1' },
});
```

### Workflow Memory

Capture high-signal workflow executions with PocketFlow nodes:

```typescript
import { KindlingNode, KindlingFlow } from '@eddacraft/kindling-adapter-pocketflow';
import type { KindlingNodeContext } from '@eddacraft/kindling-adapter-pocketflow';

// Define a node that auto-captures its lifecycle as observations
class TestRunnerNode extends KindlingNode<KindlingNodeContext> {
  constructor() {
    super({ name: 'run-integration-tests', intent: 'test' });
  }

  async exec(): Promise<unknown> {
    // Your node logic here — prep/exec/post are auto-instrumented
    return { passed: 42, failed: 0 };
  }
}

// Run inside a flow with a kindling-aware shared store
const node = new TestRunnerNode();
const flow = new KindlingFlow(node);
await flow.run({ store, scopeIds: { repoId: 'my-app' } });
```

### Pin Critical Findings

Mark important discoveries for non-evictable retrieval:

```typescript
service.pin({
  targetType: 'observation',
  targetId: errorObs.id,
  note: 'Root cause of production outage',
  ttlMs: 7 * 24 * 60 * 60 * 1000, // 1 week
});

// Pins always appear first in retrieval
const results = service.retrieve({ query: 'outage' });
console.log(results.pins); // Includes the pinned error
```

## Design Principles

1. **Capture, Don't Judge** — preserves what happened without asserting truth
2. **Deterministic & Explainable** — retrieval results include "why" explanations
3. **Local-First** — no external services, embedded SQLite
4. **Privacy-Aware** — automatic redaction of secrets, bounded output capture
5. **Provenance Always** — every piece of context points to concrete evidence

## kindling + anvil

**kindling** captures what happened. **anvil** enforces what should happen.

Request access to the anvil closed beta → [eddacraft.ai](https://eddacraft.ai)

## Development

```bash
git clone https://github.com/eddacraft/kindling.git
cd kindling
pnpm install
pnpm run build
pnpm run test
pnpm run type-check
```

This project uses [anvil Plan Spec (APS)](https://github.com/eddacraft/anvil-plan-spec) for planning.

## Documentation

Full docs at **[docs.eddacraft.ai/kindling/overview](https://docs.eddacraft.ai/kindling/overview)**:

- [Quickstart](https://docs.eddacraft.ai/kindling/quickstart/install)
- [Core Concepts](https://docs.eddacraft.ai/kindling/concepts/capsules)
- [CLI Reference](https://docs.eddacraft.ai/kindling/reference/cli)
- [Configuration](https://docs.eddacraft.ai/kindling/reference/config)
- [Writing Adapters](https://docs.eddacraft.ai/kindling/adapters/custom)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines and [SECURITY.md](SECURITY.md) for responsible disclosure.

## License

Apache 2.0 — See [LICENSE](LICENSE) for details.

---

**Built by the [eddacraft](https://eddacraft.ai) team**
