# @eddacraft/kindling-adapter-claude-code

> **⚠️ Deprecated** — this package is deprecated and will be removed at v1.0.0.
> Kindling is now a Rust daemon. Use [`@eddacraft/kindling`](https://www.npmjs.com/package/@eddacraft/kindling)
> (the thin HTTP-over-UDS client) or the `kindling` binary instead.
> See <https://github.com/eddacraft/kindling>.

Claude Code adapter for Kindling - capture tool calls and session context via hooks for memory continuity.

## Overview

This adapter integrates Kindling with [Claude Code](https://claude.ai/code) via its hooks system. It automatically captures:

- **Tool calls** (Read, Write, Edit, Bash, Glob, Grep, etc.)
- **User messages**
- **Subagent completions**
- **Session lifecycle** (start/stop)

All captured events become searchable observations in Kindling, enabling context retrieval across sessions.

## Installation

```bash
npm install @eddacraft/kindling-adapter-claude-code @eddacraft/kindling-core @eddacraft/kindling-store-sqlite
```

## Quick Start

```typescript
import { createHookHandlers } from '@eddacraft/kindling-adapter-claude-code';
import { openDatabase, SqliteKindlingStore } from '@eddacraft/kindling-store-sqlite';

// Initialize store
const db = openDatabase({ dbPath: '~/.kindling/kindling.db' });
const store = new SqliteKindlingStore(db);

// Create hook handlers
const handlers = createHookHandlers(store);

// Register with Claude Code hooks (see Hook Configuration below)
```

## Hook Configuration

The adapter provides handlers for Claude Code hooks. Register them in your `.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "type": "command",
        "command": "kindling-hook session-start"
      }
    ],
    "PostToolUse": [
      {
        "type": "command",
        "command": "kindling-hook post-tool-use"
      }
    ],
    "Stop": [
      {
        "type": "command",
        "command": "kindling-hook stop"
      }
    ]
  }
}
```

## API Reference

### `createHookHandlers(store, config?)`

Creates hook handlers connected to a Kindling store.

```typescript
const handlers = createHookHandlers(store, {
  // Capture tool results (default: true)
  captureResults: true,

  // Capture user messages (default: true)
  captureUserMessages: true,

  // Capture subagent outputs (default: true)
  captureSubagents: true,

  // Default intent for new sessions
  defaultIntent: 'Claude Code session',
});
```

### Hook Handlers

| Handler              | Hook             | Description                         |
| -------------------- | ---------------- | ----------------------------------- |
| `onSessionStart`     | SessionStart     | Opens a session capsule             |
| `onPostToolUse`      | PostToolUse      | Captures tool calls as observations |
| `onStop`             | Stop             | Closes the session capsule          |
| `onUserPromptSubmit` | UserPromptSubmit | Captures user messages              |
| `onSubagentStop`     | SubagentStop     | Captures subagent completions       |

### Utility Methods

```typescript
// Check if a session is active
handlers.isSessionActive('session-123');

// Get session statistics
handlers.getSessionStats('session-123');
// Returns: { eventCount: 42, duration: 3600000 }

// Access the underlying session manager
const manager = handlers.getSessionManager();
```

## Event Mapping

| Claude Code Event        | Observation Kind | Description          |
| ------------------------ | ---------------- | -------------------- |
| PostToolUse (Write/Edit) | `file_diff`      | File modifications   |
| PostToolUse (Bash)       | `command`        | Shell commands       |
| PostToolUse (other)      | `tool_call`      | Tool invocations     |
| UserPromptSubmit         | `message`        | User messages        |
| SubagentStop             | `node_end`       | Subagent completions |

## Content Filtering

The adapter includes safety features to prevent accidental capture of sensitive data:

- **Secret detection**: API keys, tokens, and passwords are automatically masked
- **Content truncation**: Large outputs are truncated (default: 50KB)
- **Path exclusions**: Files in `.git/`, `node_modules/`, `.env`, etc. are flagged

```typescript
import {
  filterContent,
  maskSecrets,
  isExcludedPath,
} from '@eddacraft/kindling-adapter-claude-code';

// Filter content with secret masking
const safe = filterContent('api_key=secret123');
// Result: 'api_key=[REDACTED]'

// Check if path should be excluded
isExcludedPath('/project/.env'); // true
```

## Direct Usage

You can also use the lower-level APIs directly:

```typescript
import { SessionManager, mapEvent } from '@eddacraft/kindling-adapter-claude-code';

// Create session manager
const manager = new SessionManager(store);

// Start session
const ctx = manager.onSessionStart({
  sessionId: 'session-1',
  cwd: '/home/user/project',
  intent: 'Debug authentication',
});

// Process events
const event = {
  type: 'post_tool_use',
  timestamp: Date.now(),
  sessionId: 'session-1',
  cwd: '/home/user/project',
  toolName: 'Read',
  toolInput: { file_path: '/src/auth.ts' },
  toolResult: '// auth code...',
};

const result = manager.onEvent(event);

// End session
manager.onStop('session-1', {
  summaryContent: 'Fixed token validation bug',
});
```

## Use Case: Session Continuity

The primary use case is maintaining context across Claude Code sessions:

1. **Session 1**: Work on authentication bug
   - Adapter captures: file reads, edits, test runs, errors
   - Session closes with summary

2. **Session 2**: "What did I work on yesterday?"
   - Query Kindling: `service.retrieve({ query: 'authentication' })`
   - Get back: relevant observations with provenance

This enables Claude Code to "remember" what happened in previous sessions.

## License

Apache-2.0
