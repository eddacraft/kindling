# Quickstart without Claude Code

Use kindling as a standalone local memory engine. No Claude Code hooks, no IDE
plugin, and no Rust toolchain required for the basic path.

kindling captures what happened in your project (commands, errors, decisions,
summaries) and retrieves it later with deterministic, explainable results.
Everything stays on your machine in SQLite.

## Install

Download the prebuilt `kindling` binary with the one-line installer:

```bash
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
```

The script detects your platform, verifies the release checksum, and installs
`kindling` to `~/.local/bin` by default. Add that directory to your `PATH` if the
installer asks you to.

Other install options:

- **Cargo:** `cargo install eddacraft-kindling`
- **npm (thin client + bundled binary):** `npm install @eddacraft/kindling`
- **Homebrew (macOS, planned):** see [packaging/homebrew-tap/README.md](../../packaging/homebrew-tap/README.md)

Full install guide: [docs.eddacraft.ai/kindling/quickstart/install](https://docs.eddacraft.ai/kindling/quickstart/install)

## Try it with sample data

Load the bundled demo dataset so you can search and browse immediately:

```bash
kindling demo
```

This imports sample observations, capsules, summaries and pins into
`~/.kindling/demo/kindling.db`. The command prints suggested next steps,
including the exact `--db` flag for later commands.

## Search your memory

Search across captured context with full-text retrieval:

```bash
kindling search "JWT" --db ~/.kindling/demo/kindling.db
```

Results are ranked deterministically: pins first, then the current session
summary, then provider hits with provenance explaining why each item matched.

Narrow the scope when you have multiple projects or sessions:

```bash
kindling search "authentication" --session session-1 --repo ./my-project
```

## Browse in a local viewer

Export your database to a self-contained HTML page and inspect it offline:

```bash
kindling browse --db ~/.kindling/demo/kindling.db --no-open
```

The command prints the path to the generated HTML file. Open it in any browser,
or omit `--no-open` to launch your default browser automatically.

## Capture context manually

You do not need an adapter to write memory. Log observations from the shell:

```bash
kindling log "JWT tokens expire after 15 minutes, not 1 hour"
kindling log --kind error "segfault in auth middleware after upgrade"
```

Open and close capsules for bounded sessions:

```bash
kindling capsule open --intent "investigating memory leak" --repo ./my-project
kindling capsule close cap_abc123 --summary "root cause: unbounded cache in SessionStore"
```

Pin important findings so they always appear first in searches:

```bash
kindling pin observation obs_abc123 --note "Root cause identified"
```

## Optional: daemon mode for multi-tool access

When several tools or agents need to share the same project memory, start the
daemon:

```bash
kindling serve
```

The daemon listens on a Unix domain socket (`~/.kindling/kindling.sock` by
default). Rust integrations use [`kindling-client`](https://crates.io/crates/kindling-client);
Node integrations use [`@eddacraft/kindling`](https://www.npmjs.com/package/@eddacraft/kindling).
Both auto-spawn the daemon on first use if it is not already running.

## Connect your own tool

To capture context automatically from your editor, agent framework or workflow
runner, build a thin adapter over the Node client or Rust SDK.

- **Integrations matrix:** [docs/integrations.md](../integrations.md)
- **Adapter cookbook (10 minutes):** [docs/adapters/cookbook.md](../adapters/cookbook.md)
- **Minimal working example:** [examples/adapter-minimal](../../examples/adapter-minimal/)

## What next?

| Goal                                         | Where to go                                                                                                      |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Core concepts (observations, capsules, pins) | [docs.eddacraft.ai/kindling/concepts/capsules](https://docs.eddacraft.ai/kindling/concepts/capsules)             |
| CLI reference                                | [docs.eddacraft.ai/kindling/reference/cli](https://docs.eddacraft.ai/kindling/reference/cli)                     |
| Rust SDK                                     | [`kindling-client` on crates.io](https://crates.io/crates/kindling-client)                                       |
| OpenCode adapter                             | [`@eddacraft/kindling-adapter-opencode`](https://www.npmjs.com/package/@eddacraft/kindling-adapter-opencode)     |
| PocketFlow adapter                           | [`@eddacraft/kindling-adapter-pocketflow`](https://www.npmjs.com/package/@eddacraft/kindling-adapter-pocketflow) |
