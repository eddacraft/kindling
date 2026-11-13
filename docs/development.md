# Development Guide

How to work on the kindling engine. It covers where code lives, which generated
files and drift gates exist, and step-by-step recipes for the changes people
make most often.

Before you start:

- [CONTEXT.md](../CONTEXT.md) defines the domain vocabulary. Use its terms.
- [architecture.md](architecture.md) explains the crate topology and data flow.
- [CONTRIBUTING.md](../CONTRIBUTING.md) explains what we accept, the PR process,
  and commit style.

## Toolchain

- Rust: the version is pinned by `rust-toolchain.toml`, with an MSRV of 1.89 in
  `Cargo.toml`.
- Node.js 24 or later and pnpm: only needed for `packages/` (the thin client and
  adapters), `plugins/kindling-claude-code`, the binding copy step, and the
  Markdown/TS formatter (`oxfmt`).

```bash
cargo build && cargo test                       # engine
pnpm install && pnpm build && pnpm test         # TS packages (optional)
```

## Repository map

| Path                              | What it is                                                                                     |
| --------------------------------- | ---------------------------------------------------------------------------------------------- |
| `crates/`                         | The engine, as a Rust workspace. See [architecture.md § Crates](architecture.md#crates).       |
| `schema/`                         | The canonical SQLite schema contract (`schema.sql`, `version.json`).                           |
| `crates/kindling-types/bindings/` | Generated ts-rs TypeScript bindings. Committed and drift-checked.                              |
| `crates/kindling-store/schema/`   | Vendored copy of `schema/`. Committed and drift-checked.                                       |
| `packages/kindling/`              | Thin TS HTTP-over-UDS client. `src/generated/` is a copy of the bindings.                      |
| `packages/kindling-adapter-*`     | OpenCode, PocketFlow, and VS Code adapters (private, not on npm).                              |
| `plugins/kindling-claude-code/`   | Claude Code plugin: `hooks.json` that calls `kindling hook <type>`, plus `/memory-*` commands. |
| `examples/`                       | Minimal adapter example.                                                                       |
| `fixtures/`                       | Sample Claude Code hook payloads used by tests and examples.                                   |
| `packaging/`, `install.sh`        | Homebrew tap and install script.                                                               |
| `scripts/`                        | Release, publish, schema sync, and npm deprecation scripts.                                    |
| `docs/`                           | Developer and user docs. See [docs/README.md](README.md).                                      |
| `plans/`                          | APS plans: roadmap, modules, decisions (`D-0xx`), specs, reviews.                              |

## Generated files and drift gates

CI fails if any of these are stale. Regenerate them and commit the result in the
same PR.

| If you changed…                              | Run                                                                                                     | CI job                                 |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| A type in `kindling-types` that derives `TS` | `cargo test -p kindling-types --features ts-rs`, then `node packages/kindling/scripts/sync-types.mjs`   | `ts-bindings`, `ts-generated-copy`     |
| `schema/schema.sql` or `schema/version.json` | `scripts/sync-vendored-schema.sh`, then the binding steps above (the thin client copies the schema too) | `vendored-schema`, `ts-generated-copy` |
| Any Rust code                                | `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings`                        | `fmt`, `clippy`                        |
| TS or Markdown                               | `pnpm run format` (oxfmt) and `pnpm run lint` (oxlint)                                                  | `ci.yml / check`                       |

The `windows-tcp` job runs the loopback-TCP transport. If you touch
`kindling-server` or `kindling-client` transport code, make sure your change also
works on the non-UDS path.

## Tests

Each crate keeps its unit tests inline and its integration tests in
`crates/<crate>/tests/`. These test suites are worth knowing:

| Suite                                                                           | Guards                                                                              |
| ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `kindling-store/tests/golden_file.rs`                                           | Fresh-database DDL matches the schema contract.                                     |
| `kindling-provider/tests/ts_parity.rs`                                          | Ranking matches the original TS provider (fixtures).                                |
| `kindling-service/tests/filter_node_fixtures.rs`                                | Secret masking and truncation match the Node hook filter byte for byte.             |
| `kindling-types/tests/ts_export.rs`, `registry.rs`                              | Bindings export and the kind registry stays consistent with `ObservationKind::ALL`. |
| `kindling-server/tests/api.rs`                                                  | The v1 HTTP API over a real socket.                                                 |
| `kindling-client/tests/spool.rs`, `tcp.rs`                                      | Spool delivery and retention, and the TCP and bearer path.                          |
| `kindling/tests/parity.rs`, `capture_mapping.rs`                                | Hook capture mapping and injection Markdown match the original Node hooks.          |
| `kindling/tests/e2e.rs`, `daemon.rs`, `integration_matrix.rs`                   | The binary end to end against a spawned daemon.                                     |
| `kindling/tests/install_script.rs`, `homebrew_formula.rs`, `release_scripts.rs` | Packaging and release scripts.                                                      |

Performance work needs before-and-after numbers from `kindling-bench`. See its
[README](../crates/kindling-bench/README.md).

## Invariants to preserve

Reviewers check these. Breaking one needs an issue and a decision first.

1. **Determinism.** Retrieval and list ordering must not depend on hash-map
   iteration, wall-clock time (use `retrieve_at` or explicit timestamps), or
   insertion races. Break ties on stable keys (`id`).
2. **Masking is server-side.** Secret filtering happens in `kindling-service` on
   append. Never move it into a client or adapter, and never add an append path
   that skips the service.
3. **The daemon is the only writer in multi-tool setups.** New integrations
   should use `kindling-client` or `kindling-runtime`, not open the database
   directly.
4. **Mechanism, not policy.** No server-side aggregation, scoring policy,
   promotion, or token budgeting. Expose data and let downstreams decide.
5. **Wire compatibility.** The v1 API is camelCase JSON. Adding optional fields
   is fine. Renaming or removing fields, or changing semantics, is a breaking
   change.
6. **`unsafe_code = "forbid"`** applies to the whole workspace.

## Recipes

### Add an observation kind

An observation kind is part of the schema contract and the public wire format.
Open an issue first.

1. `crates/kindling-types/src/observation.rs`: add the variant, add it to
   `ObservationKind::ALL`, and add its required provenance fields in
   `capability.rs` (the kind registry).
2. `schema/schema.sql`: extend the `kind IN (…)` `CHECK` list. Changing a `CHECK`
   constraint is a breaking schema change: follow
   [schema/README.md](../schema/README.md#how-to-add-a-migration), including a
   migration (SQLite cannot alter a `CHECK` in place).
3. Regenerate the bindings and the thin-client copy (see the drift-gate table).
4. Update [data-model.md](data-model.md), [architecture.md](architecture.md#domain-model),
   [CONTEXT.md](../CONTEXT.md), and `AGENTS.md`.

### Change the schema

Follow [schema/README.md § How to add a migration](../schema/README.md#how-to-add-a-migration).
Then update the schema version in `architecture.md`.

### Add or change a v1 endpoint

1. Request and response types: put shared domain shapes in `kindling-types`, and
   server-only DTOs in `kindling-server/src/dto.rs`.
2. Logic goes in `kindling-service` (or `kindling-provider` for ranking), never in
   the handler.
3. `kindling-server/src/handlers.rs` holds the handler, and `build_router` in
   `lib.rs` holds the route. Data routes must require `X-Kindling-Project`.
4. Add a client method in `kindling-client/src/lib.rs`, and the mirror method in
   `packages/kindling/src/client.ts`.
5. Add a test in `kindling-server/tests/api.rs` and a client test.
6. Update the endpoint table in [architecture.md](architecture.md#kindling-server)
   and the thin-client README.

### Add a CLI subcommand

1. `crates/kindling/src/cli.rs`: add the `Command` variant and its args struct.
   Include `CommonOpts` if it reads the database, so it gets `--db` and `--json`.
2. `crates/kindling/src/commands/<name>.rs`: implement it and register it in
   `commands/mod.rs`. Dispatch happens in `lib.rs`.
3. Give it human output and `--json` output through `output.rs`.
4. Add a test in `crates/kindling/tests/cli.rs`.
5. Update the subcommand list in [architecture.md](architecture.md#eddacraft-kindling-binary)
   and `crates/kindling/README.md`.

### Change hook behaviour

The Rust hooks (`crates/kindling/src/hook/`) replaced Node hooks and must stay
byte-compatible with them:

- Capture mapping (`mapping.rs`, `filter.rs`) is covered by `capture_mapping.rs`.
- Injection Markdown (`kindling-server/src/inject.rs`) is covered by `parity.rs`.
  The date format deliberately reproduces Node's en-US `toLocaleString`.
- Project-root resolution (`project.rs`) decides which database a hook writes to.
  A change here can silently split a user's memory across two databases.

If you change the output on purpose, update the fixtures in the same PR and say
so in the PR description.

### Release

See [guides/release-runbook.md](guides/release-runbook.md). Nothing publishes to
npm.
