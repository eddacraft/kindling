# kindling Use Cases

A development reference for understanding when kindling might be the right fit for your project.

## What is kindling?

kindling is a **local memory and continuity engine** for AI-assisted development. It captures observations from AI workflows (tool calls, diffs, commands, errors), organizes them into capsules (bounded units of meaning), and makes context retrievable with deterministic, explainable results.

All data is stored locally using embedded SQLite with FTS5—no cloud dependencies, no data leaving your machine.

## When kindling Fits

### 1. Building AI Coding Assistants

**If you need**: Persistent context across sessions for an AI that helps developers write code.

**The problem**: LLMs have limited context windows and no memory between sessions. Your AI assistant forgets everything it learned in the previous conversation.

**How kindling helps**:

- Captures what tools were called, what files were changed, what errors occurred
- Organizes context by session, repository, and user
- Retrieves relevant past context when starting a new session
- Provides explainable results ("this context came from session X where you fixed bug Y")

**Example**: A coding assistant that remembers "last time you worked on auth, you ran into CORS issues with the token refresh endpoint."

### 2. Multi-Agent Orchestration Systems

**If you need**: Coordination between multiple AI agents working on related tasks.

**The problem**: Agent A generates output that Agent B needs, but there's no standard way to share context, track what happened, or recover from failures.

**How kindling helps**:

- Each agent's work is captured as observations in capsules
- Agents can retrieve context from other agents' work
- Pin important observations for guaranteed inclusion in context
- Deterministic retrieval means agents get consistent, reproducible context

**Example**: A code review agent that retrieves the implementation context from the coding agent that wrote the feature.

### 3. Workflow Automation with AI Nodes

**If you need**: AI-powered nodes in a workflow pipeline (ETL, CI/CD, data processing).

**The problem**: When an AI node fails or produces unexpected output, debugging is difficult because there's no record of what the AI "saw" or "decided."

**How kindling helps**:

- Automatically creates capsules for each workflow node execution
- Records inputs, outputs, errors, and timing
- Tracks intent (what the node was trying to do) and confidence (how reliable its output is)
- Enables post-mortem analysis and replay debugging

**Example**: A data pipeline where an LLM classifies documents—kindling captures why each document was classified the way it was.

### 4. Development Session Continuity

**If you need**: To pick up where you left off after taking a break from a coding project.

**The problem**: You return to a project and can't remember what you were working on, what you tried, or what was broken.

**How kindling helps**:

- Automatically captures your development session as you work
- Retrieves "what was I doing?" context when you return
- Summarizes past sessions and key decisions
- Finds relevant past work when you encounter similar problems

**Example**: Starting your workday with "Here's what you were working on yesterday: implementing the retry logic for the payment service. The last error was a timeout on line 234."

### 5. Privacy-First AI Memory

**If you need**: AI memory without sending your data to external services.

**The problem**: Cloud-based memory solutions require sending potentially sensitive code, errors, and development context to third-party servers.

**How kindling helps**:

- All data stored locally in SQLite
- No cloud dependencies
- Export/import for backup and migration
- Redaction support for sensitive observations

**Example**: An AI assistant for a financial services company where code cannot leave the developer's machine.

### 6. Explainable AI Context

**If you need**: To understand why an AI made a particular decision or suggestion.

**The problem**: LLMs are black boxes. When they give wrong answers, you don't know what context led to the mistake.

**How kindling helps**:

- Three-tiered retrieval with provenance: pins (user-controlled), current summary, provider hits (FTS results)
- Each piece of retrieved context includes metadata about where it came from
- Deterministic ranking means you can reproduce and audit context selection

**Example**: "The AI suggested this refactor because it found 3 similar patterns in sessions from last month, ranked by recency and relevance."

## When kindling Does NOT Fit

### Not for these use cases:

1. **Stateless API calls**: If you just need to call an LLM once and throw away the response, kindling adds unnecessary overhead.

2. **Real-time streaming**: kindling is designed for post-hoc analysis and retrieval, not real-time observation streams.

3. **Distributed systems**: kindling uses local SQLite. For distributed AI systems, you'd need to aggregate data from multiple kindling instances.

4. **Non-development domains**: While kindling could theoretically be used for any AI workflow, it's optimized for software development contexts (tool calls, diffs, commands, errors).

5. **Large-scale production**: kindling is designed for individual developers or small teams, not enterprise-scale observability.

## Integration Points

kindling integrates through **adapters**:

| Adapter                                   | Use Case                                            |
| ----------------------------------------- | --------------------------------------------------- |
| `@eddacraft/kindling-adapter-opencode`    | OpenCode session integration                        |
| `@eddacraft/kindling-adapter-pocketflow`  | PocketFlow workflow nodes                           |
| `@eddacraft/kindling-adapter-claude-code` | Claude Code hooks integration                       |
| Custom adapters                           | Any AI system with tool calls, commands, or outputs |

## Key Concepts

- **Observation**: Atomic unit of captured context (tool_call, command, file_diff, error, message, node events)
- **Capsule**: Bounded unit grouping observations (session, workflow node, custom)
- **Retrieval**: Three-tiered system with pins (user-controlled), summaries (session context), and FTS hits (ranked search)
- **Provenance**: Metadata explaining where each piece of context came from

## Getting Started

```typescript
import {
  KindlingService,
  SqliteKindlingStore,
  LocalFtsProvider,
  openDatabase,
} from '@eddacraft/kindling';

// Initialize
const db = openDatabase({ path: './kindling.db' });
const store = new SqliteKindlingStore(db);
const provider = new LocalFtsProvider(db);
const service = new KindlingService({ store, provider });

// Open a session capsule
const capsule = service.openCapsule({
  type: 'session',
  intent: 'debug',
  scopeIds: { sessionId: 'session-1', repoId: 'my-project' },
});

// Capture an observation
service.appendObservation(
  {
    id: 'obs-1',
    kind: 'command',
    content: 'npm test failed with ECONNREFUSED',
    provenance: { command: 'npm test', exitCode: 1 },
    ts: Date.now(),
    scopeIds: { sessionId: 'session-1' },
    redacted: false,
  },
  { capsuleId: capsule.id },
);

// Later, retrieve context
const results = await service.retrieve({
  query: 'connection refused',
  scopeIds: { repoId: 'my-project' },
  limit: 10,
});
```

## Summary

Use kindling when you need:

- **Memory** for AI systems across sessions
- **Local** storage without cloud dependencies
- **Explainable** context with provenance
- **Deterministic** retrieval for reproducibility
- **Development-focused** observation types
