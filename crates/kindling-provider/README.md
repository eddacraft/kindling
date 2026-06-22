# kindling-provider

**Deterministic local retrieval for kindling** — local-first memory for AI-assisted development.

`kindling-provider` contains kindling's local retrieval provider. It turns stored memory into explainable search results using SQLite FTS5, BM25-style ranking and kindling's retrieval tiers.

Most users should access retrieval through [`kindling-client`](https://crates.io/crates/kindling-client), [`kindling-service`](https://crates.io/crates/kindling-service) or the `kindling search` CLI command.

## Install

```toml
[dependencies]
kindling-provider = "0.2"
```

## When to use this crate

Use `kindling-provider` when you are:

- Working on retrieval ranking.
- Building a custom memory provider.
- Testing deterministic search behaviour.
- Extending how kindling selects candidate context.

## Retrieval model

kindling retrieval is intentionally layered:

1. **Pins** — user-marked priority memory.
2. **Current summary** — active capsule/session context.
3. **Provider hits** — ranked local search results with provenance.

That structure keeps retrieval predictable, explainable and suitable for AI-assisted development workflows.

## Documentation

Full docs: **[docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)**

Relevant docs:

- [Core concepts](https://docs.eddacraft.ai/kindling/concepts/capsules)
- [Writing adapters](https://docs.eddacraft.ai/kindling/adapters/custom)

## Licence

Apache-2.0
