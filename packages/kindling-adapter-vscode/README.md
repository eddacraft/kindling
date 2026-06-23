# @eddacraft/kindling-adapter-vscode

VS Code, Cursor, and Windsurf extension for kindling. Captures file saves and editor activity into local memory via the kindling daemon.

[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling-adapter-vscode.svg)](https://www.npmjs.com/package/@eddacraft/kindling-adapter-vscode)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../../LICENSE)

## Installation

### From a VSIX

Build the extension, then install the VSIX in your editor:

```bash
cd packages/kindling-adapter-vscode
pnpm install
pnpm run build
```

Package with `vsce` (or your editor's equivalent), then install the resulting `.vsix`:

- **VS Code**: Extensions view, menu, **Install from VSIX**
- **Cursor**: same flow as VS Code
- **Windsurf**: same flow as VS Code

### Local development

Reference the folder from your editor's extensions directory, or open this package in an Extension Development Host window.

Ensure the `kindling` daemon binary is available on your `PATH`, or set `KINDLING_BINARY` to point at it. The `@eddacraft/kindling` client auto-spawns `kindling serve` on first use.

## Overview

The adapter opens a session capsule when the extension activates, records `file_diff` observations when you save files, and closes the capsule with a short summary when the extension deactivates. Optional hooks exist for terminal command capture.

All data stays local. The extension talks to the kindling daemon through the `@eddacraft/kindling` thin client.

## Commands

| Command                 | Title                   | Description                                                  |
| ----------------------- | ----------------------- | ------------------------------------------------------------ |
| `kindling.search`       | Kindling: Search Memory | Search observations in the current workspace scope           |
| `kindling.logSelection` | Kindling: Log Selection | Append the current editor selection as a message observation |
| `kindling.status`       | Kindling: Status        | Show daemon health and active session info                   |

Open the Command Palette and run any of the titles above.

## What is Captured

- **File saves**: path recorded as a `file_diff` observation
- **Selection logging**: manual capture of selected text as a `message` observation
- **Terminal commands**: optional via `EditorSessionManager.onTerminalCommand` (not wired by default)

## Programmatic Usage

```typescript
import { EditorSessionManager } from '@eddacraft/kindling-adapter-vscode';
import { Kindling } from '@eddacraft/kindling';

const kindling = new Kindling({ projectRoot: '/path/to/repo' });
const manager = new EditorSessionManager(kindling);

await manager.onSessionStart({
  sessionId: 'editor-session-1',
  intent: 'Refactor auth module',
  repoId: '/path/to/repo',
});

await manager.onFileSave({
  sessionId: 'editor-session-1',
  filePath: '/path/to/repo/src/auth.ts',
  repoId: '/path/to/repo',
});

await manager.onSessionEnd('editor-session-1', {
  summaryContent: 'Updated auth middleware',
  summaryConfidence: 0.9,
});
```

## Privacy

Observations are stored locally by the kindling daemon. The extension does not send data to external services. Review captured memory periodically and use kindling's retrieval and redaction tools as needed.

## License

Apache-2.0
