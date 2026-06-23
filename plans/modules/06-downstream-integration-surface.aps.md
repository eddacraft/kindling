# Downstream Integration Surface (kindling)

| ID     | Owner  | Status      |
| ------ | ------ | ----------- |
| KINTEG | @aneki | In Progress |

**Last reviewed:** 2026-06-23

## Purpose

Harden the contract kindling exposes to downstream consumers — chiefly **anvil**,
whose KDS (kindling daemon source) module is blocked waiting on a published
client and a stable daemon contract. This module turns anvil's integration
wishlist (received 2026-06-22) into a vetted, deduplicated work plan, grounded
against what kindling already ships.

kindling stays **mechanism, not policy**: this module exposes capabilities
(query, handshake, observability, redaction evidence) without encoding anvil's
governance semantics. anvil's `command.invoked` / `gate.evaluated` concerns live
upstream of kindling's generic observation contract.

**Scope confirmed by anvil (2026-06-22).** After the initial triage, anvil
returned a refined pass-on list that maps 1:1 onto KINTEG-001…007. Two
adjustments landed: the "publish kindling-spool" framing was corrected to
"publish `kindling-client` 0.2.0 with stable `SpooledClient`", and import/export
was de-scoped to docs-only-if-missing (see KINTEG-007). No new asks surfaced.

## Context: what already exists (do not rebuild)

Verified against the tree on 2026-06-22:

- **Client + spool are real and mature.** `kindling-client::spool::SpooledClient`
  is a durable-emit wrapper with NDJSON fallback, opportunistic drain, atomic
  temp+rename rewrite, torn-trailing-line recovery, and stable v4 ids assigned
  _before_ spool so replay is idempotent on id. There is **no separate
  `kindling-spool` crate** — anvil's "publish kindling-spool" ask is really
  "publish the client, whose spool module ships with it."
- **`eddacraft-kindling` and `kindling-client` are published at 0.1.0** on
  crates.io. The workspace is locally bumped to **0.2.0** (unpublished). The
  spool module post-dates 0.1.0, so anvil needs 0.2.0.
- **Schema/version handshake is half-built.** `GET /v1/health` already returns
  `{ version, schemaVersion, projects }` (`kindling-server/src/handlers.rs:55`)
  and `Client::health()` checks the daemon's `schemaVersion` against a
  compile-time expected version, failing loud on drift.
- **Import/export is mostly done.** `kindling export` / `kindling import` exist
  with a `bundleVersion`/`version` "1.0" literal and a working `--dry-run`
  validation path (`kindling/src/commands/export.rs`).
- **Redaction is enforced but silent.** `kindling-service/src/filter/secrets.rs`
  masks at the service boundary; the only signal returned is the `redacted`
  bool on an observation — no counts or classes.
- **Dedup is a known, documented gap.** `spool.rs` states exactly-once "requires
  the daemon to ignore (dedup) a write whose id already exists — a noted
  follow-up, not yet implemented."

## In Scope

- Publish kindling 0.2.0 (client + spool) to crates.io
- Daemon-side dedup on observation id (exactly-once-ish replay)
- A structured (non-FTS) read/query API over the daemon
- Capability handshake: version, schema version, supported observation kinds,
  storage path — over both `/v1/health` and `kindling status --json`
- Machine-readable observation-kind registry (kinds + required fields)
- Durable-emit observability (spool status) + cold-start diagnostics
- Redaction evidence (counts + classes, never values) in append responses
- Import/export compatibility guarantee + public adapter test fixtures

## Out of Scope

- anvil's policy semantics (gate decisions, usage views, KDS internals)
- A cross-process spool lock (single-producer-per-spool stays the v1 invariant)
- Streaming/subscription APIs (poll-based reads only for v1)
- Multi-tenant or remote-daemon concerns (kindling stays local-first)

## Interfaces

**Depends on:**

- 05-rust-port (daemon, client, store, service crates) — the surface these items
  extend. Items that touch only existing crates (002, 005, 006) do not block on
  port completion; 001 (publish) gates anvil consumption of all of them.

**Exposes (to anvil and other downstreams):**

- crates.io `kindling-client` 0.2.0 (with `spool`)
- Daemon read/query endpoint with kind/scope/session/repo/time-range filters
- `GET /v1/health` + `kindling status --json` capability block
- Machine-readable kind registry (endpoint + emitted bindings)
- Spool status API + `~/.kindling/` cold-start failure log
- Redaction-evidence fields on append responses
- Published hook-payload fixtures for adapter authors

## Ready Checklist

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one task defined
- [x] Existing capabilities audited so no item rebuilds shipped work

## Work Items

### KINTEG-001: Publish kindling 0.2.0 (client + spool) to crates.io

- **Intent:** Unblock anvil's KDS module, which needs `kindling-client` >= the
  version that carries `SpooledClient`.
- **Expected Outcome:** All workspace crates published at 0.2.0 via
  `scripts/publish.sh`; `kindling-client` 0.2.0 resolvable on crates.io with the
  `spool` module present. README/CHANGELOG note that the spool ships inside the
  client (no standalone `kindling-spool` crate).
- **Validation:** `cargo publish --dry-run -p kindling-client` clean; post-publish
  `cargo add kindling-client@0.2.0` resolves in a scratch crate.
- **Status:** Release prepped (PR) — **publish is user-gated** (crates.io is a
  credential-gated maintainer action, per the runbook).
- **Notes:** Correct anvil's "publish kindling-spool" framing in the release
  note: the durable-emit layer is `kindling-client::spool`, **opt-in behind the
  `spool` feature** (`features = ["spool"]`) — not on by default. Verified
  against the crates.io sparse index that the published `0.1.0` `kindling-client`
  has `features: {}` (no spool at all), so 0.2.0 is the _first_ crate carrying
  `SpooledClient` — this is exactly anvil's blocker. Prep done on this branch:
  workspace already at 0.2.0 (all 7 crates lockstep, intra-deps pinned `0.2.0`);
  CHANGELOG `[0.2.0]` section leads with the spool feature; `docs.rs` metadata
  added to `kindling-client` (`all-features = true`) so `SpooledClient` appears
  in published docs; fixed a runbook bug that listed the publish order as
  client-before-server (client's versioned dev-dep on server requires the
  reverse). Remaining user step: `scripts/publish.sh` after `cargo login`.

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
- **Status:** Proposed
- **Notes:** Stable ids already exist (assigned in `SpooledClient` before spool).
  Decide the contract: silent idempotent success vs. a `deduplicated: true`
  marker on the response (the latter pairs well with KINTEG-006 observability).
  Mind the redaction interaction — a dedup'd write must not re-run masking in a
  way that changes the stored row.

### KINTEG-003: Structured read/query API over the daemon

- **Intent:** Let anvil move usage views off `usage.ndjson` onto a stable daemon
  read API, instead of FTS search or the in-process `list` command.
- **Expected Outcome:** A daemon endpoint (e.g. `GET /v1/observations`) that
  filters by kind, scope (repo/session/agent/user), and time range, returns
  provenance, and paginates deterministically. Exposed via `kindling-client`.
- **Validation:** Endpoint tests covering each filter dimension + pagination
  determinism; client method round-trips the filters.
- **Dependencies:** KINTEG-004 (shares the kind vocabulary)
- **Status:** Proposed
- **Notes:** This is FTS-independent retrieval (no BM25 query string). Reuse the
  existing `list` CLI semantics where possible but lift them to a daemon route
  with kind + time-range filters, which `list` lacks today. `taskId` is carried
  for provenance but is documented as not retrieval-filterable — keep that
  invariant unless anvil makes a concrete case.

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
- **Status:** Proposed
- **Notes:** Half-built today — `/v1/health` already returns version + schema +
  projects. Additive work: `supportedKinds`, `storagePath`, and folding the
  daemon/schema/version block into `kindling status --json` (currently
  in-process DB stats only). Folds anvil wishlist #4 + #8 into one contract
  surface so they can't disagree.

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
- **Status:** Proposed
- **Notes:** `SpooledClient` already tracks enough to derive most fields;
  last-flush-time / last-error / replay-attempts need to be recorded on the
  struct. Cold-start logging closes the spec's "log cold-start failures to
  ~/.kindling/" open follow-up (see index Risks: orphaned/stale daemon).

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
- **Status:** Proposed
- **Notes:** Masking already happens in `filter/secrets.rs`; today only a
  `redacted` bool survives. Add class tagging to the masking pass and thread an
  evidence struct out. Keep it non-bypassable — evidence is derived from the same
  pass that masks, not a second optional scan.

### KINTEG-007: Publish adapter fixtures (+ export compatibility doc if missing)

- **Intent:** Give adapter authors public hook-payload fixtures to test against
  so they stop copying doc examples. Import/export itself is **confirmed
  mostly-done by anvil** (2026-06-22) — no functional work requested there.
- **Expected Outcome:** (a) A published set of hook-payload fixtures (Claude Code
  / OpenCode / PocketFlow) that downstream adapters can test against. (b) Only if
  missing: a written export-bundle compatibility guarantee. (Today `data-model.md`
  notes "export bundles include version for forward compatibility" but states no
  explicit stability promise — so this is a small docs task, not a no-op.)
- **Validation:** A consumer test that loads the published fixtures and
  round-trips them; doc lint that the compatibility note is reachable.
- **Dependencies:** KINTEG-004 (kind registry anchors fixture validity)
- **Status:** Proposed
- **Notes:** Scope narrowed after anvil's 2026-06-22 confirmation: they dropped
  the `--dry-run` error-path-with-paths idea I had speculatively added (it was not
  asked for) and reduced import/export to "request compatibility docs only if
  missing." Fixtures are the primary deliverable: promote/derive from the internal
  `crates/kindling/tests/fixtures/capture-cases.json` into a published, versioned
  fixture set (npm `@eddacraft/kindling` and/or a `fixtures/` dir).
