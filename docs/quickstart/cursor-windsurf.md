# Quickstart: Cursor and Windsurf

Capture file saves from Cursor or Windsurf into kindling, and search that memory
from the Command Palette. The same extension runs in VS Code.

The extension is distributed as a VSIX file. It is not listed on the VS Code
Marketplace or Open VSX, so you install it with **Install from VSIX**.

## Prerequisites

- The `kindling` binary. The extension starts the kindling daemon
  (`kindling serve`) for you, but it does not ship the binary. Install it with
  the one-line installer:

  ```bash
  curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
  ```

  Other options (Cargo, npm, Homebrew) are listed in the
  [quickstart without Claude Code](./without-claude-code.md#install).

- `kindling` must be on the `PATH` that your editor sees. Editors launched from
  a desktop shortcut or the macOS Dock may not inherit your shell `PATH`. If
  `kindling --version` works in a terminal but the extension reports the daemon
  as unavailable, set `KINDLING_BIN` to the binary's absolute path in the
  environment your editor starts with, or launch the editor from a terminal.

- To build the VSIX yourself: Node.js 24, pnpm and a clone of this repository.

## Get the VSIX

If the [GitHub Release](https://github.com/eddacraft/kindling/releases) you are
installing from lists a `kindling-<version>.vsix` asset, download it and skip to
[Install in your editor](#install-in-your-editor).

Otherwise, build it from source:

```bash
git clone https://github.com/eddacraft/kindling.git && cd kindling
pnpm install
pnpm --filter @eddacraft/kindling-adapter-vscode run package
```

This writes `packages/kindling-adapter-vscode/kindling-<version>.vsix`. The
VSIX contains a single bundled script, so it has no runtime dependencies beyond
the `kindling` binary.

## Install in your editor

The steps are the same in Cursor, Windsurf and VS Code:

1. Open the Extensions view.
2. Open the `...` menu at the top of the view and choose **Install from VSIX...**.
3. Select the `kindling-<version>.vsix` file.
4. Reload the window if the editor asks you to.

From a terminal, Cursor and VS Code also accept the file directly:

```bash
cursor --install-extension kindling-<version>.vsix
code --install-extension kindling-<version>.vsix
```

## Check it works

1. Open a project folder.
2. Run **Kindling: Status** from the Command Palette. The **Kindling** output
   channel shows the daemon version, the schema version and the active editor
   session.
3. Edit and save a file.
4. Run **Kindling: Search Memory** and search for the file name. The saved path
   appears in the **Kindling** output channel.

## What the extension captures

- **Session capsule:** opened when the extension activates, closed with a short
  summary when the window closes.
- **File saves:** each saved file path becomes a `file_diff` observation.
- **Selections:** **Kindling: Log Selection** stores the selected text as a
  `message` observation.

The extension does not capture your AI assistant's chat, tool calls or terminal
commands. To capture those, log them with the CLI (`kindling log`) or build an
adapter; see the [integrations matrix](../integrations.md).

**Kindling: Search Memory** searches the current editor session in the open
workspace. To search everything kindling holds for the project, run
`kindling search` from a terminal in the project's root folder.

## Update or remove

To update, build or download the newer VSIX and install it over the old one.
To remove, uninstall **Kindling** from the Extensions view. Captured memory stays
in kindling's local database; redact individual observations with
`kindling forget`.

## Related documentation

- [Extension README](../../packages/kindling-adapter-vscode/README.md)
- [Quickstart without Claude Code](./without-claude-code.md)
- [Integrations matrix](../integrations.md)
