# Downstream Integration Surface (kindling)

| ID     | Owner  | Status      |
| ------ | ------ | ----------- |
| KINTEG | @aneki | In Progress |

**Last reviewed:** 2026-11-13.

## Purpose

Maintain the mechanism contract consumed by anvil and other integrations.
Thirteen delivered items are in the [archive](../archive/modules/06-downstream-integration-surface.aps.md)
and [completed index](../completed.aps.md). Two items remain active: the full
adapter-fixture scope and the proposed next-minor event contract (KINTEG-015,
[design](../specs/2026-11-18-generic-event-keyed-append-design.md)).

## Current contract

- The client spool, daemon dedup, filtered enumeration, handshake and kind registry,
  observability, redaction evidence, TCP authentication, atomic capsule close,
  daemon/db routing checks and scoped-read indexes ship through v0.4.1.
- `kindling-runtime` is published on crates.io starting at 0.3.0, including
  0.4.0 and 0.4.1. It composes the server and client; it is not a replacement
  for the shared daemon contract. There is no `kindling-spool` crate.
- Keep governance policy in anvil. No remote, multi-tenant, subscription or
  server-side aggregation contract is added here.
- KINTEG-003's claim of completeness under arbitrary concurrent inserts needs
  correction via [GH-186](https://github.com/eddacraft/kindling/issues/186).
  Archiving its delivered API does not close that defect.
- Dependencies on the Rust implementation and KINTEG-001/004 are delivered;
  PORT's separate platform/lifecycle closeout does not block work on those APIs.

## Ready Checklist

- [x] Delivered dependencies reconciled against publication
- [x] Open fixture acceptance and next-minor scope distinguished
- [ ] KINTEG-015 design and release scope approved before implementation

## Work Items

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
- **Delivered dependencies:** KINTEG-004 (Complete; [archive](../archive/modules/06-downstream-integration-surface.aps.md)); its kind registry anchors fixture validity.
- **Status:** In Progress
- **Reconciliation (2026-11-13):** PR #135's Claude Code fixtures and compatibility documentation ship in v0.4.0/v0.4.1. fixtures/hook-payloads contains only claude-code.json; the expected OpenCode/PocketFlow fixture families and their consumer round-trips are not delivered. Keep the full item active rather than silently narrow its outcome.
- **Historical status:** Merged — PR #135 (`feat/publish-adapter-fixtures`, merged
  2026-07-12). Versioned Claude Code hook fixtures now ship from the repository
  and the `@eddacraft/kindling` package, with deterministic sync and consumer
  round-trip tests; `docs/data-model.md` defines the export-bundle compatibility
  contract.
- **Notes:** The fixture set is derived from
  `crates/kindling/tests/fixtures/capture-cases.json`; `check:fixtures` gates drift
  between the internal source, repository fixture, and npm package copy.

### KINTEG-015: Publish `generic-event@1` and a keyed direct-retained append in the next minor

- **Intent:** Downstream Rust consumers can store an opaque, versioned event
  under a key and get a receipt proving it is durably retained, without kindling
  learning their policy.
- **Design:** [Generic events and keyed durable append](../specs/2026-11-18-generic-event-keyed-append-design.md)
  (planning council plan-9ad9b830). The spec is authoritative for the wire
  contract, schema, durability and security behaviour. It amends this item's
  original wording in five places: uniqueness is per project store, not per
  profile; the failure set gains `Indeterminate`; `StoreBusy` folds into
  `FailedCommit{Busy}`; a forget endpoint is added; and the release is a final
  0.5.0 with no release candidate.
- **Expected Outcome:** Final 0.5.0 ships the following through
  `kindling-runtime`, `kindling-client` and new v1 endpoints, behind one
  capability feature, `generic-event@1`:
  - **Envelope:** an opaque payload of at most 256 KiB, sent as base64, with a
    consumer-declared schema name and version. kindling validates only the
    envelope. Events live in a separate `generic_events` table that FTS,
    retrieve, inject and list never see, but that export and import carry. The
    closed `ObservationKind` set and its behaviour are unchanged.
  - **Keyed durable append:** the key is unique per (schema name, key) within a
    project store. Append atomically reserves the key, the canonical scope
    (including the project id) and a SHA-256 digest. It returns a receipt only
    after a `synchronous=FULL` commit. A same-key retry of identical content
    returns the byte-identical original receipt without a second row. A
    different scope or payload is a conflict, and so is any append to a
    forgotten key.
  - **Lookup and forget:** exact lookup by key after a restart returns `found`,
    `redacted` or `notFound`, with the same event id, scope and digest as the
    receipt. Forget tombstones the payload and keeps the key reserved.
  - **Closed failure set:** `Unsupported`, `Unavailable`, `RejectedPayload`
    (invalid envelope, too large, secret detected), `Conflict` (mismatch,
    redacted), `Quota`, `LowDisk`, `FailedCommit` (busy, storage) and
    `Indeterminate` (safe to retry after the single same-key recovery
    re-append). No failure returns a receipt, and none falls back to the spool.
  - **Capability negotiation:** a `features` entry with limits on
    `/v1/health`. Releases through 0.4.x answer "unsupported" and are
    documented as incompatible with this contract.
  - **Compatibility:** schema revision 7 is additive (`user_version` stays 6),
    so 0.4.x binaries keep opening 0.5 databases. Ordinary spooled append,
    replay, observation masking and redaction are unchanged.
- **Delivery:** four sequential PRs, KINTEG-015a to 015d. All four ship
  together in one 0.5.0 release behind the single `generic-event@1` capability.
  The feature entry is not advertised until 015d merges, so the earlier merges
  stay dark.
- **Validation:** Every sub-item's validation passes. Then, at release:
  `cargo test --workspace`,
  `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test -p kindling-runtime --all-features`, the failpoint crash suite,
  the `keyed-durable` release-prep bench gate, a package-content check and a
  clean scratch-crate install of the published 0.5.0. That install commits one
  keyed event, restarts the runtime and looks up the exact row, matching anvil's
  cross-repository fixture. Runtime tests cover a crash after commit but before
  the receipt, a same-key retry leaving exactly one row, and both conflict
  kinds. Negative tests prove that quota, low disk, outage, busy, failed commit,
  indeterminate, rejected and conflict outcomes never return a receipt and never
  touch the spool.
- **Performance contract:** Inherits the KINTEG-013/014 budgets: warm ordinary
  append p95 < 1 ms and outage append p95 < 5 ms. The release-prep gate runs on
  a real ext4 or APFS disk and takes the median of 5 runs. It requires ordinary
  append p95 < 1 ms in a closed loop with keyed appends at 5/s on the same
  project, and keyed same-key replay p95 < 1 ms. A rate sweep (1, 5, 20 and
  50/s, plus saturated) is reported but not gated. CI runs smoke only.
- **Out of scope:** anvil-specific schemas, kinds or policy (anvil owns its
  `anvil.governance.v1` payload); unkeyed generic events, listing, pagination
  and batch (a later `generic-event-list@1`); a TypeScript client method, a CLI
  verb or an embedded-service-only API; a consumer allow-list for the detector;
  physical erasure on forget; retention or prune semantics beyond today's;
  cloud or sync.
- **Requested by:** anvil KFIT-005 (`eddacraft/anvil-001`,
  `plans/modules/kindling-product-fit.aps.md`). Its Track B (KFIT-007, -009,
  -010, -011, -013) waits on this release. anvil validates it with a
  cross-repository fixture that commits one keyed opaque event, restarts the
  runtime and retrieves the exact row. Before 015a starts, anvil should confirm
  that per-project key uniqueness meets KFIT-005, and confirm the key grammar
  and field naming against the detector fixtures (spec open questions 1–2).
- **Dependencies:** None upstream. Design approval is the open Ready Checklist
  item. The release is not coupled to C14: it is published manually through
  `scripts/publish.sh`.
- **Repositories:** `eddacraft/kindling`
- **Confidence:** medium. The design is settled. The remaining risks are
  macOS `F_FULLFSYNC` latency against the bench gate, and anvil's confirmation
  of the amended key domain.
- **Status:** Draft

### KINTEG-015a: Generic-event types, detector profile and schema revision 7

- **Intent:** The `generic-event@1` contract exists as shared types, fixtures
  and an additive schema revision, with no new reachable behaviour yet.
- **Expected Outcome:**
  - `kindling-types` defines the envelope, receipt, scope, lookup and forget
    states, `GenericEventLimits`, the capability feature entry and the detail
    enums (`RejectReason`, `ConflictKind`, `CommitFailure`), with the base64
    payload encoding. The ts-rs bindings are regenerated. Capability gains
    `schemaRevision` and `features` (both `serde(default)`), and `features` is
    empty for now.
  - One shared secret-rule table backs both profiles. Observation masking is
    unchanged. `GenericEventDetector` v1 matches the spec, and its vectors ship
    in `schema/fixtures/generic-event-detector-v1.json`.
  - Migration 007 and `schema.sql` add `generic_events`, the
    `generic_event_usage` counter and its triggers, and `schema_migrations` row
    7, without writing `user_version`. `version.json` carries
    `{version: 7, minReader: 6, wireVersion: 6, minCompatible: 1}`, and the
    vendored copies are synced. A single wire-version helper feeds `/v1/health`
    and `kindling status`.
  - The Rust and TS clients accept `6 <= schemaVersion <= MAX_WIRE_VERSION` and
    read `wireVersion` explicitly, which fixes both parse traps.
    `EXPECTED_SCHEMA_VERSION` keeps its name as the lower bound.
  - The server-side digest reproduces the spec's golden vectors in
    `schema/fixtures/generic-event-digest-v1.json`.
  - `schema/README.md`, `schema/schema.sql`, `CONTEXT.md`,
    `docs/architecture.md` and `docs/data-model.md` describe the revision,
    minReader and wire-version split.
- **Validation:** `cargo test -p kindling-types --features ts-rs` passes, with
  no bindings drift. `cargo test -p kindling-store` covers migration
  idempotence, the rewritten `user_version` pin tests, the golden
  no-coupling test, the frozen 0.4.1 open-check test and read-only `notFound`.
  `cargo test -p kindling-service` covers the detector fixture vectors, the
  masking parity test and the digest golden vectors. `cargo test -p kindling-client`
  covers the wire range and parse trap. Also run
  `pnpm --filter @eddacraft/kindling test`,
  `scripts/sync-vendored-schema.sh && git diff --exit-code` and
  `cargo clippy --all-targets --all-features -- -D warnings`.
- **Dependencies:** KINTEG-015 design approval (Ready Checklist).
- **Repositories:** `eddacraft/kindling`
- **Confidence:** high
- **Status:** Draft

### KINTEG-015b: Keyed durable write path, forget tombstone and export/import

- **Intent:** The service can durably reserve, replay, reject and tombstone
  keyed generic events, and can export and import them, with exact, closed
  outcomes.
- **Expected Outcome:**
  - A store-level durable write (FULL sync, plus fullfsync on macOS, verified
    before begin, an immediate transaction with a 1 s busy wait, and settings
    restored on every path) wraps every write to `generic_events`.
    `checkpoint_fullfsync` is on for every writable connection.
  - Keyed append follows the spec sequence. An existing key wins (receipt,
    mismatch or redacted conflict) before the detector, quota and free-space
    floor apply. Outcomes are typed so the daemon can map them to codes.
    `SQLITE_FULL`/`ENOSPC` map to low disk. A COMMIT-step I/O error reopens the
    service and re-checks the key.
  - The usage counter enforces an optional per-project quota, which is off by
    default. A cached free-space probe enforces a configurable floor.
  - Tombstone forget clears the payload and keeps the key, scope, digest and
    event id. It is idempotent and returns the original `redactedAt`.
  - Export writes `"1.0"`, byte-identical to 0.4, when there are no generic
    events, and `"2.0"` when there are any, tombstones included. Import of
    generic events is all-or-nothing, recomputes digests, does not re-scan,
    follows the spec's precedence table and reports stale detector versions
    and `dry_run` redacted counts.
  - Ordinary observation append, masking, forget and existing export and import
    behave as before.
- **Validation:** `cargo test -p kindling-store` and
  `cargo test -p kindling-service` pass. They cover the pragma-state test
  (success, error and panic), the keyed outcome matrix (insert, replay, both
  conflicts, a detector hit only on key miss, quota, an injected low-disk
  probe and `SQLITE_FULL`), the generic path never calling the hook-filter
  heuristic, `"1.0"` byte-identity, the `"2.0"` round-trip, import
  all-or-nothing (a mismatch on the last generic row leaves no entity writes)
  and import precedence. `cargo clippy --all-targets --all-features -- -D warnings`
  also passes.
- **Dependencies:** KINTEG-015a
- **Repositories:** `eddacraft/kindling`
- **Confidence:** medium
- **Status:** Draft

### KINTEG-015c: Daemon endpoints, capability wiring and client methods

- **Intent:** A daemon-backed client can append and look up keyed generic
  events, with typed errors, bounded deadlines and one safe recovery attempt.
- **Expected Outcome:**
  - `POST /v1/generic-events` (201 new, 200 replay) and
    `POST /v1/generic-events/lookup` run with per-project routing and a sized
    body limit. They return `{error, code, details}` bodies and the spec's
    status and code mapping. Existing endpoints are unchanged.
  - `kindling serve` runs on its own multi-thread runtime. Keyed work runs off
    the async workers behind a per-project single-permit gate, so ordinary
    handlers wait for at most one fsync and `/v1/health` stays responsive.
    Panics and poisoned locks recover.
  - The `generic-event` capability entry and its limits, including the
    configured quota and floor, are built, but not advertised by default until
    015d. Tests exercise them explicitly.
  - `kindling-client` gains append and lookup methods that return
    `AppendEventError` and `LookupEventError`. Each attempt has a 3 s deadline
    set in `ClientConfig`. An ambiguous outcome triggers exactly one same-key
    re-append, and anything other than a receipt, a conflict or a deterministic
    rejection then becomes `Indeterminate`. A missing feature or a bare 404/405
    gives `Unsupported`, and unknown feature entries are tolerated.
  - The daemon keeps keyed counters and logs errors and slow commits to stderr,
    never payload bytes.
- **Validation:** `cargo test -p kindling-server` covers every code, status and
  `details` shape, a 413 with and without a body, the capability golden shape
  and unknown-feature tolerance, and health responsiveness during a keyed
  commit. `cargo test -p kindling-client` covers the recovery matrix
  (deadline, a transport drop after send, `commit_indeterminate`, and a
  503/507/500 on recovery giving `Indeterminate`) and the negative tests (no
  receipt, spool file untouched). `cargo test -p eddacraft-kindling` and
  `cargo clippy --all-targets --all-features -- -D warnings` also pass.
- **Dependencies:** KINTEG-015b
- **Repositories:** `eddacraft/kindling`
- **Confidence:** medium
- **Status:** Draft

### KINTEG-015d: Runtime facade, forget endpoint, crash tests, bench gate, docs and 0.5.0

- **Intent:** anvil can depend on the published 0.5.0 `kindling-runtime` for
  keyed generic events, with crash behaviour proven, the performance gate met
  and the contract documented.
- **Expected Outcome:**
  - `POST /v1/generic-events/forget` and a client forget method returning
    `ForgetEventError` (gate G-3).
  - `kindling-runtime` exposes append, lookup and forget, re-exporting the
    closed enums. It checks the feature before the first write and after any
    reconnect, and pre-validates limits locally. Attaching to a 0.4.x daemon
    yields `Unsupported`, and there is never a spool fallback.
  - The daemon advertises `generic-event@1`.
  - A `test-failpoints` cargo feature drives `keyed_before_commit` and
    `keyed_after_commit` crash tests in CI on Linux, macOS and Windows. A
    nightly random-SIGKILL job checks that receipts are a subset of the rows on
    disk.
  - A `keyed-durable` kindling-bench group exists. The release-prep gate
    passes, and its results are recorded in ENGINEERING-HISTORY.
  - These docs are updated per the spec: SECURITY.md (a "Generic events"
    section), `schema/README.md`, `docs/data-model.md`, `docs/architecture.md`,
    `docs/development.md`, `docs/integrations.md`, `CONTEXT.md`, the client,
    runtime and bench READMEs, and `AGENTS.md`. CHANGELOG and
    ENGINEERING-HISTORY entries include the macOS durability caveat and the
    upgrade note.
  - The workspace is released as final 0.5.0 via `scripts/publish.sh`, with no
    release candidate, and tagged `v0.5.0` with GitHub Release assets.
- **Validation:** All of these pass:
  - `cargo test -p kindling-runtime --all-features`
  - `cargo clippy -p kindling-runtime --all-features -- -D warnings`
  - `cargo test -p eddacraft-kindling --features test-failpoints` (crash suite)
  - `cargo run -p kindling-bench --release -- --profile standard --only-group keyed-durable`,
    meeting the gate on ext4 or APFS
  - `python3 scripts/check-release-docs.py`
  - a package-content check
  - a clean scratch-crate install of the published 0.5.0 running the keyed
    round-trip across a runtime restart
- **Dependencies:** KINTEG-015c
- **Repositories:** `eddacraft/kindling`
- **Confidence:** medium
- **Status:** Draft
