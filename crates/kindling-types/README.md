# kindling-types

Canonical Rust types for Kindling — observations, capsules, summaries, pins,
retrieval. The wire format mirrors the existing TypeScript shapes in
`packages/kindling-core/src/types/` and is the contract every consumer
(Rust, TS-via-daemon, future bindings) must agree on.

## TypeScript projection

The `ts-rs` feature derives a TypeScript projection of every public type and
writes it under `bindings/` when tests run:

```sh
cargo test -p kindling-types --features ts-rs
```

The resulting `.ts` files are checked in. CI runs the same command and fails
if any binding has drifted, so a contributor changing a type cannot land the
Rust change without also refreshing the bindings.

## Wire-format conventions

- All structs are camelCase on the wire (`scope_ids` → `scopeIds`).
- Enum variants are snake_case strings (`ToolCall` → `tool_call`).
- Optional fields are omitted from JSON when `None`, never serialised as `null`.
- `Timestamp` and counts are plain JSON numbers; `i64` and `u64` are not used
  in public types (avoids the `bigint` hazard on the TypeScript side).
- `RetrievedEntity` is an untagged union of `Observation | Summary` so it
  matches the structural union TS already uses.
