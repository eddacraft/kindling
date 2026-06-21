# @eddacraft/kindling-adapter-pocketflow

PocketFlow workflow adapter for kindling - capture node executions with intent and confidence.

[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling-adapter-pocketflow.svg)](https://www.npmjs.com/package/@eddacraft/kindling-adapter-pocketflow)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../../LICENSE)

## Installation

```bash
npm install @eddacraft/kindling-adapter-pocketflow
```

## Overview

`@eddacraft/kindling-adapter-pocketflow` integrates kindling with PocketFlow workflows:

- **Node-Level Capsules** - Each node execution creates a capsule
- **Automatic Capture** - Records `node_start`, `node_output`, `node_error`, `node_end` events
- **Intent Inference** - Derives capsule intent from node names
- **Confidence Tracking** - Records success/failure confidence

## Usage

Capture is driven by extending `KindlingNode` (and optionally `KindlingFlow`).
Each node opens a `pocketflow_node` capsule on `prep`, records `node_start` /
`node_output` / `node_end` (or `node_error`) observations, then closes the
capsule on `post`. Persistence is delegated to the kindling daemon (Rust) via
the `@eddacraft/kindling` thin client, which is threaded through PocketFlow's
shared store as a `KindlingNodeContext`.

### Node-Level Capture

```typescript
import { KindlingNode, type KindlingNodeContext } from '@eddacraft/kindling-adapter-pocketflow';
import { Kindling } from '@eddacraft/kindling';

// The shared store carries the daemon client + scope for every node.
const shared: KindlingNodeContext = {
  kindling: new Kindling(),
  scopeIds: { repoId: '/home/user/my-project' },
};

// Extend KindlingNode for automatic capture.
class RunTestsNode extends KindlingNode<KindlingNodeContext> {
  async exec(): Promise<unknown> {
    const results = await runTests('**/*.test.ts');
    return { passed: results.passed, failed: results.failed };
  }
}

// metadata.name drives intent inference; the capsule is created automatically.
const node = new RunTestsNode({ name: 'run-integration-tests' });

// Running the node opens/closes its capsule and records observations.
await node.run(shared);
```

### Flow Integration

```typescript
import {
  KindlingFlow,
  KindlingNode,
  type KindlingNodeContext,
} from '@eddacraft/kindling-adapter-pocketflow';
import { Kindling } from '@eddacraft/kindling';

class RunTestsNode extends KindlingNode<KindlingNodeContext> {
  async exec(): Promise<unknown> {
    const results = await runTests('**/*.test.ts');
    return { passed: results.passed, failed: results.failed };
  }
}

const testNode = new RunTestsNode({ name: 'run-integration-tests' });
testNode.next(new DeployNode({ name: 'deploy-production' }));

// A KindlingFlow wraps the start node and records flow-level capsules.
const flow = new KindlingFlow(testNode, { name: 'ci-pipeline' });

const shared: KindlingNodeContext = {
  kindling: new Kindling(),
  scopeIds: { repoId: '/repo' },
};

// Run the flow (capsules created automatically per node and for the flow).
await flow.run(shared);
```

## Captured Events

| Event         | When                  | Content                        |
| ------------- | --------------------- | ------------------------------ |
| `node_start`  | Node begins execution | Node ID, name, input           |
| `node_output` | Node produces output  | Output data                    |
| `node_error`  | Node throws error     | Error message, stack           |
| `node_end`    | Node completes        | Final status, output, duration |

## Intent Inference

Node names are parsed to infer capsule intent:

| Node Name           | Inferred Intent |
| ------------------- | --------------- |
| `run-tests`         | `test`          |
| `build-app`         | `build`         |
| `deploy-production` | `deploy`        |
| `fix-auth-bug`      | `debug`         |
| `implement-feature` | `implement`     |

## Confidence Scoring

Confidence is tracked based on node success/failure history:

```typescript
// First run: neutral confidence
{
  confidence: 0.5;
}

// Consistent success: higher confidence
{
  confidence: 0.85;
}

// Recent failures: lower confidence
{
  confidence: 0.3;
}
```

## Configuration

Nodes receive their daemon client and scope through the shared
`KindlingNodeContext`:

```typescript
interface KindlingNodeContext {
  kindling: Kindling; // @eddacraft/kindling thin client (daemon-backed)
  scopeIds: ScopeIds; // { sessionId?, repoId?, agentId?, userId? }
  capsuleId?: string; // set by the node while it is active
}

interface NodeMetadata {
  name: string; // drives intent inference
  intent?: string; // explicit override for the inferred intent
}
```

Intent inference and confidence scoring are configurable through the
`inferIntent` / `ConfidenceTracker` helpers exported by this package.

## PocketFlow Concepts

PocketFlow is a minimalist workflow framework:

- **Node** - `prep -> exec -> post` lifecycle
- **Flow** - Orchestrates nodes via action-based transitions
- **Shared Store** - Global state accessible by all nodes
- **BatchNode/BatchFlow** - Process arrays of items

See the [PocketFlow documentation](./vendor/pocketflow/docs/) for details.

## Related Packages

- [`@eddacraft/kindling`](../kindling) - Thin client for the kindling daemon (domain types + capsule lifecycle)
- [`@eddacraft/kindling-adapter-opencode`](../kindling-adapter-opencode) - OpenCode session adapter

## License

Apache-2.0
