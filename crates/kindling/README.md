# kindling

**The local-first memory CLI for AI-assisted development.**

`kindling` captures useful development context — commands, tool calls, file edits, errors, summaries and pinned findings — and makes it searchable later. It is designed for AI coding workflows where continuity matters and context should stay local.

## Install

```bash
cargo install eddacraft-kindling
```

This installs the `kindling` binary. (The crate is published as
`eddacraft-kindling` because the bare `kindling` name on crates.io is taken by
an unrelated project; the command you run is still `kindling`.)

## Quick start

```bash
kindling demo
kindling search "JWT"
kindling browse
kindling log "JWT tokens expire after 15 minutes, not 1 hour"
kindling status
```

To run the daemon:

```bash
kindling serve
```

For Claude Code hook usage:

```bash
kindling hook session-start
kindling hook post-tool-use
kindling hook stop
```

Most users should start with the CLI and only move to the Rust SDK crates when building integrations.

## What the binary includes

The `kindling` binary provides:

- CLI commands for logging, searching, capsules, pins, export/import and status.
- A daemon entry point via `kindling serve`.
- Hook support for Claude Code session capture.
- Local SQLite-backed project memory.

## Building Rust integrations?

- [`kindling-client`](https://crates.io/crates/kindling-client) — daemon-backed SDK, safe across concurrent tools. The default choice.
- [`kindling-service`](https://crates.io/crates/kindling-service) — embedded, in-process, zero-IPC access.

## Documentation

Full docs: **[docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)**

Start with:

- [Install guide](https://docs.eddacraft.ai/kindling/quickstart/install)
- [CLI reference](https://docs.eddacraft.ai/kindling/reference/cli)
- [Configuration](https://docs.eddacraft.ai/kindling/reference/config)

## Licence

Apache-2.0
