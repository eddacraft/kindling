# @eddacraft/kindling

Local memory and continuity engine for AI-assisted development.

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../../LICENSE)

**kindling** captures observations (tool calls, diffs, commands, errors) from AI workflows, organizes them into capsules, and makes context retrievable with deterministic, explainable results. All data is stored locally by the kindling daemon (Rust).

This package is a **thin TypeScript client** for the kindling daemon. It has no native dependencies — it speaks the daemon's v1 HTTP API over a Unix domain socket (`~/.kindling/kindling.sock`) and auto-spawns `kindling serve` on first use.

## Installation

> **Not published to npm.** kindling no longer publishes to npm. The versions of
> `@eddacraft/kindling` on npm are the retired 0.1.x TypeScript engine and are
> deprecated. Use this package from a clone of the repository, as below.

From a clone of [the repository](https://github.com/eddacraft/kindling) (Node.js 24 and pnpm required):

```bash
git clone https://github.com/eddacraft/kindling.git && cd kindling
pnpm install
pnpm --filter @eddacraft/kindling run build
```

Inside the workspace, depend on it with `"@eddacraft/kindling": "workspace:*"`. Outside it, bundle the built `packages/kindling/dist` into your own tool (the editor extension bundles the thin client this way with esbuild).

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

## Hook-payload fixtures (for adapter authors)

Building an adapter? Test your capture pipeline against the engine's **published,
versioned** hook-payload fixtures instead of hand-copying doc examples. Each case
pairs a captured hook input with the exact observation kindling produces, and the
set is derived from (and drift-gated against) the engine's internal source of truth.

```typescript
// The published fixtures ship with the package and carry a pinnable `version`.
import fixtures from '@eddacraft/kindling/fixtures' with { type: 'json' };

for (const { name, hookInput, expected } of fixtures.cases) {
  const observation = myAdapter.map(hookInput);
  expect(observation).toMatchObject(expected); // { kind, content, provenance, scopeIds }
}
```

The same JSON is published at the repo root under `fixtures/hook-payloads/` for
non-npm consumers. Pin on `fixtures.version` to detect format changes.

## Adapters

- [`@eddacraft/kindling-adapter-opencode`](../kindling-adapter-opencode): OpenCode session integration
- [`@eddacraft/kindling-adapter-pocketflow`](../kindling-adapter-pocketflow): PocketFlow workflow integration
- [`@eddacraft/kindling-adapter-vscode`](../kindling-adapter-vscode): VS Code, Cursor and Windsurf extension (bundles this client)

## Documentation

Full documentation at [docs.eddacraft.ai/docs/kindling](https://docs.eddacraft.ai/docs/kindling/overview).

## Requirements

- Node.js >= 24
- ESM only (`"type": "module"`)

## License

Apache-2.0

## Related

- [anvil](https://eddacraft.ai) — policy and enforcement layer for AI-assisted development; complements kindling by enforcing what should happen rather than capturing what did.
