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
- `Timestamp` and counts are plain JSON numbers. `Timestamp` is an `i64`
  alias (epoch milliseconds) and is the only integer wider than `i32` in
  public types; every field that holds a `Timestamp` carries a
  `#[ts(type = "number")]` override so the TypeScript projection emits
  `number`, not `bigint`. New fields with `Timestamp` must carry the same
  override — the round-trip and bindings tests will fail otherwise.
- `RetrievedEntity` is an untagged union of `Observation | Summary` so it
  matches the structural union TS already uses.
