# @eddacraft/kindling

Local memory and continuity engine for AI-assisted development.

[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling.svg)](https://www.npmjs.com/package/@eddacraft/kindling)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../../LICENSE)

**kindling** captures observations (tool calls, diffs, commands, errors) from AI workflows, organizes them into capsules, and makes context retrievable with deterministic, explainable results. All data is stored locally using embedded SQLite with FTS5.

## Installation

```bash
npm install @eddacraft/kindling
```

This is the main package — it bundles core types, SQLite storage (better-sqlite3, WAL mode, FTS5), local FTS retrieval, and an optional API server.

### Installation Documentation

For other installation options and detailed setup instructions, visit [kindling installation quickstart](https://docs.eddacraft.ai/kindling/quickstart/install).

## Quick Start

```typescript
import { randomUUID } from 'node:crypto';
import {
  KindlingService,
  openDatabase,
  SqliteKindlingStore,
  LocalFtsProvider,
} from '@eddacraft/kindling';

// Initialise
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

// Retrieve relevant context
const results = await service.retrieve({
  query: 'authentication token',
  scopeIds: { sessionId: 'session-1' },
});

// Close session with summary
service.closeCapsule(capsule.id, {
  generateSummary: true,
  summaryContent: 'Fixed JWT expiration check in token validation middleware',
});

db.close();
```

## What's Included

| Export                                | Source Package          | Description                                                     |
| ------------------------------------- | ----------------------- | --------------------------------------------------------------- |
| `KindlingService`                     | kindling-core           | Capsule lifecycle, observation capture, retrieval orchestration |
| `openDatabase`, `SqliteKindlingStore` | kindling-store-sqlite   | SQLite persistence with FTS5, WAL mode, migrations              |
| `LocalFtsProvider`                    | kindling-provider-local | Ranked FTS retrieval with explainability                        |
| `@eddacraft/kindling/server`          | kindling-server         | Fastify API server for multi-agent concurrency                  |
| `@eddacraft/kindling/client`          | kindling-server         | HTTP client for the API server                                  |

## Entry Points

```typescript
// Core + store + provider (most users)
import {
  KindlingService,
  openDatabase,
  SqliteKindlingStore,
  LocalFtsProvider,
} from '@eddacraft/kindling';

// API server (multi-agent setups)
import { createServer } from '@eddacraft/kindling/server';

// HTTP client
import { KindlingApiClient } from '@eddacraft/kindling/client';
```

## Lightweight Alternatives

If you don't need the full bundle:

- [`@eddacraft/kindling-core`](https://www.npmjs.com/package/@eddacraft/kindling-core) — Types + service only (for adapter authors, browser users)
- [`@eddacraft/kindling-store-sqljs`](https://www.npmjs.com/package/@eddacraft/kindling-store-sqljs) — Browser/WASM store (sql.js)

## Adapters

- [`@eddacraft/kindling-adapter-opencode`](https://www.npmjs.com/package/@eddacraft/kindling-adapter-opencode) — OpenCode session integration
- [`@eddacraft/kindling-adapter-claude-code`](https://www.npmjs.com/package/@eddacraft/kindling-adapter-claude-code) — Claude Code hooks integration
- [`@eddacraft/kindling-adapter-pocketflow`](https://www.npmjs.com/package/@eddacraft/kindling-adapter-pocketflow) — PocketFlow workflow integration

## Documentation

Full documentation at [docs.eddacraft.ai/docs/kindling](https://docs.eddacraft.ai/docs/kindling/overview).

## Requirements

- Node.js >= 20.0.0
- ESM only (`"type": "module"`)

## License

Apache-2.0

## Related

- [anvil](https://eddacraft.ai) — policy and enforcement layer for AI-assisted development; complements kindling by enforcing what should happen rather than capturing what did.
