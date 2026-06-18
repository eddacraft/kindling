# kindling for Claude Code

**Remember what you worked on across sessions.**

kindling automatically captures your Claude Code sessions and injects prior context when you start a new one. All data is stored locally in SQLite with full-text search.

## Install

**Prerequisite — the `kindling` binary.** The capture/injection hooks run
`kindling hook <type>`, so the `kindling` binary must be on your `PATH`.
Install it via any channel:

```bash
# one-line installer (Linux/macOS)
curl -fsSL https://install.kindling.dev | sh
# or Homebrew (macOS)
brew install eddacraft/tap/kindling
# or from source
cargo install kindling
```

Verify with `kindling --version`. (Hooks fail open — if the binary is missing
they no-op and never block your session.)

**Add the marketplace, then install the plugin:**

```
/plugin marketplace add eddacraft/kindling
/plugin install kindling@kindling-plugins
```

**Or load directly for development/testing:**

```bash
claude --plugin-dir ./plugins/kindling-claude-code
```

The plugin has no build step and no npm dependencies — both the capture
hooks and the `/memory` slash commands shell out to the `kindling` binary, so
all you need is that binary on your `PATH` (see the prerequisite above).

## What It Does

When you start a Claude Code session, kindling:

1. **Opens a session capsule** to track all activity
2. **Injects prior context** from previous sessions in this project
3. **Captures tool calls** (Read, Write, Edit, Bash, etc.)
4. **Captures your messages** as observations
5. **Closes the capsule** when the session ends

All captured data is stored in a project-scoped SQLite database with FTS5 full-text search.

## Commands

| Command                         | Description                         |
| ------------------------------- | ----------------------------------- |
| `/memory search <query>`        | Search past sessions                |
| `/memory status`                | Show database stats                 |
| `/memory pin [note] [--ttl 7d]` | Pin last observation (optional TTL) |
| `/memory pins`                  | List all pins                       |
| `/memory unpin <id>`            | Remove a pin                        |
| `/memory forget <id>`           | Redact an observation               |

## Use Cases

### Resume yesterday's work

```
/memory search authentication
```

Shows your recent work on auth, including files edited and commands run.

### Pin important decisions

```
/memory pin "Root cause: token expiry check was off by one"
```

Pins the last observation for quick retrieval.

### Forget something sensitive

```
/memory forget a3f2b1c4
```

Redacts an observation from search results while preserving referential integrity.

## Configuration

Environment variables:

| Variable                  | Default | Description                               |
| ------------------------- | ------- | ----------------------------------------- |
| `KINDLING_INJECT_CONTEXT` | `true`  | Enable context injection on session start |
| `KINDLING_MAX_CONTEXT`    | `10`    | Maximum results for context injection     |
| `KINDLING_DB_PATH`        | auto    | Override database path                    |

## Data Storage

Data is stored locally per-project:

```
~/.kindling/projects/<project-hash>/kindling.db
```

Each project gets its own isolated database. No data is shared between projects by default.

## Privacy

- **Local only** — no data leaves your machine
- **Secret filtering** — API keys and tokens are automatically masked
- **Per-project isolation** — projects don't share data
- **You control it** — delete `~/.kindling/` to clear all memory, or use `/memory forget` for individual items

## Requirements

- Claude Code
- The `kindling` binary on your `PATH` (powers both the capture/injection hooks and the `/memory` slash commands — see [Install](#install))
- Node.js >= 18 (already required by Claude Code; the `/memory` slash commands are thin Node wrappers that shell out to the `kindling` binary)

## License

Apache-2.0
