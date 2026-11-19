# Downstream Integration Surface (kindling)

| ID     | Owner  | Status      |
| ------ | ------ | ----------- |
| KINTEG | @aneki | In Progress |

**Last reviewed:** 2026-11-19.

## Purpose

Maintain the mechanism contract consumed by anvil and other integrations.
Thirteen delivered items are in the [archive](../archive/modules/06-downstream-integration-surface.aps.md)
and [completed index](../completed.aps.md). Active work: the full
adapter-fixture scope (KINTEG-007) and the next-minor generic-event contract
(KINTEG-015), delivered in sequence through KINTEG-015a to 015e
([design](../specs/2026-11-18-generic-event-keyed-append-design.md)).

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
- [x] KINTEG-015 design and release scope approved before implementation
      ([#282](https://github.com/eddacraft/kindling/pull/282), merged 2026-11-19)

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
  under a key and hold a receipt proving it is durably retained, without kindling
  learning their policy.
- **Design:** [Generic events and keyed durable append](../specs/2026-11-18-generic-event-keyed-append-design.md)
  (council plan-9ad9b830, revised after the Phase 4 review). The spec is
  authoritative for numbers, codes and mechanisms. Its
  [Amendments](../specs/2026-11-18-generic-event-keyed-append-design.md#amendments-to-the-original-kinteg-015-wording)
  section changes this item's original wording.
- **Expected Outcome:** Final 0.5.0 ships through `kindling-runtime`,
  `kindling-client` and v1 endpoints, behind one capability feature:
  - A consumer can append an opaque, versioned event under a key. It gets a
    receipt only once the row is durable. An identical retry returns the same
    receipt and never creates a second row. Different content under the key, or
    any append to a forgotten key, is a conflict
    ([Wire contract](../specs/2026-11-18-generic-event-keyed-append-design.md#wire-contract-t2),
    [Durability](../specs/2026-11-18-generic-event-keyed-append-design.md#durability-concurrency-and-failure-handling-t3)).
  - After a restart, the consumer can look up the exact row and its receipt. It
    can also forget the row while the key stays reserved
    ([Forget](../specs/2026-11-18-generic-event-keyed-append-design.md#forget)).
  - Every failure is one of the spec's closed outcomes. A failure never returns
    a receipt and never writes to the spool
    ([Error codes](../specs/2026-11-18-generic-event-keyed-append-design.md#error-codes-and-invariants),
    [Recovery](../specs/2026-11-18-generic-event-keyed-append-design.md#recovery-flow)).
  - A consumer can tell before writing whether the attached daemon supports the
    contract, and 0.4.x answers "unsupported"
    ([Capability](../specs/2026-11-18-generic-event-keyed-append-design.md#capability-and-versioning)).
  - 0.4.x binaries keep working against 0.5 databases and daemons. Ordinary
    append, replay, masking and redaction are unchanged
    ([Mixed-version matrix](../specs/2026-11-18-generic-event-keyed-append-design.md#mixed-version-matrix-t1)).
- **Delivery:** Five sequential PRs (015a to 015e) ship together as one 0.5.0
  release behind one capability. Nothing is advertised until 015d merges.
  - No release is cut from `main` between the 015a and 015d merges. Any 0.4.x
    patch comes from `release/0.4`.
  - Each sub-item adds its own `Unreleased` CHANGELOG and ENGINEERING-HISTORY
    entries.
  - See [Delivery and release](../specs/2026-11-18-generic-event-keyed-append-design.md#delivery-and-release).
- **Validation:** Every sub-item's validation passes. The last check is 015e's
  packaged check and post-publish install, which reproduce anvil's fixture:
  commit one keyed event, restart, and look up the exact row with the project id
  asserted
  ([Test plan](../specs/2026-11-18-generic-event-keyed-append-design.md#test-and-validation-plan)).
- **Performance contract:** Inherits the KINTEG-013/014 budgets unchanged. The
  keyed-durable gate, the gating platform, the action if the gate fails and the
  APFS reporting are defined in
  [Performance](../specs/2026-11-18-generic-event-keyed-append-design.md#performance-contract-and-bench-t3-g-2).
- **Out of scope:** The spec's
  [non-goals](../specs/2026-11-18-generic-event-keyed-append-design.md#goals-and-non-goals):
  anvil schemas, kinds or policy; unkeyed events, listing, pagination and batch;
  a TypeScript client method, a CLI verb or an embedded-service API; a detector
  allow-list; physical erasure; retention or prune changes; cloud or sync.
- **Requested by:** anvil KFIT-005 (`eddacraft/anvil-001`,
  `plans/modules/kindling-product-fit.aps.md`). Its Track B (KFIT-007, -009,
  -010, -011, -013) waits on this release. anvil validates the release with a
  cross-repository fixture that commits one keyed opaque event, restarts the
  runtime and retrieves the exact row. anvil answered the spec's open
  questions 1–2 on 2026-11-19 (both resolved).
- **Dependencies:** None upstream. Design approval is recorded in the Ready
  Checklist (#282). This item is not coupled to C14.
- **Repositories:** `eddacraft/kindling`
- **Confidence:** medium. The main risk is macOS full-fsync cost; anvil has
  confirmed the per-project key domain and the settings surface is resolved
  (open questions 1 and 4).
- **Status:** Draft

### KINTEG-015a: Generic-event types, detector profile and schema revision 7

- **Intent:** The `generic-event@1` contract exists as shared types, published
  fixtures and an additive schema revision. There is no new reachable behaviour
  yet.
- **Expected Outcome:**
  - The wire shapes and detail enums in
    [Rust API](../specs/2026-11-18-generic-event-keyed-append-design.md#rust-api)
    exist in `kindling-types`. Their TypeScript bindings follow the spec's type
    mapping.
  - The rules in
    [GenericEventDetector v1](../specs/2026-11-18-generic-event-keyed-append-design.md#genericeventdetector-v1-rules)
    and their fixture file are published. Observation masking output is
    unchanged.
  - Fresh and migrated stores carry the revision described in
    [Storage](../specs/2026-11-18-generic-event-keyed-append-design.md#storage-and-schema-compatibility-t1),
    and a 0.4.1 binary still opens them.
  - The Rust and TS clients accept any daemon within the spec's wire range.
    `/v1/health` and `kindling status` report the wire and revision stamps.
  - The server digest reproduces the golden vectors in
    [Digest framing](../specs/2026-11-18-generic-event-keyed-append-design.md#digest-framing).
  - `Unreleased` records describe the schema-contract change.
- **Validation:** All of the following pass:
  - `cargo test -p kindling-types --features ts-rs`, with no bindings drift.
  - `cargo test -p kindling-store`: migration idempotence, the rewritten pin
    tests, the no-coupling golden test, the frozen 0.4.1 open check, and each
    open-rule row.
  - `cargo test -p kindling-service`: the detector fixtures, the equality check
    between the fixture and the spec block, masking parity, and the digest
    vectors.
  - `cargo test -p kindling-client`: the wire range and the parse trap.
  - `pnpm --filter @eddacraft/kindling test`
  - `scripts/sync-vendored-schema.sh && git diff --exit-code`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `python3 scripts/check-release-docs.py`
- **Dependencies:** KINTEG-015 design approval (Ready Checklist, #282); the
  spec's open questions 1–2, resolved by anvil on 2026-11-19
- **Repositories:** `eddacraft/kindling`
- **Confidence:** high
- **Status:** Ready

### KINTEG-015b: Keyed durable write path, durability barrier, forget tombstone and export/import

- **Intent:** The service durably reserves, replays, rejects and tombstones
  keyed generic events. It never vouches for a row whose durability it has not
  confirmed, and it exports and imports these events losslessly.
- **Expected Outcome:**
  - Every generic-event write is fully synced. Ordinary connection settings are
    left as they were, as described in
    [Durable write](../specs/2026-11-18-generic-event-keyed-append-design.md#durable-write).
  - After any open, reopen, commit-step error or panic, no existing row is
    receipted, found or forgotten until its durability is confirmed, as
    described in
    [Durability barrier and keyed-suspect state](../specs/2026-11-18-generic-event-keyed-append-design.md#durability-barrier-and-keyed-suspect-state).
  - Keyed append, forget and their failures follow
    [Sequences and server errors](../specs/2026-11-18-generic-event-keyed-append-design.md#sequences-and-server-errors).
    An existing key always wins. Only the failures the spec marks as definite
    report "not committed".
  - Lookup returns a row only when its recomputed digest matches.
  - The optional quota and the low-disk floor reject new rows but never a
    replay. The quota is set through `RuntimeConfig` only (open question 4,
    resolved); the floor's default value and probe are pending open questions 3
    and 5.
  - Bundles and imports follow
    [Export bundles](../specs/2026-11-18-generic-event-keyed-append-design.md#export-bundles)
    and [Import](../specs/2026-11-18-generic-event-keyed-append-design.md#import).
    Bundles without generic events, and their import, behave exactly as in 0.4.
  - Ordinary observation behaviour is unchanged.
  - `Unreleased` records are updated.
- **Validation:** `cargo test -p kindling-store` and
  `cargo test -p kindling-service` pass, covering:
  - the pragma-state test (success, error and panic);
  - the barrier test: reopen a DB whose WAL holds an unsynced commit frame and
    assert the barrier runs before any replay;
  - commit-error and panic tests: never a receipt, the project becomes suspect,
    and the mutex is not poisoned;
  - the keyed outcome matrix;
  - a tampered row returning `digest_mismatch`;
  - a read-only open returning `notFound`;
  - the generic path never calling the hook-filter heuristic;
  - export byte-identity, and import all-or-nothing behaviour and precedence.

  `cargo clippy --all-targets --all-features -- -D warnings` and
  `python3 scripts/check-release-docs.py` also pass.

- **Dependencies:** KINTEG-015a
- **Repositories:** `eddacraft/kindling`
- **Confidence:** medium
- **Status:** Draft

### KINTEG-015c: Daemon append and lookup routes, concurrency and client recovery

- **Intent:** A daemon-backed client can append and look up keyed generic events
  with typed outcomes, bounded waits and one safe recovery attempt. The keyed
  path does not stall the daemon beyond the stated bound.
- **Expected Outcome:**
  - The append and lookup routes answer with the codes, statuses and details in
    [Error codes and invariants](../specs/2026-11-18-generic-event-keyed-append-design.md#error-codes-and-invariants).
    Existing endpoints are unchanged.
  - Keyed work never runs on the async workers. An ordinary request waits no
    longer than the bound in
    [Concurrency and shutdown](../specs/2026-11-18-generic-event-keyed-append-design.md#concurrency-and-shutdown),
    and a poisoned lock never takes the daemon down.
  - The capability entry is built, but advertised only under the test-only
    feature named in
    [Capability](../specs/2026-11-18-generic-event-keyed-append-design.md#capability-and-versioning).
  - `kindling-client` exposes the append and lookup methods and the per-call
    options in
    [Rust API](../specs/2026-11-18-generic-event-keyed-append-design.md#rust-api).
    It resolves ambiguous outcomes exactly as in
    [Recovery flow](../specs/2026-11-18-generic-event-keyed-append-design.md#recovery-flow).
  - Keyed counters and logs exist, and they never contain payload bytes.
  - `Unreleased` records are updated.
- **Validation:**
  - `cargo test -p kindling-server`: every code, status and `details` shape; a
    413 with and without a body; the capability golden shape; tolerance of
    unknown features. With more concurrent ordinary appends than workers during
    an injected slow keyed commit, `/v1/health` still responds. When a client
    deadline fires mid-fsync, the ordinary wait stays within one keyed
    transaction. With several keyed requests queued, an ordinary append still
    waits for at most one keyed transaction.
  - `cargo test -p kindling-client`: the recovery matrix (404, 413, 401, 503,
    507 or 500 on the recovery attempt returns `Indeterminate`); the send
    boundary; the negative tests (no receipt, spool untouched).
  - `cargo test -p eddacraft-kindling`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `python3 scripts/check-release-docs.py`
- **Dependencies:** KINTEG-015b
- **Repositories:** `eddacraft/kindling`
- **Confidence:** medium
- **Status:** Draft

### KINTEG-015d: Forget route, runtime facade, advertising and crash tests

- **Intent:** A `kindling-runtime` consumer can append, look up and forget keyed
  generic events against an advertised capability, with crash behaviour proven
  in CI.
- **Expected Outcome:**
  - Forget is reachable through the daemon, the client and the runtime, with the
    spec's closed outcomes (gate G-3).
  - The runtime exposes the methods, the feature check and the project id listed
    in
    [Rust API](../specs/2026-11-18-generic-event-keyed-append-design.md#rust-api).
    It reports when its settings were not applied to an attached daemon. It
    never falls back to the spool, and its shutdown waits for in-flight keyed
    work.
  - The daemon advertises `generic-event@1`.
  - Failpoint crash tests run in CI, including on a new macOS job, and a new
    nightly crash workflow exists. Both are new CI, pending open question 6.
  - `Unreleased` records are updated.
- **Validation:**
  - `cargo test -p kindling-runtime --all-features`: feature checks;
    `Unsupported` against a 0.4-like daemon; `settings_applied`; `project_id()`
    matching the receipt; a lookup under a different root returning `notFound`;
    shutdown waiting for in-flight work; the negative tests.
  - `cargo clippy -p kindling-runtime --all-features -- -D warnings`
  - `cargo test -p eddacraft-kindling --features test-failpoints` (the crash
    suite) on Linux, macOS and Windows.
  - The forget tests in `cargo test -p kindling-server` and
    `cargo test -p kindling-client`.
  - `python3 scripts/check-release-docs.py`
- **Dependencies:** KINTEG-015c
- **Repositories:** `eddacraft/kindling`
- **Confidence:** medium
- **Status:** Draft

### KINTEG-015e: Release prep and 0.5.0 publication

- **Intent:** The published 0.5.0 crates deliver the contract as specified, and
  this is proven both before and after publishing.
- **Expected Outcome:**
  - The ext4 bench gate in
    [Performance](../specs/2026-11-18-generic-event-keyed-append-design.md#performance-contract-and-bench-t3-g-2)
    passes on the release commit. Its results, with the APFS report, are
    recorded in ENGINEERING-HISTORY. A gate failure blocks the release and
    returns the decision to the maintainer.
  - Every document listed in
    [Documentation and release notes](../specs/2026-11-18-generic-event-keyed-append-design.md#documentation-and-release-notes)
    is current, including the macOS caveat and the upgrade note.
  - The pre-publish packaged check in
    [Delivery and release](../specs/2026-11-18-generic-event-keyed-append-design.md#delivery-and-release)
    passes before anything is published.
  - Final 0.5.0 is published, with no release candidate, and tagged. A clean
    install from crates.io repeats the anvil round-trip.
- **Validation:**
  - `cargo run -p kindling-bench --release -- --profile standard --only-group keyed-durable`
    meets the gate on ext4.
  - `cargo package --workspace`
  - The scratch crate runs commit, restart and lookup against the packaged
    crates.
  - `cargo test -p eddacraft-kindling --features test-failpoints` passes on the
    release commit.
  - `DRY_RUN=1 scripts/publish.sh`
  - `python3 scripts/check-release-docs.py`
  - Then `scripts/publish.sh`, followed by the post-publish scratch install.
- **Dependencies:** KINTEG-015d
- **Repositories:** `eddacraft/kindling`
- **Confidence:** medium
- **Status:** Draft
