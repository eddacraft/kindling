# Changelog

All notable changes to the kindling project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Heads-up.** kindling is now Rust-canonical: the engine is a Cargo workspace
> in `crates/`, and the npm `@eddacraft/kindling` package is a thin
> HTTP-over-UDS client that bundles the Rust `kindling` binary via per-platform
> `optionalDependencies` (or use `cargo install eddacraft-kindling` / the install
> script). As of 0.2.0 the retired TypeScript implementation packages have been
> removed from the repo; their last `0.1.x` npm versions remain published and
> are marked deprecated on the registry. See
> `plans/specs/2026-05-03-rust-canonical-thin-client-design.md`.

## [Unreleased]

## [0.3.0] - 2026-06-26

Downstream-integration release. The daemon/client contract anvil consumes gains
an exhaustive read API and a bounded spool, plus a one-dependency runtime facade.
This is a **breaking** release for `kindling-client` (`SpoolConfig` is now
`#[non_exhaustive]`); pre-1.0, a breaking change bumps the minor — hence 0.3.0.

### Breaking

- **`kindling-client::SpoolConfig` is now `#[non_exhaustive]`** and gains
  `max_bytes` / `max_age_ms` fields. Construct it via `SpoolConfig::new(path)`
  plus the `with_max_bytes` / `with_max_age_ms` builders rather than a struct
  literal.
- **`kindling-runtime`'s public config/error/strategy types are
  `#[non_exhaustive]`** (built via their constructors/builders).

### Added

- **Observation list/enumerate read API (KINTEG-003).** New
  `POST /v1/observations/list` and `Client::list_observations` return the _full_
  set of observations matching a `(kind, scope, half-open time-range)` filter,
  deterministically paginated with an opaque keyset cursor over the stable
  `(ts, id)` order — distinct from the ranked, capped `POST /v1/retrieve`. Lets a
  consumer compute exact counts / set-differences over every matching row;
  redacted rows are excluded unless `includeRedacted` is set. No schema change
  (still v5). Unblocks anvil's daemon-backed usage views (KDS-004).
- **Spool retention cap (KINTEG-009).** `SpoolConfig` gains optional
  `max_bytes` / `max_age_ms` caps (default unbounded). When set, `flush()` trims
  the oldest entries (age, then bytes) under its file lock — preserving drain
  order and never dropping an un-drained entry ahead of a kept newer one; shed
  entries are counted in `SpoolStatus::dropped_count`. Lets a downstream replace
  a rolling NDJSON sidecar without a retention regression (KDS-005 prerequisite).
- **`kindling-runtime` facade crate (KINTEG-008).** A single dependency bundling
  attach-or-start daemon wiring and a spooled client, so a Rust consumer can embed
  kindling without the `kindling` CLI on `PATH` or manual client + server glue.
  First crates.io publish.
- **Conversion surface** merged to main: `kindling demo`, `kindling browse`, `@eddacraft/kindling-adapter-vscode`, onboarding docs (`docs/quickstart/`, `docs/integrations.md`, adapter cookbook), Homebrew formula updates (macOS + Linux glibc), and automated homebrew-tap PR on release.

### Removed

- Retired TypeScript implementation package directories (`kindling-core`, `kindling-store-sqlite`, `kindling-store-sqljs`, `kindling-provider-local`, `kindling-server`, `kindling-cli`, `kindling-adapter-claude-code`) and stale `tsconfig.base.json` path entries.

### Fixed

- Claude Code plugin test script: `node --test test/*.test.js` (Node 26 no longer accepts a bare `test/` directory path).
- `kindling browse` HTML viewer: escape `</` in embedded export JSON so observation content cannot break out of the bundle `<script>` block.

## [0.2.0] - 2026-06-22

The Rust cutover. kindling is now Rust-canonical: the engine ships as a Cargo
workspace (`crates/`) publishing to crates.io, and the npm `@eddacraft/kindling`
package is a thin client over the Rust binary. The Rust workspace is versioned
independently; `0.1.0` was the first crates.io publish and `0.2.0` is the first
to carry the opt-in `spool` durable-emit feature — `kindling-client`'s
`SpooledClient` — which unblocks daemon-backed downstream consumers (anvil).

### Added

- **`SpooledClient` durable-emit layer** for `kindling-client`, behind the
  opt-in `spool` feature
  (`kindling-client = { version = "0.2.0", features = ["spool"] }`). Wraps the
  client so an `append_observation` that cannot reach the daemon buffers the
  observation to a local append-only NDJSON spool and replays it — in append
  order, idempotent on a stable observation id assigned before spooling — on the
  next successful append or an explicit `flush()`. The daemon's SQLite store
  stays the sole source of truth; the spool is a transient write buffer, never a
  parallel one. Delivery is **at-least-once** in 0.2.0; daemon-side dedup for
  exactly-once is a tracked follow-up (KINTEG-002). New to crates.io — the
  published `0.1.0` `kindling-client` shipped no `spool` feature. There is no
  standalone `kindling-spool` crate; the module lives in `kindling-client`.
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
- **npm packages versioned to 0.2.0** in lockstep — the thin `@eddacraft/kindling`
  client and the `opencode` / `pocketflow` adapters (the only remaining npm
  packages), bumped from the published `0.1.2`.

### Removed

- **Retired the TypeScript implementation packages** — `-core`, `-store-sqlite`,
  `-store-sqljs`, `-provider-local`, `-server`, `-cli`, and
  `-adapter-claude-code` — and removed their source from the workspace (PORT-020).
  The Rust daemon + the thin `@eddacraft/kindling` client own this surface;
  Claude Code support is now the `kindling` binary's built-in hooks. `packages/`
  contains only `@eddacraft/kindling` and the `opencode` / `pocketflow` adapters,
  and `pnpm install` no longer pulls `better-sqlite3` (native), `sql.js`, or
  `fastify`.

### Deprecated

- The retired packages above keep their last-published `0.1.x` npm versions
  (still installable) and are marked **deprecated** on the registry, pointing
  consumers to `@eddacraft/kindling` or the `kindling` binary.

### Security

- **Dev-toolchain CVE sweep.** Cleared all 16 Dependabot alerts (1 critical, 7
  high, 6 moderate, 2 low), every one a development-only dependency — none ship
  in any published package (the thin client has no runtime deps). Bumped
  `vitest` to `^3.2.6` (clears the critical Vitest-UI RCE and pulls patched
  vite/rollup/postcss/picomatch), dropped a vestigial direct `esbuild` devDep,
  and pinned the remaining transitives to patched versions via `pnpm.overrides`
  (esbuild, vite, rollup, postcss, flatted, js-yaml, minimatch, picomatch, yaml,
  brace-expansion). One unrelated `ajv` advisory (via `eslint`, `$data` ReDoS) is
  left as-is: eslint pins `ajv@^6` with no in-range fix, the option isn't used,
  and forcing a major would break linting.

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
