# Downstream Integration Surface — Delivered Work

| ID     | Owner  | Status   |
| ------ | ------ | -------- |
| KINTEG | @aneki | Complete |

**Reconciled:** 2026-11-13.

## Purpose

Completed work is indexed in [completed.aps.md](../../completed.aps.md).
Release receipts and the full inventory are in the
[reconciliation report](../../reconciliation/2026-11-13-aps-cleanup.md).
Original outcomes, validation commands, implementation notes and earlier status
receipts below are historical. Removed TypeScript packages and superseded crate
layouts are not current setup instructions. Completion records delivery at the
stated boundary; it does not close later defects or claim fresh native tests.

KINTEG-007 and KINTEG-015 remain in the [active module](../../modules/06-downstream-integration-surface.aps.md).
KINTEG-003 ships deterministic keyset pagination, but its old completeness-under-concurrent-inserts
claim remains disputed in [GH-186](https://github.com/eddacraft/kindling/issues/186).
The runtime is published: crates.io carries kindling-runtime 0.3.0, 0.4.0 and 0.4.1.
Historical unpublished-runtime and next-publish notes below have been superseded.

## Work Items

### KINTEG-001: Publish kindling 0.2.0 (client + spool) to crates.io

- **Intent:** Unblock anvil's KDS module, which needs `kindling-client` >= the
  version that carries `SpooledClient`.
- **Expected Outcome:** All workspace crates published at 0.2.0 via
  `scripts/publish.sh`; `kindling-client` 0.2.0 resolvable on crates.io with the
  `spool` module present. README/CHANGELOG note that the spool ships inside the
  client (no standalone `kindling-spool` crate).
- **Validation:** `cargo publish --dry-run -p kindling-client` clean; post-publish
  `cargo add kindling-client@0.2.0 --features spool` resolves in a scratch crate.
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.
- **Historical status:** Done — published 2026-06-24 via `scripts/publish.sh` after
  `cargo login`. All seven workspace crates at 0.2.0 on crates.io; registry
  verification: `cargo add kindling-client@=0.2.0 --features spool` resolves from
  `registry+https://github.com/rust-lang/crates.io-index`; `docs.rs/kindling-client/0.2.0`
  documents `SpooledClient`.
- **Notes:** The durable-emit layer is `kindling-client::spool`, **opt-in behind the
  `spool` feature** (`features = ["spool"]`) — not on by default. 0.2.0 is the
  first crates.io release carrying `SpooledClient` (0.1.0 had `features: {}`).
  Prep landed in PR #118; publish-readiness tests in
  `crates/kindling/tests/publish_readiness.rs`.

### KINTEG-002: Daemon-side observation dedup (exactly-once-ish replay)

- **Intent:** Make spool replay after a crash idempotent so an observation
  committed before the spool was rewritten is not stored twice.
- **Expected Outcome:** The daemon ignores an append whose observation `id`
  already exists (insert-or-ignore / upsert-by-id at the store boundary),
  returning the existing row rather than erroring. Spool replay becomes
  exactly-once-ish; `spool.rs`'s "noted follow-up" is closed.
- **Validation:** Store-level test: appending the same id twice yields one row;
  client-level test: a replay of an already-delivered spool entry is a no-op.
- **Dependencies:** —
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.
- **Historical status:** Merged — PR #121 (`feat/kinteg-002-daemon-dedup`, merged 2026-06-24).
- **Notes:** Stable ids already exist (assigned in `SpooledClient` before spool).
  Contract decided: a **`deduplicated: bool`** marker (not silent) — `AppendOutcome`
  in `kindling-service`, `AppendResult` in `kindling-client`, top-level
  `deduplicated` on the `POST /v1/observations` response. Store `insert_observation`
  → `INSERT OR IGNORE` returning written?; on a duplicate the service returns the
  **existing stored row** (no re-mask, no mutation); `attach_observation_to_capsule`
  also made idempotent. Redaction interaction covered (forget-then-replay returns
  the redacted row). Pairs with KINTEG-006 observability.

### KINTEG-003: Structured read/query API over the daemon

- **Intent:** Let anvil move usage views off `usage.ndjson` onto a stable daemon
  read API, instead of FTS search or the in-process `list` command.
- **Expected Outcome:** A daemon endpoint (e.g. `GET /v1/observations`) that
  filters by kind, scope (repo/session/agent/user), and time range, returns
  provenance, and paginates deterministically. Exposed via `kindling-client`.
- **Validation:** Endpoint tests covering each filter dimension + pagination
  determinism; client method round-trips the filters.
- **Dependencies:** KINTEG-004 (shares the kind vocabulary)
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.
- **Historical status:** Merged — PR #128 (`feat/kinteg-003-list-api`, merged 2026-06-26).
  `POST /v1/observations/list` + `Client::list_observations` shipped; gates green
  (fmt/clippy/workspace tests + ts-rs & vendored-types drift checks). Released in
  0.3.0 (crates.io publish user-gated). Design accepted as D-009 (planning
  council, 2026-06-26).
  Spec: `plans/specs/2026-06-26-kindling-read-api-and-spool-cap-design.md`.
- **Notes:** This is FTS-independent retrieval (no BM25 query string). Reuse the
  existing `list` CLI semantics where possible but lift them to a daemon route
  with kind + time-range filters, which `list` lacks today. `taskId` is carried
  for provenance but is documented as not retrieval-filterable — keep that
  invariant unless anvil makes a concrete case.
- **Design (D-009):** `POST /v1/observations/list` (POST+JSON, matches every data
  route) with `{ scopeIds, kinds?, since?, until?, limit?, cursor?, includeRedacted? }`.
  Keyset cursor (opaque base64 `<ts>:<id>`) over the store's stable `(ts ASC, id ASC)`
  order — the only scheme complete under concurrent appends; **no offset**. Half-open
  `[since, until)` bounds (kills boundary double-count); `kinds` is a list (omitted =
  all); `limit` server-clamped to ≤1000; redacted excluded by default with a v1
  `includeRedacted` flag (so a `forget()` on a `command.invoked` row doesn't inflate
  anvil's `never_invoked`). Response `{ observations[], nextCursor? }` — **no
  totalCount**, absent `nextCursor` = complete. **No server-side aggregation**
  (anvil's view semantics stay in anvil — mechanism, not policy). **No schema bump**
  (read-only, no DDL; stays v5; `idx_obs_repo_ts` serves the repo-scoped scan; a
  dedicated ASC index + migration 006 is deferred until profiling proves a filesort;
  old daemon → 404). New `store::list_observations` (not an overload of
  `query_observations`), service method (no masking on reads), client
  `list_observations`, and `kindling-types` request/result types. Consumer contract
  (documented for anvil): `repo_id` must match append-time strings; the list is a
  daemon-store view only (anvil flushes its spool before listing); daemon-down reads
  return `Unavailable` with no fallback.

### KINTEG-009: Spool retention cap (size + age)

- **Intent:** Bound `SpooledClient`'s spool so retiring anvil's `usage.ndjson`
  sidecar onto the spool (anvil KDS-005) is not a retention regression. The sidecar
  trims to a rolling 7-day / 64 MiB window; the spool today has no cap and grows
  unbounded under a prolonged daemon outage.
- **Expected Outcome:** `SpoolConfig` gains `max_bytes: Option<u64>` and
  `max_age_ms: Option<i64>` (struct becomes `#[non_exhaustive]` + builder), both
  **defaulting to `None`/unbounded** (opt-in; anvil wires 64 MiB / 7d). `SpoolEntry`
  gains `spooled_at` (real age basis). Trim drops the **oldest contiguous prefix
  only** (age first, then bytes), **only inside the `flush()` lock** as part of the
  existing atomic `temp_sibling`+rename rewrite — preserving drain order and never
  dropping an un-drained entry ahead of a kept newer one. A lone entry larger than
  `max_bytes` is kept (high-water target, not a hard ceiling). `dropped_count` added
  to `SpoolStatus` so shed data is observable. Trim is documented as intentional
  bounded loss — distinct from flush's "never silently drop" — i.e. "respect
  at-least-once" means don't-reorder / don't-drop-newer-while-keeping-older, not
  infinite retention. Client bumps **0.2 → 0.3**.
- **Validation:** trim-by-bytes/age preserves order (oldest dropped); trim under
  outage then flush still drains the remainder; lone oversize entry retained; legacy
  entry without `spooled_at` is byte-trimmable but not age-trimmed; empty-spool trim
  is a no-op; `dropped_count` increments; property test — survivors are always a
  contiguous oldest-dropped suffix.
- **Dependencies:** — (touches only `kindling-client`; independent of KINTEG-002/008)
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.
- **Historical status:** Merged — PR #126 (`feat/kinteg-009-spool-retention`, merged 2026-06-26).
  `SpoolConfig` `max_bytes`/`max_age_ms` (default unbounded), `SpoolEntry.spooled_at`,
  oldest-prefix trim inside `flush()` lock, `SpoolStatus::dropped_count`; 13 spool
  tests green. Design: D-009 / `plans/specs/2026-06-26-kindling-read-api-and-spool-cap-design.md`
  (landed PR #125). **Publish:** next `kindling-client` release must be **≥ 0.3.0**
  (`SpoolConfig` field addition; user-gated `scripts/publish.sh`).
- **Notes:** No independent/concurrent trim and no CLI `spool trim` subcommand — an
  out-of-band trim racing a flush (or a second process sharing the path) can drop
  in-flight entries; single-producer-per-path stays the v1 invariant (the in-process
  `Mutex` does not protect two processes). Source: anvil issue #2916.

### KINTEG-004: Capability handshake + machine-readable kind registry

- **Intent:** Give anvil a single call to learn daemon version, schema version,
  supported observation kinds (+ required fields), and storage path, so it can
  fail fast on contract drift instead of guessing shapes.
- **Expected Outcome:** `/v1/health` and `kindling status --json` both surface a
  capability block: `{ version, schemaVersion, supportedKinds, storagePath, ...}`.
  A machine-readable kind registry (kinds + required fields) is emitted — ideally
  reusing the existing `ts-rs` bindings pipeline so the registry can't drift from
  `ObservationKind`.
- **Validation:** Golden-JSON test on the health/status capability block;
  registry test asserting every `ObservationKind` variant is present with its
  required fields.
- **Dependencies:** —
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.
- **Historical status:** Done — PR #117 (`feat/kinteg-004-capability-handshake`, merged
  2026-06-23). Shared `kindling-types::build_capability` feeds `/v1/health`,
  `kindling status --json`, Rust `Client::health()`, and the TS thin client;
  kind registry derived from `ObservationKind::ALL` with ts-rs bindings.
- **Notes:** Unblocks KINTEG-003 and KINTEG-007 (both depend on the kind vocabulary).

### KINTEG-005: Durable-emit observability + cold-start diagnostics

- **Intent:** Make "are observations stuck?" and "why didn't the daemon spawn?"
  answerable from host tools without source diving.
- **Expected Outcome:** (a) A spool status surface — pending count (already have
  `pending_count()`), last flush time, last error, replay attempts, spool path —
  exposed via a client method and a `kindling spool status` command. (b) Auto-spawn
  / cold-start failures logged to `~/.kindling/` (e.g. `spawn.log`) so a failed
  daemon launch is diagnosable from the host.
- **Validation:** Spool-status test asserting the fields after a forced outage +
  flush; spawn-failure test asserting a log line is written on a simulated
  spawn failure.
- **Dependencies:** —
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.
- **Historical status:** Complete (2026-06-24)
- **Notes:** Shipped `SpoolStatus` + `spool_status()` on `SpooledClient`,
  passive `spool_status_from_path()` (reads `{spool}.status.json` sidecar) for CLI
  inspection, `kindling spool status --spool-path --json`, and spawn-failure logging
  to `~/.kindling/spawn.log` (configurable via `ClientConfig::spawn_log_path`).
  Flush/error/replay metadata is best-effort in the sidecar (never blocks spool/flush).

### KINTEG-006: Redaction evidence in append responses

- **Intent:** Let callers prove sensitive data was handled, without leaking the
  values, satisfying anvil's redaction-evidence ask.
- **Expected Outcome:** Append responses (and/or diagnostics) carry redaction
  evidence — a count and the matched classes (e.g. `apiKey`, `bearerToken`),
  never the matched substrings. Surfaced through the service → server → client
  chain.
- **Validation:** Service test asserting the evidence (count + classes) for a
  payload with N secrets; assert no raw secret bytes appear in the response.
- **Dependencies:** —
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.
- **Historical status:** Merged — PR #136 (`feat/service-redaction-evidence`, merged
  2026-07-12). `RedactionEvidence` now carries the masked-match count and distinct
  secret classes through service, server, Rust client, and generated TypeScript
  bindings without exposing matched values.
- **Notes:** Evidence is produced by the same masking pass that redacts content,
  including on deduplicated appends, and defaults safely for rolling upgrades
  against older daemons.

### KINTEG-008: `kindling-runtime` — anvil-first integration facade

- **Intent:** Give anvil (and other Rust downstreams) a **single Cargo dependency**
  that bundles daemon startup, client wiring, and durable emit — without
  requiring the `kindling` CLI on `PATH` or manual `Spawner::custom` glue.
- **Expected Outcome:** New workspace crate `kindling-runtime` published to
  crates.io. Default features (`client`, `spool`, `embedded-daemon`) expose
  `Runtime::start(config) -> Runtime` with:
  - **Attach-or-start** on the configured UDS socket (reuse an existing daemon
    when present; otherwise start `kindling_server::serve` in-process on a tokio
    task via `Spawner::custom`, matching the pattern in
    `crates/kindling-client/tests/client.rs::cold_spawn_starts_daemon`).
  - **`spooled_client()`** as the primary emit surface (spool enabled by default
    at the runtime layer; bare `kindling-client` keeps spool opt-in for compat).
  - **`client()`** for callers that want the thin client without spool.
  - Re-exported `kindling-types` domain types so consumers need not depend on
    types separately.
  - Optional `external-spawn` feature: fall back to `Command::new("kindling")`
    when the host already ships the CLI (current default behaviour).
- **Expected Outcome (docs):** README with anvil-oriented quickstart:
  `kindling-runtime = { version = "0.3", features = ["embedded-daemon"] }` and a
  minimal `Runtime::start` → `spooled_client().append_observation` example.
  Design spec at `plans/specs/2026-06-24-kindling-runtime-design.md`.
- **Validation:**
  - `cargo test -p kindling-runtime` — cold embedded start, attach to
    pre-running daemon (spawner not called), spooled append round-trip into store.
  - `cargo clippy -p kindling-runtime --all-features -- -D warnings` clean.
  - `cargo package --list -p kindling-runtime` includes `Cargo.toml`, `README.md`.
  - Post-publish: `cargo add kindling-runtime` resolves in a scratch crate; anvil
    KDS can drop direct `kindling-client` + `kindling-server` deps.
- **Dependencies:** KINTEG-001 (Done); KINTEG-002 (Merged — dedup closes the
  at-least-once gap for spool-as-default). PORT-011 (Merged in anvil with raw
  `kindling-client`) documents baseline integration pain; anvil can migrate to
  `kindling-runtime` after publish.
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.
- **Historical status:** Merged — PR #129 (`feat/kinteg-008-onto-main`, merged 2026-06-26).
  Cherry-picked onto current `main` (PR #122 had merged into `feat/kinteg-002-daemon-dedup`,
  not `main`). New crate `kindling-runtime` (v0.2.0) with `Runtime::start` /
  `spooled_client()` / attach-or-start; `spawn_log_path` compat fix for KINTEG-005;
  `publish.sh` updated; `publish_readiness` asserts 8 crates. **Publish:** next
  release should include `kindling-runtime` (user-gated `scripts/publish.sh`).
- **Notes:**
  - **Not** a merge of the seven crates — composes `kindling-client` +
    `kindling-server`; CLI/npm adapters unchanged.
  - `scripts/publish.sh` order: insert `kindling-runtime` after `kindling-server`,
    before `kindling-client` (runtime depends on both).
  - v1 is daemon-mode only. A follow-up `embedded-service` feature (direct
    `KindlingService`, zero IPC) is explicitly deferred unless anvil proves a
    shared-socket-free hot path.
  - KINTEG-003/004 capability and query methods should eventually hang off
    `Runtime` (thin delegates), not duplicate wire shapes.

### KINTEG-010: Authenticate the TCP daemon transport (no per-user boundary)

- **Intent:** Close the gap where the TCP fallback transport exposes the same
  router as the UDS path but with no per-user authentication — material on
  platforms (e.g. Windows) where loopback TCP is the default transport.
- **Expected Outcome:** Non-health requests over TCP require an unguessable
  per-daemon secret (or an OS transport with per-user ACLs). The
  `X-Kindling-Project` header is never treated as authorization, and any
  discovery/port file is written owner-only.
- **Validation:** Server test: a TCP data request without the secret is
  rejected, with it accepted; assert an arbitrary `X-Kindling-Project` header
  alone cannot read or modify another project's memory.
- **Dependencies:** —
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.
- **Historical status:** Merged — PR #140 (`fix/tcp-daemon-auth`, merged 2026-07-12).
  The daemon now writes an owner-only per-daemon bearer token beside the TCP port
  file; every TCP request except health requires it, while UDS behaviour is
  unchanged.
- **Notes:** The client reads and sends the token automatically. Tests cover
  missing, incorrect, and correct tokens; prove `X-Kindling-Project` alone grants
  no access; and verify owner-only token permissions on Unix.

### KINTEG-011: Make capsule close atomic when the summary is missing

- **Intent:** A close that fails summary validation must not leave the capsule
  closed. Today the status update can succeed before `SummaryNotFound` is
  returned, so callers get a failure while the capsule is left closed —
  corrupting lifecycle state relative to the reported error and blocking a clean
  retry.
- **Expected Outcome:** `close_capsule` validates the summary exists for the
  capsule before updating status (or wraps update + validation in a transaction
  that rolls back on failure). A failed close leaves the capsule open with
  `closed_at` unchanged.
- **Validation:** Update
  `crates/kindling-store/tests/store_integration.rs::close_capsule_validates_summary`
  to assert the capsule remains open (status and `closed_at` unchanged) after a
  `SummaryNotFound` failure.
- **Dependencies:** —
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.
- **Historical status:** Merged — PR #133 (`fix/atomic-capsule-close`, merged 2026-07-12).
  `close_capsule` now validates the summary before updating capsule status.
- **Notes:** The integration test proves a missing summary returns
  `SummaryNotFound` while leaving the capsule open with `closed_at` unchanged.

### KINTEG-012: Honor `--db` routing hint in daemon-backed commands

- **Intent:** `build_client` is documented to honor `--db` as a socket-routing
  project hint, but the implementation ignores it and always constructs a default
  client. So `--via-daemon` commands with an explicit `--db` are silently routed
  by process cwd / the default client, potentially reading or writing a different
  project DB than the invocation names.
- **Expected Outcome:** The resolved common `--db` value is threaded into
  daemon-backed client construction and translated into the daemon routing /
  project hint the API contract promises — or `--via-daemon` with `--db` is
  rejected for all affected commands with a clear error.
- **Validation:** Daemon test exercising `--via-daemon <cmd> --db <path>` that
  asserts routing to the named project (or a clean rejection), extending the
  current import-only `--via-daemon` + `--db` coverage.
- **Dependencies:** Relates to KINTEG-003 (read/query API also rides daemon
  routing).
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.
- **Historical status:** Merged — PR #134 (`fix/reject-via-daemon-db`, merged 2026-07-12).
  Every daemon-backed command now rejects `--via-daemon` combined with `--db`
  instead of silently routing to the default project.
- **Notes:** Centralised client construction enforces the rejection for log,
  capsule, search, pin, unpin, and forget commands; daemon tests cover each verb
  and require an error that names both conflicting flags.

### KINTEG-013: Keep outage spooling independent of backlog depth

- **Intent:** Prevent sustained daemon outages from making each buffered append
  progressively slower as the spool grows. The current append path parses and
  rewrites the whole backlog and retries a known-unavailable daemon for nearly
  every new observation.
- **Expected Outcome:** A single-producer `SpooledClient` caches its live pending
  count, applies a bounded retry backoff after connectivity failures, and appends
  behind an existing backlog without rescanning it. Ordered replay, stable ids,
  passive on-disk status, retention trimming, and exactly-once-ish application
  remain unchanged.
- **Validation:** A public-interface regression test proves a burst during one
  outage does not attempt backlog replay per appended observation; the client
  spool integration suite, workspace tests, clippy, and the outage benchmark
  profile pass. The benchmark records early and late outage-write windows so
  backlog-dependent latency remains visible.
- **Dependencies:** KINTEG-002, KINTEG-005, KINTEG-009 (all closed).
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.
- **Historical status:** Merged — PR #143 (`feat/performance-benchmarks`, merged 2026-08-04, tip `f6dcd7d`).
- **Notes:** Standard-risk performance hardening under the existing
  single-producer-per-spool invariant. It does not add aggregation policy,
  cross-process locking, or change retention defaults. Design:
  `plans/specs/2026-08-03-kindling-performance-hardening-design.md`.
  Anvil consumer history: eddacraft/anvil-001 `benchmarks/history/kindling/2026-08-04.json`.

### KINTEG-014: Index deterministic scoped reads

- **Intent:** Remove SQLite's temporary sort from the bounded observation-list
  path. Existing scope indexes end at `ts`, while deterministic keyset ordering
  and cursors use `(ts, id)`.
- **Expected Outcome:** Schema migration 006 rebuilds the repo and session scope
  indexes as `(scope_id, ts, id)`, preserving their partial predicates. Fresh
  and upgraded databases expose the same indexes, older read-only databases
  remain readable, and list results/cursors are unchanged.
- **Validation:** Migration coverage upgrades a v5-shaped database and checks
  index columns plus `user_version`; list filtering/pagination suites pass; an
  `EXPLAIN QUERY PLAN` regression check rejects temporary ordering for the
  representative repo-scoped keyset query; workspace schema drift gates pass.
- **Dependencies:** KINTEG-003 (closed).
- **Status:** Complete
- **Delivery evidence (2026-11-13):** Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.
- **Historical status:** Merged — PR #143 (`feat/performance-benchmarks`, merged 2026-08-04, tip `f6dcd7d`).
- **Notes:** This implements the index migration explicitly deferred by D-009
  until profiling proved a filesort. It adds no aggregation endpoint or
  downstream policy. Design:
  `plans/specs/2026-08-03-kindling-scoped-read-index-design.md`.
