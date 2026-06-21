# Changelog

All notable changes to the kindling project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Heads-up on the road to 1.0.0.** kindling is being re-implemented in Rust
> as the canonical engine, with `@eddacraft/kindling` repurposed as a thin
> HTTP-over-UDS client for the Rust `kindling` binary (installed separately via
> `cargo install eddacraft-kindling` or the install script). The
> existing TypeScript implementation packages (`-core`, `-store-sqlite`,
> `-store-sqljs`, `-provider-local`, `-server`, `-cli`) are deprecated and
> will be removed at 1.0.0. The 0.1.x line continues to receive maintenance
> until the Rust cutover lands. See
> `plans/specs/2026-05-03-rust-canonical-thin-client-design.md`.

## [Unreleased]

The Rust cutover. kindling is now Rust-canonical: the engine ships as a Cargo
workspace (`crates/`) publishing to crates.io, and the npm `@eddacraft/kindling`
package is a thin client over the Rust binary. The Rust workspace is versioned
independently and lands its first crates.io publish at `0.1.0`.

### Added

- **Rust engine** published to crates.io: `eddacraft-kindling` (the `kindling`
  CLI binary + Claude Code hooks), `kindling-client` (daemon-backed SDK, the
  default for integrations), `kindling-service` (embedded, in-process),
  `kindling-server` (daemon runtime, HTTP/1 over a Unix domain socket),
  `kindling-store` (SQLite + FTS5 + WAL), `kindling-provider` (deterministic
  FTS5 BM25 + recency retrieval) and `kindling-types` (shared domain types with
  optional `ts-rs` bindings). The binary crate is named `eddacraft-kindling`
  because `kindling` is taken on crates.io by an unrelated project; the
  installed command stays `kindling`.
- **`kindling serve`** daemon with per-project routing (`X-Kindling-Project`),
  auto-spawn on first client use, UDS transport (TCP loopback fallback on
  Windows), PID lock, and idle shutdown (default 30 min). Adds a `--daemonize`
  flag (PORT-016/017).
- **`kindling-client`** re-exports the domain types so it is a standalone SDK.
- **`kindling forget`** verb / `POST /v1/observations/:id/forget` for redaction;
  plugin `/memory` commands now run against the `kindling` binary (PORT-015).
- **Public crate READMEs and metadata** for the crates.io launch, plus
  cargo-publish readiness and install channels (PORT-014); `install.sh`.
- **Self-contained npm install:** `@eddacraft/kindling` now ships the `kindling`
  binary through per-platform `optionalDependencies`
  (`@eddacraft/kindling-<os>-<arch>[-musl]`, one prebuilt binary each with
  `os`/`cpu`/`libc` fields). Your package manager downloads only the matching
  one — no postinstall, works under `--ignore-scripts`. `$KINDLING_BIN` and a
  `kindling` on `PATH` still override the bundled binary. The packages are
  generated and published from the same cross-built artifacts as the GitHub
  Release (`scripts/build-platform-packages.mjs`).
- **Intent-capture health report** (KINTENT-006).

### Changed

- **Crate consolidation (11 → 7):** `filter` folded into `kindling-service`,
  `spool` into `kindling-client` (as an opt-in durable-emit feature), and
  `cli` + `hook` + the umbrella crate into a single `kindling` crate.
- **Adapters** (OpenCode, PocketFlow) and the **Claude Code hooks** were cut over
  to the thin client / `kindling` binary (PORT-015, PORT-019); the npm adapters
  now depend on `@eddacraft/kindling` rather than the TS core.
- **Branding:** lowercase `kindling`, `eddacraft`, and `anvil` in prose.

### Deprecated

- The TypeScript implementation packages (`-core`, `-store-sqlite`,
  `-store-sqljs`, `-provider-local`, `-server`, `-cli`) are deprecated (PORT-020)
  and will be removed at 1.0.0.

## [0.1.3] - 2026-05-08

### Added

- **CLI**: new `kindling log` and `kindling capsule` write commands, plus an
  install script and a substantially expanded README with cross-platform
  install guides.
- **Claude Code plugin**: `recall` skill for agent memory retrieval, with
  auto-invoke triggers and an explicit `/kindling:recall <query>` command.
- **Cross-language schema contract**: `schema/schema.sql` and
  `schema/version.json` are now the canonical SQLite schema for both the
  TypeScript store and the upcoming Rust store (SCHEMA-001..005).
- **Main package README** with cross-platform install instructions for
  `@eddacraft/kindling`.

### Changed

- **Dependencies**: bumped `fastify` 5.7.4 → 5.8.1 in `kindling-server`.
- **Repository structure**: `packages/kindling-api-server/` renamed to
  `packages/kindling-server/` to match the npm package name. The published
  package name (`@eddacraft/kindling-server`) is unchanged — no consumer
  action required.

### Internal

- Adopted a single-`main` branching model: feature work merges to `main`,
  releases are tagged from `main`, GitHub Releases on `main` trigger
  `publish.yml`. Documented in `docs/guides/`.
- Replaced `prettier` with `oxfmt` for formatting.
- Rebuilt the Claude Code plugin bundle.
- Began the Rust port (Phase 1, foundation): workspace scaffold landed in
  `crates/`. The crates are not published to npm and have no impact on this
  release — see the heads-up above.

## [0.1.2] - 2026-02-16

### Changed

- **Performance**: Denormalized scope ID columns replace `json_extract()` in queries (migration 004)
- **Performance**: FTS scoring moved to SQL with CTE-based queries; BM25 normalization done cross-table in JS
- **Performance**: Cached project root via `KINDLING_REPO_ROOT` env var in Claude Code hooks

### Fixed

- Shell argument injection in Claude Code command wrappers (`$ARGUMENTS` now quoted)
- Readonly export/import handles pre-migration-004 databases gracefully

### Internal

- Command scripts extracted from inline `node -e` blocks to standalone files
- Plugin bundle rebuilt with all optimizations

## [0.1.1] - 2025-02-10

### Changed

- Version bump for monorepo release consistency (no functional changes)

## [0.1.0] - 2025-02-09

### Added

- Initial public release
- Core domain model with observations, capsules, summaries, and pins
- SQLite persistence with FTS5 full-text search
- sql.js WASM store for browser compatibility
- Local retrieval provider with deterministic ranking
- OpenCode session adapter
- PocketFlow workflow adapter with intent inference and confidence tracking
- CLI tools for status, search, list, pin, export, import, and serve commands
- API server for multi-agent concurrency
- GitHub sync commands for Claude Code Web integration
- Automatic secret detection and redaction
- Export/import functionality for data portability
- Test coverage across all packages

### Security

- Automatic redaction of secrets in captured content
- Configurable excluded file patterns for sensitive paths
- Bounded output capture to prevent excessive storage
