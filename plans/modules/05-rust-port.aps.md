# Rust Port

| ID   | Owner  | Status |
| ---- | ------ | ------ |
| PORT | @aneki | Ready  |

## Purpose

Port Kindling's production surface to Rust as a dual-maintain project alongside the existing TypeScript packages, with Rust as the canonical source of truth for domain types. Replace the awkward TypeScript bridge between Anvil (nearly 100% Rust) and Kindling with direct Rust-to-Rust integration, and ship a single statically-linked `kindling` binary that covers hooks, CLI, and HTTP server.

Supersedes `02-rust-hook-binary` and `03-rust-cli`. Full rationale in `plans/specs/2026-04-15-rust-port-design.md`.

## In Scope

- Nine-crate Rust workspace (`kindling-types`, `-store`, `-provider`, `-service`, `-filter`, `-hook`, `-server`, `-cli`, umbrella `kindling`)
- Rust as the source of truth for domain types; `ts-rs` generates TypeScript `.d.ts`
- All 7 Claude Code hook types handled by `kindling-hook` (stdin/stdout JSON contract unchanged)
- All 12 CLI commands in `kindling-cli` (clap)
- HTTP API server in `kindling-server` (axum, same endpoints as Fastify)
- FTS5 retrieval with BM25 normalization (tiered: pins → summary → candidates)
- Content filtering (secret masking, truncation)
- Cross-platform release binaries (Linux x86_64/aarch64/musl, macOS x86_64/aarch64, Windows x86_64)
- Distribution via `cargo install kindling`, Homebrew tap, and `curl | sh` install script
- Direct Rust-to-Rust Anvil integration (`use kindling_service::KindlingService`)
- Deprecation and removal of `@eddacraft/anvil-kindling-integration` after Anvil cuts over

## Out of Scope

- Semantic search / embeddings (future work)
- Browser WASM store rewrite — `@eddacraft/kindling-store-sqljs` stays TypeScript
- Adapter packages (OpenCode, PocketFlow) — remain TypeScript consumers
- Removing the TypeScript npm surface — `@eddacraft/kindling-core` continues shipping, consuming generated types
- Schema migrations in Rust — TypeScript store remains the migration author; Rust implements against `schema/schema.sql`

## Interfaces

**Depends on:**

- `01-npm-publish` — stable npm surface so TS consumers have something to depend on while Rust catches up
- `04-schema-contract` (Done) — `schema/schema.sql` and `schema/version.json` are the cross-language contract both implementations read
- `schema/version.json` `PRAGMA user_version = 5` — Rust checks compat at startup

**Exposes:**

- `crates/` workspace at repo root, built with `cargo build --release`
- `kindling` binary (single statically-linked artifact) — `kindling-hook`, `kindling`, `kindling serve` subcommands
- Generated TypeScript types at `packages/kindling-core/src/generated/` from `ts-rs`
- `cargo install kindling` and Homebrew formula

**Supersedes:**

- `02-rust-hook-binary` — Phase 2 of this module absorbs HOOK-001..HOOK-008
- `03-rust-cli` — Phase 3 of this module absorbs CLI-001..CLI-005

## Ready Checklist

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] Design spec written (`plans/specs/2026-04-15-rust-port-design.md`)
- [x] Tasks broken down by phase

## Phases

| Phase | Tasks         | Outcome                                                             |
| ----- | ------------- | ------------------------------------------------------------------- |
| 1     | PORT-001..004 | Foundation: workspace, types, store, filter                         |
| 2     | PORT-005..010 | Service + Hook: Anvil unblocks, hook binary ships                   |
| 3     | PORT-011..014 | CLI + Server: single `kindling` binary distributed everywhere       |
| 4     | PORT-015..017 | Type bridge: TS packages consume generated types; TS bridge retired |

## Tasks

### Phase 1 — Foundation

#### PORT-001: Rust workspace scaffold

- **Intent:** Cargo workspace initialized with the 9 crates and CI baseline
- **Expected Outcome:** `crates/` directory at repo root with `Cargo.toml` workspace manifest; all 9 crate skeletons compile (`cargo build --workspace`); `.github/workflows/rust.yml` runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test --workspace` on every push
- **Validation:** `cargo build --workspace --release` succeeds locally and in CI
- **Status:** Ready

#### PORT-002: kindling-types crate

- **Intent:** Canonical Rust definitions for `Observation`, `Capsule`, `Retrieval`, `ScopeIds`, `Id`, `Timestamp`, `Result<T>` with `ts-rs` derives
- **Expected Outcome:** Types in `kindling-types` match shapes in `packages/kindling-core/src/types/*.ts`; `#[derive(TS)]` produces `.d.ts` output that round-trips JSON with TS originals
- **Validation:** `cargo test -p kindling-types` passes including round-trip tests against sample JSON fixtures; `ts-rs` export runs clean
- **Status:** Ready
- **Dependencies:** PORT-001

#### PORT-003: kindling-store crate

- **Intent:** SQLite persistence layer implemented in Rust against `schema/schema.sql`
- **Expected Outcome:** `rusqlite` with `bundled` feature; reads `PRAGMA user_version` and asserts compatibility with `schema/version.json`; supports open/close capsules, append observation, attach observation to capsule, insert pin/unpin, and all FTS-indexed writes; WAL mode enabled; per-project database isolation (`~/.kindling/projects/<hash>/`)
- **Validation:** `cargo test -p kindling-store` passes integration tests against a temp database; a database created by the TypeScript store is readable by the Rust store (golden file test)
- **Status:** Ready
- **Dependencies:** PORT-002, schema contract (module 04, Done)

#### PORT-004: kindling-filter crate

- **Intent:** Content filtering (secret masking, truncation, excluded-path filtering) matching Node.js behavior byte-for-byte
- **Expected Outcome:** API keys, tokens, and passwords redacted with the same patterns as the Node.js filter; content truncated at the same limits; excluded paths filtered using the same rules
- **Validation:** `cargo test -p kindling-filter` passes filter tests with known secret patterns; snapshot tests compare filter output against Node.js fixtures
- **Status:** Ready
- **Dependencies:** PORT-001

### Phase 2 — Service + Hook

#### PORT-005: kindling-provider crate

- **Intent:** Local FTS retrieval provider with BM25 normalization and tiered retrieval
- **Expected Outcome:** FTS5 search with BM25 scoring normalized to [0, 1]; tiered retrieval (pins → current summary → ranked candidates); deterministic ordering; `RetrieveResult` shape matches the TS provider
- **Validation:** `cargo test -p kindling-provider` passes; identical queries against the same database produce the same ranked results in Rust and TS (cross-implementation parity test)
- **Status:** Ready
- **Dependencies:** PORT-003

#### PORT-006: kindling-service crate

- **Intent:** Full orchestration layer — `openCapsule`, `closeCapsule`, `appendObservation`, `retrieve`, `pin`, `unpin` — available as a library
- **Expected Outcome:** `KindlingService::new(config)` returns a service handle; all six methods behave identically to `@eddacraft/kindling-core`'s `KindlingService`; errors propagate via the Result type pattern
- **Validation:** `cargo test -p kindling-service` passes; contract tests comparing service outputs against the TS service for identical inputs
- **Status:** Ready
- **Dependencies:** PORT-003, PORT-004, PORT-005

#### PORT-007: kindling-hook crate

- **Intent:** All 7 Claude Code hook types (session-start, post-tool-use, post-tool-use-failure, user-prompt-submit, subagent-stop, pre-compact, stop) handled via stdin JSON
- **Expected Outcome:** Binary reads Claude Code hook context from stdin, performs the correct DB operation via `kindling-service`, returns JSON response on stdout matching the Node.js script contract exactly
- **Validation:** Integration tests pipe JSON fixtures through the binary and verify DB state; hook latency <10ms measured over the 7 hook types
- **Status:** Ready
- **Dependencies:** PORT-006

#### PORT-008: Context injection

- **Intent:** SessionStart and PreCompact hooks inject prior context
- **Expected Outcome:** SessionStart returns pins + recent observations in the injected context JSON; PreCompact returns pins + latest summary; structure identical to Node.js hook output
- **Validation:** Integration test verifies injected context JSON structure matches Node.js output byte-for-byte on identical fixtures
- **Status:** Ready
- **Dependencies:** PORT-007

#### PORT-009: Cross-platform CI hook builds

- **Intent:** Hook binary builds for all target platforms from Phase 2 onward
- **Expected Outcome:** GitHub Actions produces release artifacts for Linux (x86_64, aarch64, musl), macOS (x86_64, aarch64), Windows (x86_64); `cross` or `cargo-zigbuild` handles cross-targets; artifacts are downloadable from the release page
- **Validation:** CI matrix green; each artifact runs `kindling-hook --version` successfully on the target platform
- **Status:** Ready
- **Dependencies:** PORT-007

#### PORT-010: Anvil integration proof

- **Intent:** Demonstrate direct Rust-to-Rust integration from Anvil without the TS bridge
- **Expected Outcome:** One Anvil crate (pick the observation emitter that would otherwise call the TS bridge) depends on `kindling-service` and `kindling-types`, emits an observation directly to the Kindling database, and matches the output produced by the TS bridge for the same input
- **Validation:** Parity test in Anvil comparing TS-bridge-emitted observation vs. Rust-direct-emitted observation for the same input; both land identically in the Kindling database
- **Status:** Ready
- **Dependencies:** PORT-006

### Phase 3 — CLI + Server

#### PORT-011: kindling-cli crate

- **Intent:** All 12 CLI commands via `clap` (init, log, capsule open/close, status, search, list, pin, unpin, export, import, serve)
- **Expected Outcome:** `kindling status`, `kindling search`, `kindling list`, etc. all work with both JSON and text output modes; flags and output shapes match the Commander.js CLI
- **Validation:** Integration tests for each command; snapshot tests comparing JSON output against the TS CLI for identical inputs
- **Status:** Ready
- **Dependencies:** PORT-006

#### PORT-012: kindling-server crate

- **Intent:** HTTP API server in `axum` with the same endpoints as the Fastify server
- **Expected Outcome:** `kindling serve` starts the HTTP API on `127.0.0.1:8080`; every route from `@eddacraft/kindling-server` is implemented with identical request/response shapes
- **Validation:** Existing API integration tests (from `packages/kindling-server/`) pass against the Rust server; contract test hits both servers and compares responses
- **Status:** Ready
- **Dependencies:** PORT-006

#### PORT-013: Umbrella `kindling` binary

- **Intent:** Single binary entry point that dispatches to hook, CLI, or server based on subcommand or invocation name
- **Expected Outcome:** One artifact: `kindling hook`, `kindling <cli-command>`, `kindling serve`; symlink-aware so `kindling-hook` continues to work as a drop-in replacement for the Node.js hook scripts
- **Validation:** Single binary size under 20 MB stripped; all three surfaces tested via the umbrella binary
- **Status:** Ready
- **Dependencies:** PORT-007, PORT-011, PORT-012

#### PORT-014: Distribution

- **Intent:** Multiple install paths for the unified binary
- **Expected Outcome:** `cargo install kindling` publishes to crates.io; Homebrew tap at `eddacraft/kindling` with formula; `curl -sSL install.kindling.dev | sh` installs the latest release binary for the detected platform; plugin `hooks.json` updated to call the installed binary
- **Validation:** Fresh install on Linux, macOS, and Windows via each method; post-install Claude Code session exercises all 7 hook types end-to-end
- **Status:** Draft
- **Dependencies:** PORT-009, PORT-013

### Phase 4 — Type bridge + deprecation

#### PORT-015: ts-rs export pipeline

- **Intent:** Generated TypeScript type definitions available for the npm packages to consume
- **Expected Outcome:** `cargo test -p kindling-types --features ts-rs` writes `.d.ts` files to `packages/kindling-core/src/generated/`; generated files are committed; CI fails if generated output drifts from the committed files
- **Validation:** CI job re-runs `ts-rs` and `git diff --exit-code packages/kindling-core/src/generated/` — passes if no drift
- **Status:** Draft
- **Dependencies:** PORT-002

#### PORT-016: TypeScript core refactor

- **Intent:** `@eddacraft/kindling-core` becomes a thin wrapper over generated types
- **Expected Outcome:** Domain types in `kindling-core/src/types/` re-export from `src/generated/`; validation helpers stay hand-written; public API unchanged for downstream adapter consumers; `kindling-store-sqljs` consumes generated types for in-browser round-tripping
- **Validation:** `pnpm run type-check` and `pnpm run test` pass across all TS packages; adapter packages compile without changes
- **Status:** Draft
- **Dependencies:** PORT-015

#### PORT-017: Anvil TS bridge deprecation

- **Intent:** Remove the TypeScript bridge after Anvil crates cut over to `kindling-service`
- **Expected Outcome:** Anvil's observation emitters across all relevant crates use `kindling_service::KindlingService` directly; `@eddacraft/anvil-kindling-integration` package is marked deprecated on npm with a terminal warning on install; eventual removal tracked in the Anvil repo
- **Validation:** Anvil builds without depending on `@eddacraft/anvil-kindling-integration`; a full EddaCraft dev session emits observations end-to-end via Rust-only path
- **Status:** Draft
- **Dependencies:** PORT-010, PORT-016
