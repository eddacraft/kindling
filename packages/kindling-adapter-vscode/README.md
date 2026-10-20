# @eddacraft/kindling-adapter-vscode

VS Code, Cursor, and Windsurf extension for kindling. Captures file saves and editor activity into local memory via the kindling daemon.

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../../LICENSE)

## Installation

The extension ships as a VSIX file attached to each
[GitHub Release](https://github.com/eddacraft/kindling/releases). It is not
listed on the VS Code Marketplace or Open VSX, and this package is not
published to npm. Step-by-step instructions for Cursor and Windsurf are in the
[Cursor and Windsurf quickstart](../../docs/quickstart/cursor-windsurf.md).

### Prerequisite: the kindling binary

The extension starts the kindling daemon (`kindling serve`) on first use but does
not ship the binary. Install `kindling` and make sure it is on the `PATH` your
editor sees, or set `KINDLING_BIN` to the binary's absolute path. See the
[install options](../../docs/quickstart/without-claude-code.md#install).

### Build the VSIX

From a clone of the repository (Node.js 24 and pnpm required):

```bash
pnpm install
pnpm --filter @eddacraft/kindling-adapter-vscode run package
```

This writes `packages/kindling-adapter-vscode/kindling-<version>.vsix`. Pass
`--out <dir>` after `--` to write it elsewhere:

```bash
pnpm --filter @eddacraft/kindling-adapter-vscode run package -- --out ./out
```

The `package` script compiles the TypeScript, bundles the extension and the
`@eddacraft/kindling` client into a single CommonJS file with esbuild, then runs
`vsce package`. The VSIX has no runtime dependencies beyond the `kindling`
binary.

If a [GitHub Release](https://github.com/eddacraft/kindling/releases) lists a
`kindling-<version>.vsix` asset, you can download that instead of building.

### Install the VSIX

In VS Code, Cursor or Windsurf: open the Extensions view, open the `...` menu
and choose **Install from VSIX...**, then select the file. From a terminal:

```bash
code --install-extension kindling-<version>.vsix
cursor --install-extension kindling-<version>.vsix
```

### Local development

Open this package in an Extension Development Host window, or reference the
folder from your editor's extensions directory.

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
