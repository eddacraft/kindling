# Rust Port — Delivered Work

| ID   | Owner  | Status   |
| ---- | ------ | -------- |
| PORT | @aneki | Complete |

**Reconciled:** 2026-11-13.

## Purpose

Completed work is indexed in [completed.aps.md](../../completed.aps.md).
Release receipts and the full inventory are in the
[reconciliation report](../../reconciliation/2026-11-13-aps-cleanup.md).
Original outcomes, validation commands, implementation notes and earlier status
receipts below are historical. Removed TypeScript packages and superseded crate
layouts are not current setup instructions. Completion records delivery at the
stated boundary; it does not close later defects or claim fresh native tests.

PORT-010/014/015/020 remain in the [active module](../../modules/05-rust-port.aps.md).
PORT-018 below was superseded before registry delivery and is excluded from completed counts.

## Work Items

### PORT-001: Rust workspace scaffold

- **Intent:** Cargo workspace initialised with the 10 crates and CI baseline
- **Expected Outcome:** `crates/` directory at repo root with `Cargo.toml` workspace manifest; all 10 crate skeletons compile (`cargo build --workspace`); `.github/workflows/rust.yml` runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test --workspace` on every push
- **Validation:** `cargo build --workspace --release` succeeds locally and in CI
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Done — landed on `feat/rust-port-scaffold`. Local verification: build + fmt + clippy + test all green. CI workflow runs on push/PR to main.

### PORT-002: kindling-types crate

- **Intent:** Canonical Rust definitions for `Observation`, `Capsule`, `Retrieval`, `ScopeIds`, `Id`, `Timestamp`, `Result<T>` with `ts-rs` derives
- **Expected Outcome:** Types in `kindling-types` match shapes in `packages/kindling-core/src/types/*.ts`; `#[derive(TS)]` produces `.d.ts` output that round-trips JSON with the existing TS shapes; export task wired up
- **Validation:** `cargo test -p kindling-types` passes including round-trip tests against sample JSON fixtures; `cargo test -p kindling-types --features ts-rs` writes `.d.ts` files cleanly
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Done — `feat/rust-port-types`. 16 round-trip tests + 22 ts-rs derive tests + 1 export test pass; bindings written to `crates/kindling-types/bindings/` and committed; CI gate `ts-bindings` job fails on drift. `Result<T, E>` excluded as it's a TS control-flow shape — Rust uses native `std::result::Result`. `Timestamp` is the only integer wider than `i32` in public types (an `i64` alias for epoch ms); every field that uses it carries a `#[ts(type = "number")]` override so the TS projection emits `number`, not `bigint`, matching the existing wire format.
- **Dependencies:** PORT-001

### PORT-003: kindling-store crate

- **Intent:** SQLite persistence layer in Rust against `schema/schema.sql`
- **Expected Outcome:** `rusqlite` with `bundled` feature; reads `PRAGMA user_version` and asserts compatibility with `schema/version.json`; supports open/close capsules, append observation, attach observation to capsule, insert pin/unpin, and all FTS-indexed writes; WAL mode enabled; per-project database isolation (`~/.kindling/projects/<hash>/`)
- **Validation:** `cargo test -p kindling-store` passes integration tests against a temp database; a database created by the existing TypeScript store is readable by the Rust store (golden file test)
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Done — `feat/rust-port-store`. 17 integration + 7 golden-file + 6 unit tests pass. `schema/schema.sql` + `version.json` embedded via `include_str!` (drift impossible; `cargo publish` will need a copy step, noted for PORT-014). Fresh DBs get the canonical schema with seeded `schema_migrations`; existing DBs are version-gated (`SchemaTooOld` below `minCompatible` or pre-005 `user_version = 0`, `SchemaTooNew` above `version`). Golden fixture at `crates/kindling-store/tests/fixtures/ts-golden.db` (regenerate with `generate-golden-db.mjs`); covers TS→Rust reads and Rust writes firing TS-defined FTS triggers. Per-project hash verified against Node `crypto` output. `summaries.capsule_id` is UNIQUE — one summary per capsule is schema-enforced, the TS store's `ORDER BY created_at DESC` is defensive only.
- **Dependencies:** PORT-002, schema contract (module 04, Done)

### PORT-004: kindling-filter crate

- **Intent:** Content filtering (secret masking, truncation, excluded-path filtering) matching Node.js behaviour byte-for-byte
- **Expected Outcome:** API keys, tokens, and passwords redacted with the same patterns as the Node.js filter; content truncated at the same limits; excluded paths filtered using the same rules
- **Validation:** `cargo test -p kindling-filter` passes filter tests with known secret patterns; snapshot tests compare filter output against Node.js fixtures
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Done — `feat/rust-port-filter`. Canonical behaviour reference is the production hook filter (`plugins/kindling-claude-code/hooks/lib/filter.js`): 5 secret patterns, 10k char limit (2k for Grep/Glob), `[Truncated N chars]` notice; excluded-path rules from the adapter filter (the hook filter has none). Truncation counts UTF-16 code units to match JS `String.length`/`substring` (rounds down at surrogate-pair boundaries). 47 snapshot fixtures (22 content, 11 tool, 14 path) generated by running the real Node implementations (`tests/fixtures/generate-fixtures.mjs`), all byte-for-byte. Known caveat: non-string tool results serialize via `serde_json` (sorted keys) vs JS insertion order — real parity owed by PORT-009 where raw JSON text is available.
- **Dependencies:** PORT-001

### PORT-005: kindling-provider crate

- **Intent:** Local FTS retrieval provider with BM25 normalisation and tiered retrieval
- **Expected Outcome:** FTS5 search with BM25 scoring normalised to [0, 1]; tiered retrieval (pins → current summary → ranked candidates); deterministic ordering; `RetrieveResult` shape matches the Rust types crate
- **Validation:** `cargo test -p kindling-provider` passes; identical queries against the same database produce the same ranked results as the existing TS provider (cross-implementation parity test, retired after Phase 4)
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Done — `feat/rust-port-provider`, PR #60 (merged 2026-06-11). Implementation notes: BM25 normalisation runs in Rust across both entity types (matching the TS provider); `RetrievalProvider` trait takes `now` explicitly (the only API deviation from TS, for deterministic recency/pin-expiry); orchestrator ports `kindling-core`'s `retrieve` with token budgeting intentionally absent (deprecated). Parity locked by committed golden fixtures (`tests/fixtures/ts-parity.{db,json}`, regen script needs Node v25.6.0 via fnm) — 14 cases JSON-identical incl. exact scores. Match-context truncation is UTF-16 code-unit based, rounds down at surrogate pairs (kindling-filter policy). Parity tests retire after Phase 4.
- **Dependencies:** PORT-003

### PORT-006: kindling-service crate (in-process API)

- **Intent:** Full orchestration layer — `open_capsule`, `close_capsule`, `append_observation`, `retrieve`, `pin`, `unpin` — available as a Rust library for direct in-process use (anvil headless workflows, daemon, CLI)
- **Expected Outcome:** `KindlingService::new(config)` returns a service handle; all six methods behave identically to `@eddacraft/kindling-core`'s `KindlingService`; errors propagate via the Result type pattern; server-side filtering applied at the service boundary
- **Validation:** `cargo test -p kindling-service` passes; contract tests comparing service outputs against the TS service for identical inputs (retired after Phase 4)
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Merged — `feat/rust-port-service`, PR #63 (rebase commit `79e76ac`). 24 integration tests; 6 core methods + read accessors; `ServiceError` Result surface; service-boundary secret masking (mask_secrets on append, both validate paths; truncation deferred to PORT-009); `*_at(now)` clock seams; export/import deferred to PORT-012.
- **Dependencies:** PORT-003, PORT-004, PORT-005

### PORT-007: kindling-server crate (UDS daemon)

- **Intent:** Long-running local daemon serving the v1 HTTP API over a Unix domain socket
- **Expected Outcome:** `kindling serve` listens on `~/.kindling/kindling.sock` (mode `0600`) with TCP fallback on Windows; axum routes for `/v1/health`, `/v1/capsules`, `/v1/observations`, `/v1/retrieve`, `/v1/pins`; per-project DB routing via `X-Kindling-Project` header or body field; PID file with stale-PID cleanup; idle shutdown after configurable timeout (default 30 min)
- **Validation:** Integration tests hit the daemon over UDS with `hyper`; concurrent writes from two clients land cleanly under WAL mode; stale-PID cleanup test verifies a previous crashed daemon doesn't block a fresh spawn
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Merged — `feat/rust-port-server`, PR #64 (rebase commit `f0c6713`). 13 hyper-over-UDS tests; axum v1 routes; per-project routing via `X-Kindling-Project` (project-root string hashed by `project_id` → dir name, no path traversal); PID stale-cleanup; idle shutdown (default 30 min); UDS 0600 + parent dir 0700. Header-only routing (not body-field). Follow-ups: 422 vs 400 for malformed bodies; PID acquire not atomic; CLI wiring is PORT-013.
- **Dependencies:** PORT-006

### PORT-008: kindling-client crate (Rust HTTP-over-UDS client)

- **Intent:** Rust client library for talking to the daemon — used by the hook subcommand, optionally by the CLI, and by anvil for concurrent-safe integration
- **Expected Outcome:** `Client::new()` auto-detects the default socket path; on `ECONNREFUSED` or missing socket, `exec`s `kindling serve --daemonize` and polls for up to 1s; methods mirror `kindling-service` API (`open_capsule`, `append_observation`, `retrieve`, `pin`, `unpin`, plus `health`); `health` checks `schemaVersion` against the client's expected version and fails loud on mismatch
- **Validation:** Integration tests cover cold-spawn, warm-call, schema-mismatch, and connection-refused-without-binary paths; cold-spawn latency measured under 100ms on a typical dev machine
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Merged — `feat/rust-port-client`, PR #66 (rebase commit `fbf56d8`). Thin async hyper-over-UDS client; methods mirror service; auto-spawn (poll within budget); compile-time schema check from `schema/version.json`; 12 tests incl. ~7ms cold-spawn. Prod deps = kindling-types + http stack only.
- **Dependencies:** PORT-007

### PORT-009: kindling-hook crate

- **Intent:** All 7 Claude Code hook types (session-start, post-tool-use, post-tool-use-failure, user-prompt-submit, subagent-stop, pre-compact, stop) handled via stdin JSON, dispatched through `kindling-client`
- **Expected Outcome:** `kindling hook <type>` reads Claude Code hook context from stdin, calls the daemon via `kindling-client`, returns JSON response on stdout matching the Node.js script contract exactly; SessionStart and PreCompact return injected context (pins + recent observations / pins + latest summary)
- **Validation:** Integration tests pipe JSON fixtures through the binary and verify daemon DB state; injected context JSON byte-for-byte matches Node.js output on identical fixtures; warm-hook latency <10ms measured over the 7 hook types; cold-hook (with daemon spawn) <100ms
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Merged — `feat/rust-port-hook`, PR #69 (rebase commits `0a94d70`+`562da49`). Daemon-owns-injection: added `/v1/context/{session-start,pre-compact}` (server formats byte-for-byte Node markdown + toLocaleString date), service assembly methods, client methods, `GET /v1/capsules/open`. Hook crate: all 7 types, capture byte-for-byte from 24 real-Node fixtures, always exits 0, warm ~2ms/cold ~14ms. Enabled serde_json `preserve_order` (types/store/hook) — closed the key-order parity debt. Known gaps: captures not attached to session capsule; two-filter design (adapter content filter + service secret mask).
- **Dependencies:** PORT-008

### PORT-012: kindling-cli crate

- **Intent:** All 12 CLI commands via `clap` (init, log, capsule open/close, status, search, list, pin, unpin, export, import, serve)
- **Expected Outcome:** Commands work with both JSON and text output modes; flags and output shapes match the Commander.js CLI; commands default to in-process service (`kindling-service`) but `--via-daemon` switches to `kindling-client` for safe concurrent operation
- **Validation:** Integration tests for each command; snapshot tests comparing JSON output against the existing TS CLI for identical inputs (retired after Phase 4)
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Merged — `feat/rust-port-cli`, PR #71 (rebase `b8be23f`). 12 clap commands; dual-mode (in-process default / `--via-daemon` → client); export/import bundle impl (deferred from PORT-006) in kindling-service + store, byte-compatible with TS bundle; `serve`→UDS daemon; `init` Claude-Code step stubbed → PORT-015. Snapshot goldens hand-derived (TS CLI unrunnable on Node v26).
- **Dependencies:** PORT-006, PORT-008

### PORT-013: Umbrella `kindling` binary

- **Intent:** Single binary entry point that dispatches to hook, CLI, or server based on subcommand
- **Expected Outcome:** One artefact: `kindling serve`, `kindling hook <type>`, `kindling <cli-command>`; symlink-aware so `kindling-hook` continues to work as a drop-in name for hook scripts if needed
- **Validation:** Single binary size under 20 MB stripped; all three surfaces tested via the umbrella binary
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Merged — `feat/rust-port-umbrella`, PR #73 (`fd2f438`+`8e46094`). Umbrella dispatches all 3 surfaces (hook + symlink / serve / cli verbs); `run_hook` lib refactor; `kindling --version`→`kindling 0.0.1` exit 0; 6.9 MB stripped; 7 umbrella tests. Windows fix: cfg-gated kindling-client UDS transport (UnixStream unavailable on windows) with a loud stub.
- **Dependencies:** PORT-009, PORT-012

### PORT-016: ts-rs export pipeline

- **Intent:** Generated TypeScript type definitions available for the thin TS client to consume
- **Expected Outcome:** `cargo test -p kindling-types --features ts-rs` writes `.d.ts` files to a stable location (`crates/kindling-types/bindings/`); a copy step bundles them into `packages/kindling/src/generated/`; CI fails if generated output drifts from the committed files
- **Validation:** CI job re-runs `ts-rs` and `git diff --exit-code` on generated files — passes if no drift
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Merged — `feat/rust-port-thin-client` (bundled with PORT-017), PR #77 (`6462fde`+`a413dad`). Core ts-rs generation + `crates/kindling-types/bindings/` + drift gate already done in PORT-002; this added the copy step into `packages/kindling/src/generated/` + a CI drift gate for the copy.
- **Dependencies:** PORT-002

### PORT-017: `@eddacraft/kindling` thin client SDK

- **Intent:** Repurpose the primary npm package as the thin HTTP-over-UDS client + auto-spawn + generated types
- **Expected Outcome:** `packages/kindling/` rewritten — removes better-sqlite3, FTS, server, CLI; adds `kindling` client class with the same method shape as the old service (`openCapsule`, `closeCapsule`, `appendObservation`, `retrieve`, `pin`, `unpin`); auto-spawns daemon on first call; uses generated types from PORT-016; ships with no native dependencies
- **Validation:** `pnpm test` in `packages/kindling/` passes against a running daemon; integration test spawns daemon from a clean state on first call; published package contains no native modules
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Merged — `feat/rust-port-thin-client`, PR #77 (`6462fde`+`a413dad`). `packages/kindling` rewritten as a thin HTTP-over-UDS client (Node `http`, no native deps, `0.2.0`); `kindling` class mirrors the old service; auto-spawns daemon on first call (real `serve --socket --kindling-home` flags); uses generated types from PORT-016. 5 vitest tests vs a locally-built daemon; daemon-integration tests `describe.skipIf(!existsSync(target/debug/kindling))` with a dedicated `thin-client` rust.yml job that builds the binary then runs them. The npm postinstall binary DOWNLOAD is PORT-018 (needs a real release) — here the binary is resolved via PATH / `KINDLING_BIN`.
- **Dependencies:** PORT-013, PORT-016

### PORT-019: Adapter packages cutover

- **Intent:** Existing TS adapter packages (`-claude-code`, `-opencode`, `-pocketflow`) consume the new thin client
- **Expected Outcome:** Each adapter's dependency on `@eddacraft/kindling-core` (and any direct store/provider deps) replaced with the new `@eddacraft/kindling` thin client; adapter code calls daemon-backed APIs; adapters published as compatible minor versions
- **Validation:** Each adapter's existing test suite passes against a running daemon; example apps in each adapter README run end-to-end
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.
- **Historical status:** Merged — `feat/rust-port-adapter-cutover`, PR #79 (`744ad8f`). Cut the LIVE integrations (opencode, pocketflow) over from `@eddacraft/kindling-core` (in-process store/service) to the `@eddacraft/kindling` thin client (daemon-backed); both `0.2.0`, no native deps; opencode 77 + pocketflow 163 tests pass; rust.yml thin-client job extended to exercise both. Known reductions: opencode `/memory` `status`→daemon health, `forget`+`export` stubbed (daemon lacked redact/export/stats endpoints at cutover; `forget` later added in `4512ec8`). The claude-code adapter is DEFERRED to PORT-020 deprecation — superseded by the Rust hook (PORT-009) + plugin cutover (PORT-015), so rewiring it is wasted effort.
- **Dependencies:** PORT-017

### PORT-011: anvil integration proof

- **Intent:** Demonstrate direct Rust-to-Rust integration from anvil with no TS bridge involvement
- **Expected Outcome:** One anvil crate (pick the observation emitter that would otherwise call the TS bridge) depends on `kindling-client` (daemon path) and emits an observation directly to the kindling daemon; output matches what the TS bridge produces for the same input
- **Validation:** Parity test in anvil comparing TS-bridge-emitted observation vs. Rust-direct-emitted observation for the same input; both land identically in the kindling database
- **Status:** Complete
- **Delivery evidence (2026-11-13):** anvil PRs #2897 (9ab2cfd0) and #2906 (a20b2c88) are ancestors of published anvil v0.9.0-beta (2026-07-12, commit 6b0ed1d1). KDS-001/002/003 release receipts and parity test are present in the sibling repository.
- **Historical status:** Merged — anvil **eddacraft/anvil-001** PR #2897 (KDS-001 +
  KDS-003, 2026-06-24) + PR #2906 (KDS-002 sink selection). `KindlingDaemonSink`
  in `anvil-cli` over `kindling-client@0.2` + `spool`; NDJSON-vs-daemon parity
  and spool replay tests green. Opt-in via `ANVIL_KINDLING_SINK=daemon` (default
  `ndjson` unchanged). Tracked in kindling [#124](https://github.com/eddacraft/kindling/issues/124).
  Handoff: [`plans/execution/PORT-011-anvil-handoff.md`](../../execution/PORT-011-anvil-handoff.md).
- **Dependencies:** PORT-008 (Done), KINTEG-001 (Done)

> **Note (`kindling-client` spool):** The reusable durable-emit layer is `kindling-client::spool::SpooledClient` (opt-in `features = ["spool"]` — no standalone `kindling-spool` crate). It keeps the daemon (SQLite) authoritative and treats a local append-only NDJSON spool as a transient write buffer: `append_observation` tries the socket and, on a connectivity failure only (`Unavailable`/`Http`), buffers to the spool; daemon-rejection errors (`Api`/`SchemaMismatch`/…) propagate and are never spooled. The spool drains on `flush()` and opportunistically on the next successful append. PORT-011's anvil sink (and any other producer that wants outage-survivable emits, e.g. the `usage.ndjson` pattern) builds on this instead of reinventing a fallback. v1 is at-least-once on a stable observation id; exactly-once is a noted follow-up (daemon-side dedup-on-id).

## Superseded work

### PORT-018: npm self-contained binary delivery (Superseded by D-010)

- **Intent:** `pnpm add @eddacraft/kindling` puts the platform binary in place automatically
- **Expected Outcome:** Per-platform `@eddacraft/kindling-<os>-<arch>[-musl]` packages (one prebuilt `kindling` binary each, with `os`/`cpu`/`libc` fields) declared as `optionalDependencies`; the package manager installs ONLY the matching one — no postinstall, works under `--ignore-scripts`. The client resolves the binary via `require.resolve`, falling back to `$KINDLING_BIN` → a bundled `bin/kindling` → `kindling` on `PATH`. Supersedes the original postinstall-download framing (see D-007).
- **Validation:** Fresh `pnpm add` in a clean project on Linux/macOS/Windows installs the matching binary and exposes a working client; `--ignore-scripts` install still yields a working binary; `pnpm install --frozen-lockfile` stays green.
- **Status:** Superseded by D-010 (2026-10-21): kindling stopped publishing to npm before any platform package reached the registry; `publish.yml`, `build-platform-packages.mjs`, its `--check` CI step and the client's platform-package lookup are removed (`chore/drop-npm-publish`). Prior state: Done (in-repo) — `feat/npm-platform-binaries`, PR #104 (merged). esbuild/swc-style optionalDependencies model instead of a postinstall download; a reusable `_cross-build.yml` is shared by `release.yml` (Release assets) and `publish.yml` (npm platform packages from the same archives). optionalDependencies are **injected at publish time, not committed**, so the lockfile stays installable before the platform packages exist on the registry. `build-platform-packages.mjs --check` passes on main. Windows is x64-only (matches the release matrix). End-to-end registry publish validation is user-gated (real release).
- **Dependencies:** PORT-014, PORT-017
