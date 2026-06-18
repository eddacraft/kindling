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

### Node Adapter

```typescript
import { NodeAdapter, NodeStatus } from '@eddacraft/kindling-adapter-pocketflow';
import { SqliteKindlingStore } from '@eddacraft/kindling-store-sqlite';

const store = new SqliteKindlingStore(db);
const adapter = new NodeAdapter({
  store,
  repoId: '/home/user/my-project',
});

// When a node starts
adapter.onNodeStart({
  node: { id: 'test-1', name: 'run-integration-tests' },
  status: NodeStatus.Running,
  input: { testPattern: '**/*.test.ts' },
});

// When a node produces output
adapter.onNodeOutput({
  node: { id: 'test-1', name: 'run-integration-tests' },
  output: { passed: 42, failed: 0 },
});

// When a node ends
adapter.onNodeEnd({
  node: { id: 'test-1', name: 'run-integration-tests' },
  status: NodeStatus.Success,
  output: { passed: 42, failed: 0, duration: 3500 },
});
```

### Flow Integration

```typescript
import { KindlingFlow, KindlingNode } from '@eddacraft/kindling-adapter-pocketflow';

// Extend KindlingNode for automatic capture
class MyTestNode extends KindlingNode {
  async exec(input: TestInput): Promise<TestOutput> {
    const results = await runTests(input.pattern);
    return { passed: results.passed, failed: results.failed };
  }
}

// Create a flow
const flow = new KindlingFlow({
  store,
  repoId: '/repo',
});

flow.addNode('test', new MyTestNode());
flow.addNode('deploy', new DeployNode());
flow.connect('test', 'deploy', 'success');

// Run the flow (capsules created automatically)
await flow.run({ pattern: '**/*.test.ts' });
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

```typescript
interface NodeAdapterOptions {
  store: KindlingStore;
  repoId: string;
  agentId?: string;
  userId?: string;

  // Intent inference
  intentMapping?: Record<string, string>;

  // Confidence tracking
  confidenceWindow?: number; // Default: 10 runs
}
```

## PocketFlow Concepts

PocketFlow is a minimalist workflow framework:

- **Node** - `prep -> exec -> post` lifecycle
- **Flow** - Orchestrates nodes via action-based transitions
- **Shared Store** - Global state accessible by all nodes
- **BatchNode/BatchFlow** - Process arrays of items

See the [PocketFlow documentation](./vendor/pocketflow/docs/) for details.

## Related Packages

- [`@eddacraft/kindling-core`](../kindling-core) - Domain types and capsule lifecycle
- [`@eddacraft/kindling-store-sqlite`](../kindling-store-sqlite) - SQLite persistence
- [`@eddacraft/kindling-adapter-opencode`](../kindling-adapter-opencode) - OpenCode session adapter

## License

Apache-2.0
