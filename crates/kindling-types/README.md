# kindling-types

**Shared domain types for kindling** — local-first memory for AI-assisted development.

`kindling-types` defines the core data shapes used across kindling: observations, capsules, summaries, pins, retrieval results, scope identifiers and related API types.

Most Rust integrations should depend on [`kindling-client`](https://crates.io/crates/kindling-client), which re-exports these types. Use this crate directly only when you need the domain model without the client, service or storage layers.

## Install

```toml
[dependencies]
kindling-types = "0.2"
```

## When to use this crate

Use `kindling-types` when you are:

- Building a custom integration around kindling's data model.
- Sharing kindling types across crates without pulling in client or storage code.
- Generating or validating bindings.
- Working on protocol-level compatibility.

## TypeScript bindings

The crate supports optional TypeScript projection generation through the `ts-rs` feature:

```sh
cargo test -p kindling-types --features ts-rs
```

The resulting `.ts` files are checked in under `bindings/`; CI fails if any binding drifts from its Rust type.

## Documentation

Full docs: **[docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)**

Relevant docs:

- [Core concepts](https://docs.eddacraft.ai/kindling/concepts/capsules)
- [Writing adapters](https://docs.eddacraft.ai/kindling/adapters/custom)

## Licence

Apache-2.0
