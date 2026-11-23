# Retrieval Performance

| ID   | Owner        | Status | Model           | Reasoning |
| ---- | ------------ | ------ | --------------- | --------- |
| PERF | @chrisco-dev | Ready  | claude-opus-5-5 | high      |

**Last reviewed:** 2026-11-23. The investigation behind this module is in
[#316](https://github.com/eddacraft/kindling/issues/316). The full report is a
comment on that issue. The order and scope were agreed with Josh on 2026-11-23,
after the query-syntax fix ([#311](https://github.com/eddacraft/kindling/issues/311),
PR #317).

## Purpose

Make retrieval cost scale with what the caller asked for, not with the size of
the store. Today a broad query over a 220k-row store takes 160–280 ms, and every
other request for that project waits behind it in the daemon.

## Current truth

- After #306, the score is `0.7 · minmax(BM25 over the whole filtered match set)
  + 0.3 · recency`. So every filtered match is read and scored, and cost is
  O(matches).
- The largest single SQLite cost is the join from each FTS match to the wide
  `observations` row. `id`, `ts`, the scope ids and `redacted` sit after
  `content`, behind its overflow pages (experiment 7).
- A prototype of a narrow metadata table and a 30-day window exists on a local
  experiment branch. It isn't shippable: both changes sit behind environment
  toggles, and the migration isn't versioned.
- The daemon holds one `Mutex<KindlingService>` per project
  (`crates/kindling-server/src/state.rs`). Handlers take that lock inside async
  functions, so a slow search blocks every write for that project and one
  runtime worker thread.

## In Scope

- An exact speed-up from a narrow metadata table for the candidate join.
- Concurrent reads and writes in the daemon.
- Where scope filtering happens relative to the FTS search and scoring.
- An approximate 30-day window, gated so that only queries with large match sets
  use it.

## Out of Scope

- An OR fallback for AND queries that find nothing (#316 recommendation 4).
- Reducing indexed boilerplate (#316 recommendation 5).
- A second index such as Tantivy (#316 recommendation 6).
- VS Code search scope (#312) and exact identifier lookup (the #311 follow-up).

## Interfaces

**Depends on:**

- The schema contract (`schema/`, `schema/README.md`): PERF-001 adds a
  versioned, additive migration.
- The ranking contract (`docs/retrieval-contract.md`, #306): PERF-001, PERF-002
  and PERF-003 keep it exactly. PERF-004 relaxes it, only for large match sets.

**Exposes:**

- No new public API or wire fields. Result changes are limited to PERF-004,
  which is documented in the retrieval contract and the CHANGELOG.

## Decisions

- **The exact changes go first.** PERF-001 to PERF-003 must leave every
  ranking-contract test passing unchanged.
- **PERF-004 is a ranking change.** It was agreed with Josh, provided it only
  applies above a match-count threshold. Below the threshold, results stay
  exactly as they are today.
- **One pull request per item.** Each item branches from `main`. Stacks are
  disabled by policy, so dependent items start after their dependency merges.

## Ready Checklist

- [x] Investigation and measurements published (#316).
- [x] Order and scope agreed with the maintainer (2026-11-23).
- [x] Ranking relaxation for PERF-004 agreed, gated on match-set size.

## Work Items

### PERF-001: Narrow metadata table for the candidate join

- **Status:** Ready
- **Intent:** Make broad searches 2–4× faster without changing any result.
- **Expected Outcome:** The provider's observation candidate join reads a narrow
  per-observation metadata table instead of the wide `observations` row. The
  table is created and backfilled by a versioned additive migration and kept in
  sync on insert, update, redaction and delete. Results are identical to
  before for every query and scope. A benchmark on a corpus of 200k rows or more
  shows the large-match-set queries at least 2× faster, and small ones no
  slower.
- **Validation:**
  - `cargo test --workspace`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - The #306 ranking-contract tests pass unchanged.
  - A store test proves the metadata stays consistent through insert, redact,
    forget, and opening a database created at schema version 7.
  - The benchmark results, before and after, are recorded in the PR.
  - `python3 scripts/check-release-docs.py`
- **Files:** `schema/schema.sql`, `schema/version.json`, `schema/README.md`,
  `crates/kindling-store/src/schema.rs`, `crates/kindling-store/src/db.rs`,
  `crates/kindling-provider/src/provider.rs`, `crates/kindling-bench/`,
  `docs/data-model.md`, `CHANGELOG.md`, `ENGINEERING-HISTORY.md`
- **Dependencies:** none
- **Confidence:** high

### PERF-002: Concurrent reads and writes in the daemon

- **Status:** Ready
- **Intent:** Stop a slow search from blocking capture and other requests for
  the same project.
- **Expected Outcome:**
  - **First, measure.** Record hook-write and small-query latency while broad
    searches run against the same project. This is the baseline.
  - **Then the change.** Reads use their own SQLite connections, alongside one
    write connection, relying on WAL. Blocking database work no longer runs on
    async runtime threads.
  - **Results.** Under the same load, p95 write latency falls by at least an
    order of magnitude. Every API response and status code is unchanged. Daemon
    ownership (#309), per-project routing and shutdown keep their current
    behaviour.
- **Validation:**
  - `cargo test --workspace`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - A server test proves a write completes while a long read is in progress.
  - The before and after latency measurements are recorded in the PR.
  - `python3 scripts/check-release-docs.py`
- **Files:** `crates/kindling-server/src/state.rs`,
  `crates/kindling-server/src/handlers.rs`, `crates/kindling-service/src/`,
  `crates/kindling-store/src/db.rs`, `docs/architecture.md`,
  `ENGINEERING-HISTORY.md`
- **Dependencies:** none
- **Confidence:** medium

### PERF-003: Filter by scope before scoring the full match set

- **Status:** Ready
- **Intent:** Make a scoped search pay for matches in its own scope, not for
  matches across the whole store.
- **Expected Outcome:**
  - **Measure first.** On top of PERF-001, record the query plans and timings
    for repo-scoped and session-scoped searches, comparing a scope-first order
    (scope index or rowid range, then the FTS probe) with an FTS-first order.
  - **If a scope-first order is at least 1.5× faster on large match sets,**
    retrieval uses it, and results stay identical.
  - **If it isn't,** the measurements are recorded on #316, and the item closes
    without a code change.
- **Validation:**
  - `cargo test --workspace`
  - The #306 ranking-contract tests pass unchanged.
  - The query plans and benchmark results are recorded in the PR, or on #316
    when no change is made.
- **Files:** `crates/kindling-provider/src/provider.rs`, `crates/kindling-bench/`
- **Dependencies:** PERF-001
- **Confidence:** medium

### PERF-004: Search the last 30 days first, only for large match sets

- **Status:** Ready
- **Intent:** Bound the cost of the most expensive queries while keeping
  today's exact results for everything else.
- **Expected Outcome:**
  - **The gate.** A cheap match-count check runs first.
  - **Below the threshold** (default about 1,000 matches in scope), retrieval
    is exactly as it is today.
  - **Above the threshold,** candidates come from the last 30 days, with a
    fallback to the full search when that window has fewer than
    `max_results` matches.
  - **Rows out of time order.** Rows whose `ts` doesn't rise with rowid, such as
    imports and backfills, can't drop a result that the window should include.
  - **Documentation.** The retrieval contract documents the relaxed guarantee,
    and the CHANGELOG notes the ranking change.
  - **Speed.** The worst-case benchmark query is at least 2× faster than
    PERF-001 alone.
- **Validation:**
  - `cargo test --workspace`
  - The #306 ranking-contract tests pass below the threshold. Tests above the
    threshold pin the windowed behaviour and the fallback.
  - The benchmark results and the top-10 overlap versus exact ranking are
    recorded in the PR.
  - `python3 scripts/check-release-docs.py`
- **Files:** `crates/kindling-provider/src/provider.rs`,
  `docs/retrieval-contract.md`, `CHANGELOG.md`, `ENGINEERING-HISTORY.md`
- **Dependencies:** PERF-001, PERF-003
- **Confidence:** medium
