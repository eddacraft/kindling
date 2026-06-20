# PocketFlow Capabilities

This document describes what PocketFlow enables beyond core kindling functionality. PocketFlow is vendored at `packages/kindling-adapter-pocketflow/vendor/pocketflow/`.

## What is PocketFlow?

PocketFlow is a **100-line minimalist LLM framework** for building Agents, Task Decomposition, RAG, and more. It provides:

- **Lightweight**: Core graph abstraction in ~100 lines, zero dependencies
- **Expressive**: Agents, Workflows, RAG, MapReduce patterns
- **Agentic-Coding**: Simple enough for AI agents to help build complex LLM applications

## Core Abstractions

### Node

A **Node** handles a single task in the workflow:

```typescript
import { Node } from '@eddacraft/kindling-adapter-pocketflow';

class SummarizeNode extends Node<SharedStore> {
  async prep(shared: SharedStore): Promise<string> {
    return shared.document; // Read from shared store
  }

  async run(document: string): Promise<string> {
    return await callLlm(`Summarize: ${document}`);
  }

  async post(shared: SharedStore, _prep: string, summary: string): Promise<string | undefined> {
    shared.summary = summary; // Write to shared store
    return 'default'; // Action for next node
  }
}
```

Lifecycle: `prep()` → `run()` → `post()`

### Flow

A **Flow** connects nodes through action-based transitions:

```typescript
import { Flow } from '@eddacraft/kindling-adapter-pocketflow';

const inputNode = new InputNode();
const processNode = new ProcessNode();
const outputNode = new OutputNode();

// Linear connection
inputNode.next(processNode).next(outputNode);

// Or conditional branching
processNode.on('success', outputNode);
processNode.on('retry', processNode); // Loop back

const flow = new Flow(inputNode);
await flow.run(sharedStore);
```

### Shared Store

Communication between nodes via a shared object:

```typescript
interface MySharedStore {
  input?: string;
  processed?: string;
  output?: string;
}

const shared: MySharedStore = {};
await flow.run(shared);
console.log(shared.output);
```

## Design Patterns Enabled

### 1. Agent Pattern

An autonomous agent that makes decisions and takes actions:

```typescript
class AgentNode extends Node<AgentStore> {
  async run(context: string): Promise<string> {
    const decision = await callLlm(`
      Context: ${context}
      Available actions: search, write, complete
      What should I do next?
    `);
    return decision;
  }

  async post(shared: AgentStore, _: unknown, decision: string): Promise<string | undefined> {
    if (decision.includes('search')) return 'search';
    if (decision.includes('write')) return 'write';
    return 'complete';
  }
}

// Connect to action nodes
agentNode.on('search', searchNode);
agentNode.on('write', writeNode);
agentNode.on('complete', finishNode);
```

**Use cases**:

- Coding assistants that decide what tool to use
- Research agents that explore topics autonomously
- Task automation that adapts to context

### 2. Workflow Pattern

Chaining multiple tasks into a pipeline:

```typescript
const extract = new ExtractNode();
const transform = new TransformNode();
const load = new LoadNode();

extract.next(transform).next(load);

const etlFlow = new Flow(extract);
```

**Use cases**:

- Data processing pipelines
- CI/CD automation
- Document processing (parse → analyze → generate)

### 3. RAG Pattern (Retrieval-Augmented Generation)

Combining retrieval with generation:

```typescript
// Offline: Index documents
class EmbedNode extends BatchNode<RagStore> {
  async prep(shared: RagStore): Promise<string[]> {
    return shared.documents;
  }

  async run(doc: string): Promise<Vector> {
    return await getEmbedding(doc);
  }

  async post(shared: RagStore, docs: string[], embeddings: Vector[]): Promise<void> {
    shared.index = createIndex(docs, embeddings);
  }
}

// Online: Query and generate
class RetrieveNode extends Node<RagStore> {
  async run(query: string): Promise<string[]> {
    const queryVec = await getEmbedding(query);
    return shared.index.search(queryVec, 5);
  }
}

class GenerateNode extends Node<RagStore> {
  async run(context: string[]): Promise<string> {
    return await callLlm(`Answer based on: ${context.join('\n')}`);
  }
}
```

**Use cases**:

- Documentation Q&A
- Code search and explanation
- Knowledge base assistants

### 4. MapReduce Pattern

Processing large data by splitting, processing, and combining:

```typescript
import { BatchNode } from '@eddacraft/kindling-adapter-pocketflow';

// Map: Process each chunk
class MapNode extends BatchNode<MapReduceStore> {
  async prep(shared: MapReduceStore): Promise<string[]> {
    return chunkText(shared.largeDocument, 10000);
  }

  async run(chunk: string): Promise<string> {
    return await callLlm(`Summarize: ${chunk}`);
  }
}

// Reduce: Combine results
class ReduceNode extends Node<MapReduceStore> {
  async run(summaries: string[]): Promise<string> {
    return await callLlm(`Combine these summaries: ${summaries.join('\n')}`);
  }
}
```

**Use cases**:

- Summarizing large documents
- Processing many files in parallel
- Aggregating results from multiple sources

### 5. Multi-Agent Pattern

Coordinating multiple specialized agents:

```typescript
// Coordinator decides which agent to use
class CoordinatorNode extends Node<MultiAgentStore> {
  async post(shared: MultiAgentStore, _: unknown, task: string): Promise<string> {
    if (task.includes('code')) return 'coder';
    if (task.includes('review')) return 'reviewer';
    return 'default';
  }
}

// Specialized agents
const coderAgent = new CoderAgentNode();
const reviewerAgent = new ReviewerAgentNode();

coordinator.on('coder', coderAgent);
coordinator.on('reviewer', reviewerAgent);
```

**Use cases**:

- Complex coding tasks (one agent writes, another reviews)
- Research teams (one searches, another synthesizes)
- Game playing (multiple players with different strategies)

### 6. Structured Output Pattern

Ensuring consistent output format:

```typescript
class StructuredNode extends Node<StructuredStore> {
  async run(input: string): Promise<object> {
    const result = await callLlm(`
      Extract the following from the text:
      - name: string
      - age: number
      - skills: string[]

      Text: ${input}
      Output as JSON:
    `);
    return JSON.parse(result);
  }
}
```

**Use cases**:

- Form filling from unstructured text
- Data extraction from documents
- API response formatting

## kindling + PocketFlow Integration

PocketFlow is a TypeScript framework, so the kindling integration ships as a
published TS adapter: `@eddacraft/kindling-adapter-pocketflow` (v0.2.0). The
adapter depends on the thin `@eddacraft/kindling` npm client, which speaks
HTTP-over-UDS to the **Rust kindling daemon** — that is where capsules and
observations are actually persisted. (kindling is Rust-canonical; the adapter no
longer depends on any TypeScript implementation package such as
`@eddacraft/kindling-core`.)

The adapter provides `KindlingNode` and `KindlingFlow` that automatically
capture observations via the thin client:

```typescript
import {
  KindlingNode,
  KindlingFlow,
  inferIntent,
  ConfidenceTracker,
} from '@eddacraft/kindling-adapter-pocketflow';

// Create a node with automatic capsule management
class MyNode extends KindlingNode<MyContext> {
  async run(input: string): Promise<string> {
    return await processInput(input);
  }
}

// Intent is inferred from node name
const node = new MyNode({ name: 'analyze-code' }); // intent: 'analyze'

// Confidence tracking
const tracker = new ConfidenceTracker();
tracker.recordSuccess('analyze-code');
const confidence = tracker.getConfidence('analyze-code'); // 0.6
```

### What Gets Captured

For each node invocation the adapter opens a capsule of type
`pocketflow_node` and records these observation kinds:

- `node_start`: When the node begins (with intent and parameters)
- `node_output`: The truncated output of the node
- `node_error`: If the node fails (with error details and retry count)
- `node_end`: When the node completes (with duration and status)

All observations are attached to the `pocketflow_node` capsule scoped to that
node invocation, and are persisted by the Rust daemon via the thin client.

### Intent Inference

Node names are automatically parsed to infer intent:

| Node Name           | Inferred Intent |
| ------------------- | --------------- |
| `run-tests`         | `test`          |
| `buildApp`          | `build`         |
| `deploy_production` | `deploy`        |
| `fixAuthBug`        | `debug`         |
| `implementFeature`  | `feature`       |
| `analyzeMetrics`    | `analyze`       |
| `generateReport`    | `generate`      |

### Confidence Tracking

Track reliability over time:

```typescript
const tracker = new ConfidenceTracker({
  historySize: 10, // Keep last 10 invocations
  baseConfidence: 0.5, // Starting confidence
  successIncrement: 0.1, // Boost per success
  failureDecrement: 0.15, // Penalty per failure
});

// After successful invocation
tracker.recordSuccess('my-node');

// After failure
tracker.recordFailure('my-node', 'Connection timeout');

// Get confidence for retrieval ranking
const metadata = tracker.getProvenanceMetadata('my-node');
// { confidence: 0.65, successCount: 3, failureCount: 1, ... }
```

## Summary

PocketFlow enables building sophisticated AI systems with minimal code:

| Pattern     | Lines of Code | Use Case                       |
| ----------- | ------------- | ------------------------------ |
| Simple Node | ~20           | Single LLM call                |
| Linear Flow | ~30           | Sequential pipeline            |
| Agent       | ~50           | Autonomous decision-making     |
| RAG         | ~80           | Knowledge-augmented generation |
| MapReduce   | ~60           | Large data processing          |
| Multi-Agent | ~100          | Coordinated specialists        |

Combined with kindling, you get:

- **Observability**: Every node invocation is captured
- **Context**: Past invocations inform future decisions
- **Reliability**: Confidence tracking identifies flaky nodes
- **Debugging**: Full provenance for post-mortem analysis
