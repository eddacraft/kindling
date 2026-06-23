# Build your adapter in 10 minutes

An adapter is a thin layer that translates events from your tool into kindling
observations. You do not need to touch SQLite, FTS indexing or retrieval logic.
The daemon owns persistence; your adapter calls a small API.

This guide uses the Node client ([`@eddacraft/kindling`](../../packages/kindling/)).
The same shape exists in Rust via [`kindling-client`](https://crates.io/crates/kindling-client).

## What you will build

A minimal session adapter that:

1. Opens a `session` capsule when work starts
2. Appends observations as events arrive
3. Retrieves relevant context on demand
4. Closes the capsule with a summary when work ends

The full runnable example lives at [examples/adapter-minimal](../../examples/adapter-minimal/).

## Prerequisites

- Node.js >= 20
- The `kindling` binary on `PATH` (via [install.sh](../../install.sh) or `cargo install eddacraft-kindling`)

```bash
npm install @eddacraft/kindling
```

## Step 1: Create the client

```typescript
import { Kindling } from '@eddacraft/kindling';

const kindling = new Kindling({
  projectRoot: process.cwd(),
});
```

The client auto-spawns `kindling serve` on first use. For explicit control,
start the daemon yourself: `kindling serve`.

## Step 2: Open a capsule

A capsule groups observations for one bounded unit of work (a session, a
workflow node, a review pass).

```typescript
const capsule = await kindling.openCapsule({
  kind: 'session',
  intent: 'debug authentication issue',
  scopeIds: {
    sessionId: 'session-1',
    repoId: process.cwd(),
  },
});
```

`scopeIds` tie memory to a session, repo, agent or task. Pass the same scope
when retrieving later.

## Step 3: Append observations

Map your tool's events to observation kinds: `tool_call`, `command`, `file_diff`,
`error`, `message`, and workflow kinds for PocketFlow-style nodes.

```typescript
await kindling.appendObservation(
  {
    kind: 'error',
    content: 'JWT validation failed: token expired',
    provenance: { source: 'my-adapter', stack: 'Error at validate.ts:42' },
    scopeIds: { sessionId: 'session-1', repoId: process.cwd() },
  },
  { capsuleId: capsule.id },
);
```

The daemon assigns `id`, `ts` and redaction. Attach observations to the open
capsule with `capsuleId` so they appear in session summaries.

## Step 4: Retrieve context

Before your agent acts, pull relevant prior context:

```typescript
const result = await kindling.retrieve({
  query: 'JWT token expiration',
  scopeIds: { sessionId: 'session-1', repoId: process.cwd() },
});

for (const candidate of result.candidates) {
  console.log(candidate.score, candidate.entity.content);
}
```

Retrieval is deterministic: pins first, then the current summary, then ranked
FTS hits with provenance.

## Step 5: Close the capsule

When the session ends, close with an optional summary:

```typescript
await kindling.closeCapsule(capsule.id, {
  generateSummary: true,
  summaryContent: 'Fixed JWT expiration check in token validation middleware',
});
```

## Adapter design checklist

| Concern          | Recommendation                                                                     |
| ---------------- | ---------------------------------------------------------------------------------- |
| Session identity | Use a stable `sessionId` per editor tab, agent run or chat thread.                 |
| Repo scope       | Set `repoId` to the project root so memory does not leak across repos.             |
| Event mapping    | Map tool-specific events to kindling observation kinds; keep `content` searchable. |
| Provenance       | Store raw metadata in `provenance` (tool name, file path, exit code).              |
| Crash recovery   | Call `getOpenCapsule(sessionId)` before opening a new capsule.                     |
| Filtering        | Skip noisy or duplicate events at the adapter layer.                               |

## Rust equivalent

```rust
use kindling_client::{Client, CapsuleType, ObservationInput, ObservationKind, RetrieveOptions, ScopeIds};

let client = Client::new()?;
let scope = ScopeIds { session_id: Some("session-1".into()), ..Default::default() };

let capsule = client
    .open_capsule(CapsuleType::Session, "debug auth", scope.clone(), None)
    .await?;

client
    .append_observation(
        ObservationInput {
            kind: ObservationKind::Error,
            content: "JWT validation failed".into(),
            scope_ids: scope.clone(),
            ..Default::default()
        },
        Some(capsule.id.clone()),
        Some(true),
    )
    .await?;

let results = client
    .retrieve(RetrieveOptions {
        query: "JWT".into(),
        scope_ids: scope,
        ..Default::default()
    })
    .await?;

client.close_capsule(&capsule.id, Default::default()).await?;
```

## Examples in this repo

| Example                                                                    | Description                            |
| -------------------------------------------------------------------------- | -------------------------------------- |
| [adapter-minimal](../../examples/adapter-minimal/)                         | Smallest end-to-end TypeScript adapter |
| [kindling-adapter-opencode](../../packages/kindling-adapter-opencode/)     | Production OpenCode session adapter    |
| [kindling-adapter-pocketflow](../../packages/kindling-adapter-pocketflow/) | Production PocketFlow workflow adapter |

## Next steps

- Read the [integrations matrix](../integrations.md) to see where your tool fits.
- Study the [retrieval contract](../retrieval-contract.md) for deterministic search behaviour.
- Open a PR with your adapter if you want it listed in the matrix.
