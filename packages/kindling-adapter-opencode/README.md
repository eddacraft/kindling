# @eddacraft/kindling-adapter-opencode

OpenCode session adapter for kindling - capture tool calls, commands, and file changes from AI coding sessions.

[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling-adapter-opencode.svg)](https://www.npmjs.com/package/@eddacraft/kindling-adapter-opencode)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../../LICENSE)

## Installation

```bash
npm install @eddacraft/kindling-adapter-opencode
```

## Overview

Captures observations from OpenCode development sessions for local memory and continuity.

## What is Captured

The OpenCode adapter automatically captures the following types of events from your development sessions:

### Tool Calls

- **Tool name** and **arguments**
- **Results** or **errors**
- **Duration** (execution time)
- **Timestamp**

### Command Execution

- **Command text** (e.g., `git status`, `npm test`)
- **Exit code**
- **stdout** and **stderr** output
- **Working directory**

### File Changes

- **File paths** modified
- **Diff** content (additions/deletions)
- **Change summary** (lines added/deleted)

### Errors

- **Error message**
- **Stack trace** preview
- **Error source** (runtime, validation, etc.)

### Messages

- **User messages** (prompts, questions)
- **Assistant messages** (responses, explanations)
- **Message length** and **model** used

## What is NOT Captured

Session lifecycle events (`session_start`, `session_end`) are **skipped** - they are used to manage capsules but not stored as observations.

## Safety & Privacy

### Automatic Redaction

The adapter automatically detects and redacts sensitive information:

- **API keys, tokens, passwords**: Patterns like `api_key=`, `token:`, `password=` are detected and values replaced with `[REDACTED]`
- **AWS credentials**: AWS secret access keys are masked
- **Bearer/Basic auth**: Authorization headers are sanitized
- **Long secret-like strings**: 32+ character alphanumeric strings with mixed letters/numbers are flagged as potential tokens

### Excluded Files

The following file paths are automatically excluded from capture:

- `node_modules/` directories
- `.git/` directories
- `.env` files
- `.pem`, `.key` certificate files
- Files containing `credentials` or `secrets` in the path

### Content Truncation

Large outputs are automatically truncated to **50,000 characters** to prevent excessive storage usage. A truncation notice is appended when content is shortened.

## Usage

### Starting a Session

```typescript
import { SessionManager } from '@eddacraft/kindling-adapter-opencode';
import { Kindling } from '@eddacraft/kindling';

// Connect to the kindling daemon (auto-spawns `kindling serve` on first use)
const kindling = new Kindling();

// Create session manager backed by the daemon client
const manager = new SessionManager(kindling);

// Start session
const context = await manager.onSessionStart({
  sessionId: 'session-123',
  intent: 'Fix authentication bug',
  repoId: '/home/user/my-project',
});
```

### Processing Events

```typescript
// Process tool call event
await manager.onEvent({
  type: 'tool_call',
  timestamp: Date.now(),
  sessionId: 'session-123',
  toolName: 'read_file',
  args: { path: 'src/auth.ts' },
  result: 'file contents...',
});

// Process command event
await manager.onEvent({
  type: 'command',
  timestamp: Date.now(),
  sessionId: 'session-123',
  command: 'npm test',
  exitCode: 0,
  stdout: 'All tests passed',
});
```

### Ending a Session

```typescript
// End session with optional summary
await manager.onSessionEnd('session-123', {
  reason: 'completed',
  summaryContent: 'Fixed JWT validation in auth middleware',
  summaryConfidence: 0.9,
});
```

### Content Filtering

```typescript
import { filterContent, truncateContent, maskSecrets } from '@eddacraft/kindling-adapter-opencode';

// Apply all safety filters
const filtered = filterContent(content, {
  maxLength: 10000,
  maskSecrets: true,
  showTruncationNotice: true,
});

// Just truncate
const truncated = truncateContent(longContent, { maxLength: 5000 });

// Just mask secrets
const masked = maskSecrets(contentWithSecrets);
```

## Configuration

Currently, safety filters are applied by default and cannot be disabled. Future versions may add opt-in/opt-out configuration.

## Data Storage

All captured observations are stored locally by the kindling daemon (Rust), which the adapter reaches through the `@eddacraft/kindling` thin client. No data is sent to external services.

Observations are:

- **Deterministically ordered** by timestamp and sequence number
- **Scoped** to session, repository, agent, and user
- **Queryable** via full-text search and filters
- **Redactable** after capture if needed

## Privacy Considerations

**What you should know:**

1. **Local only**: All data stays on your machine in a local SQLite database
2. **Automatic sanitization**: Sensitive data is detected and redacted automatically
3. **Manual review**: You can inspect and redact observations after capture
4. **Export/forget**: Observations can be exported or permanently deleted

**What you should check:**

- Review captured observations periodically for accidentally captured secrets
- Use `/memory forget <id>` to redact specific observations
- Consider excluding sensitive repositories from capture

## License

Apache-2.0
