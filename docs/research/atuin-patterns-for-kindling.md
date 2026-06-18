# Patterns from Atuin Applicable to kindling

Research date: 2026-02-23
Source: https://github.com/atuinsh/atuin

## Background

[Atuin](https://github.com/atuinsh/atuin) replaces traditional shell history
(`~/.bash_history`) with a SQLite database capturing rich metadata per command:
working directory, exit code, duration, hostname, session ID, and timestamps. It
provides interactive full-screen TUI search (replacing Ctrl+R), optional E2E
encrypted sync across machines, and has expanded into dotfiles management and
executable runbooks. 200k+ developers use it.

Both Atuin and kindling share core architectural DNA: local SQLite storage,
scope-based filtering, hook-driven context capture, and local-first design with
optional sync. The differences are in domain (shell commands vs AI workflow
observations) and maturity of specific subsystems.

---

## Near-term (small delta from current architecture)

### 1. Workspace auto-detection

**Atuin pattern:** A "workspace" filter mode auto-activates when the user is
inside a git repo, filtering history to commands from _any_ directory within that
repo's tree. No flags required.

**kindling opportunity:** kindling has `repoId` as a scope dimension but it
requires explicit passing (`--repo`). Auto-detecting the current git repo root
from `cwd` at retrieval time and defaulting `repoId` to it would make search
contextual by default. The Claude Code adapter and CLI could both do this.

**Effort:** Low. `git rev-parse --show-toplevel` or equivalent check, default
scope population in CLI search and adapter hooks.

### 2. Command execution metadata

**Atuin pattern:** Stores exit code, duration (nanoseconds), and working
directory per command.

**kindling opportunity:** The `command` observation type captures the command
text but not its outcome. Enriching `provenance` (already a JSON field) with
`exitCode`, `durationMs`, and `cwd` would improve retrieval ranking — failed
commands and long-running commands are disproportionately worth remembering.
Could also feed into stats.

**Effort:** Low. No schema changes needed — `provenance` is already freeform
JSON. Adapter-side changes only.

### 3. Stats / analytics command

**Atuin pattern:** `atuin stats` shows most-used commands, frequency
distributions, configurable subcommand grouping (e.g. `kubectl get` not just
`kubectl`).

**kindling opportunity:** `kindling stats` could show:

- Observation counts by kind over time
- Most-referenced capsules
- Common error patterns (frequent error observation content)
- Pin usage and churn
- FTS hit rate / retrieval effectiveness
- Session duration distribution

All queryable from the existing schema with straightforward SQL.

**Effort:** Low-medium. New CLI command, a handful of aggregate queries.

### 4. Richer search filters (CLI flags)

**Atuin pattern:** `atuin search --exit 0 --after "yesterday 3pm" make` —
combining content matching with structured metadata filters.

**kindling opportunity:** Current `kindling search` takes a query and optional
`--session`/`--repo`. Adding filters:

- `--kind error|command|file_diff|...`
- `--after "yesterday"` / `--before "2 days ago"`
- `--capsule-type session|pocketflow_node`
- `--status open|closed` (capsule status)

These map to existing indexed columns.

**Effort:** Low. CLI argument parsing + WHERE clause construction. The FTS
provider and orchestrator already support scope filtering.

---

## Medium-term (new subsystems, but fits existing architecture)

### 5. Interactive TUI search

**Atuin pattern:** Full-screen interactive search with live-toggleable filter
modes (session / directory / host / global / workspace). `Alt+number` quick
jump, `Ctrl+O` command inspector, tab-to-edit.

**kindling opportunity:** kindling's CLI search returns results and exits. An
interactive mode where you type queries, toggle scope filters in real-time,
preview capsule/observation contents, and select results to expand would be a
significant usability upgrade. Consider `ink` (React for CLIs), `blessed`, or
`@inquirer/prompts`.

**Effort:** Medium. New rendering layer, but retrieval backend already exists.

### 6. E2E encrypted sync

**Atuin pattern:** PASETO v4 + PASERK per-record key wrapping. Each record gets
its own random encryption key, which is then wrapped with the master key. Server
never sees plaintext. Key rotation only re-wraps keys, not data.

**kindling opportunity:** Current sync is GitHub-based (push to private repo,
cleartext in the repo). Adopting a wrapped-key model where observation/capsule
content is encrypted before leaving the machine would be a stronger privacy
guarantee. The sync index (capsule IDs, timestamps, types) could stay cleartext
for discoverability while content stays encrypted.

**Design note:** Atuin's per-host record chains eliminate merge conflicts — each
machine owns its own append-only stream. kindling's capsules are already
scoped by sessionId/repoId, making a similar conflict-free design natural.

**Effort:** Medium-high. New encryption module, key management UX, migration
path for existing synced data.

### 7. Directory-scoped retrieval

**Atuin pattern:** A "directory" filter scopes history to the exact current
working directory (distinct from "workspace" which covers the whole repo).

**kindling opportunity:** In monorepos, `repoId` is too broad. A `directoryId`
or `cwd` field on observations would enable "what happened in this folder"
queries. Useful for large projects with distinct subsystems.

**Effort:** Medium. New scope dimension, schema addition, adapter changes.

---

## Longer-term / Speculative

### 8. Direct shell integration via preexec hooks

**Atuin pattern:** Hooks into bash/zsh/fish via preexec/precmd to capture every
command automatically with zero friction.

**kindling opportunity:** Currently captures through AI-tool adapters only.
Adding shell hooks would capture the full developer workflow — manual git,
make, npm, docker commands that happen _between_ AI interactions. This makes
memory much more complete and enables "what did I do to fix this last time"
retrieval.

**Implementation note:** Could be a new `kindling-adapter-shell` package that
emits `command` observations. Atuin's approach of `eval "$(kindling init zsh)"`
is clean.

### 9. Dotfiles / environment capture

**Atuin pattern:** Expanded into syncing aliases and env vars across machines.

**kindling opportunity:** Capsules could capture environment snapshots: tool
versions, relevant config, env vars at session start. Enables "what was
different about my environment when this worked" debugging. Not sync per se, but
recording environment state as observations.

### 10. Cross-machine identity

**Atuin pattern:** Hostname per command, global sync, search across all machines.

**kindling opportunity:** The `userId` scope dimension exists but is unused. A
multi-machine kindling where memory spans devices ("I solved this on my laptop
last week") would require real sync infrastructure but the scope model already
supports it.

---

## Design patterns to adopt (not features, but principles)

| Atuin pattern                                                | kindling equivalent                                    | Gap                                 |
| ------------------------------------------------------------ | ------------------------------------------------------ | ----------------------------------- |
| Never modifies original data (shell history file untouched)  | Observations are immutable, redaction replaces content | Already aligned                     |
| Filter modes toggleable in real-time during search           | Retrieval scopes fixed per query                       | Interactive TUI would close this    |
| Capture everything by default, filter at retrieval           | Adapter-mediated capture (selective)                   | Shell integration would close this  |
| Zero-config works, power users tune via TOML                 | CLI requires explicit flags                            | Workspace auto-detection would help |
| Progressive enhancement (local → sync → kv → aliases)        | Local → GitHub sync                                    | Natural extension path exists       |
| Per-host append-only chains (conflict-free sync)             | Capsules scoped by session/repo                        | Good fit for future sync model      |
| Secrets filtering (regex patterns to exclude sensitive data) | Redaction API (explicit, after the fact)               | Could add pre-capture filtering     |

---

## What NOT to borrow

- **Executable runbooks / Desktop app** — PocketFlow already covers workflow
  orchestration. Different domain.
- **Sync server infrastructure** — Operating a Rust server with PostgreSQL is
  significant complexity. GitHub-based sync fits local-first ethos better for
  now.
- **Shell-as-primary-interface** — kindling's primary consumers are AI tools,
  not humans typing in terminals. Shell integration is additive, not a pivot.

---

## Recommended priority

1. **Workspace auto-detection** — highest signal-to-effort ratio
2. **Execution metadata in provenance** — trivial change, improves retrieval quality
3. **Richer CLI search filters** — straightforward, high usability impact
4. **Stats command** — fun, useful, builds on existing schema
5. **Interactive TUI** — significant UX upgrade, medium effort
6. **Pre-capture secrets filtering** — important for shell integration
7. **E2E encrypted sync** — when sync becomes more central
8. **Shell preexec integration** — when expanding beyond AI-tool capture
