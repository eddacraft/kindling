# Changelog

All notable changes to the Kindling project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Heads-up on the road to 1.0.0.** Kindling is being re-implemented in Rust
> as the canonical engine, with `@eddacraft/kindling` repurposed as a thin
> HTTP-over-UDS client that downloads the Rust binary at install time. The
> existing TypeScript implementation packages (`-core`, `-store-sqlite`,
> `-store-sqljs`, `-provider-local`, `-server`, `-cli`) are deprecated and
> will be removed at 1.0.0. The 0.1.x line continues to receive maintenance
> until the Rust cutover lands. See
> `plans/specs/2026-05-03-rust-canonical-thin-client-design.md`.

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

- Adopted the `main`/`dev` branching model: feature work merges to `dev`,
  releases promote `dev` → `main`, GitHub Releases on `main` trigger
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
- Comprehensive test coverage across all packages

### Security

- Automatic redaction of secrets in captured content
- Configurable excluded file patterns for sensitive paths
- Bounded output capture to prevent excessive storage
