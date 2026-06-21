# @eddacraft/kindling

Local memory and continuity engine for AI-assisted development.

[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling.svg)](https://www.npmjs.com/package/@eddacraft/kindling)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../../LICENSE)

**kindling** captures observations (tool calls, diffs, commands, errors) from AI workflows, organizes them into capsules, and makes context retrievable with deterministic, explainable results. All data is stored locally by the kindling daemon (Rust).

This package is a **thin TypeScript client** for the kindling daemon. It has no native dependencies — it speaks the daemon's v1 HTTP API over a Unix domain socket (`~/.kindling/kindling.sock`) and auto-spawns `kindling serve` on first use.

## Installation

Install the client from npm:

```bash
npm install @eddacraft/kindling
```

The client talks to the Rust `kindling` daemon, which is installed separately:

```bash
# via cargo
cargo install eddacraft-kindling

# or via the install script
curl -fsSL https://docs.eddacraft.ai/kindling/install.sh | sh
```

### Installation Documentation

For other installation options and detailed setup instructions, visit [kindling installation quickstart](https://docs.eddacraft.ai/kindling/quickstart/install).

## Quick Start

```typescript
import { Kindling } from '@eddacraft/kindling';

// Connect to the daemon (auto-spawns `kindling serve` on first use)
const kindling = new Kindling();

// Open a session capsule
const capsule = await kindling.openCapsule({
  kind: 'session',
  intent: 'debug authentication issue',
  scopeIds: { sessionId: 'session-1', repoId: 'my-project' },
});

// Capture observations (the daemon owns id, timestamp, and redaction)
await kindling.appendObservation(
  {
    kind: 'error',
    content: 'JWT validation failed: token expired',
    provenance: { stack: 'Error: Token expired\n  at validateToken.ts:42' },
    scopeIds: { sessionId: 'session-1' },
  },
  { capsuleId: capsule.id },
);

// Retrieve relevant context
const results = await kindling.retrieve({
  query: 'authentication token',
  scopeIds: { sessionId: 'session-1' },
});

// Close session with summary
await kindling.closeCapsule(capsule.id, {
  generateSummary: true,
  summaryContent: 'Fixed JWT expiration check in token validation middleware',
});
```

## What's Included

This package exports the thin client and its types:

| Export                                       | Description                                                      |
| -------------------------------------------- | ---------------------------------------------------------------- |
| `Kindling`                                   | Daemon client: capsule lifecycle, observation capture, retrieval |
| `KindlingError`, `DaemonUnavailableError`, … | Typed errors for daemon transport and API failures               |
| `resolveConfig`, `defaultSocketPath`, …      | Configuration + socket/project resolution helpers                |
| Generated domain types                       | `Capsule`, `Observation`, `Pin`, `RetrieveResult`, and friends   |

All persistence, FTS retrieval, and concurrency are handled by the Rust daemon, not this package.

## Adapters

- [`@eddacraft/kindling-adapter-opencode`](https://www.npmjs.com/package/@eddacraft/kindling-adapter-opencode) — OpenCode session integration
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
