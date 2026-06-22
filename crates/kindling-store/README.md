# kindling-store

**SQLite persistence for kindling** — local-first memory for AI-assisted development.

`kindling-store` is the local persistence layer for kindling. It stores observations, capsules, summaries, pins and retrieval indexes in SQLite, with FTS5 and WAL mode enabled.

Most applications should use [`kindling-client`](https://crates.io/crates/kindling-client) or [`kindling-service`](https://crates.io/crates/kindling-service) rather than depending on this crate directly.

## Install

```toml
[dependencies]
kindling-store = "0.2"
```

## When to use this crate

Use `kindling-store` when you are:

- Extending kindling's persistence layer.
- Testing or inspecting storage behaviour directly.
- Building a custom service around the kindling schema.
- Working on migrations, SQLite tuning or project database layout.

## Storage model

kindling stores local development memory as structured records:

- **Observations:** captured tool calls, commands, errors, file diffs and messages.
- **Capsules:** bounded sessions or workflow runs.
- **Summaries:** compacted context for continuity.
- **Pins:** user-controlled priority memory.
- **Retrieval indexes:** local search over captured context.

## Documentation

Full docs: **[docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)**

Relevant docs:

- [Core concepts](https://docs.eddacraft.ai/kindling/concepts/capsules)
- [Configuration](https://docs.eddacraft.ai/kindling/reference/config)

## Licence

Apache-2.0
