# Downstream Integration Surface (kindling)

| ID     | Owner  | Status      |
| ------ | ------ | ----------- |
| KINTEG | @aneki | In Progress |

**Last reviewed:** 2026-11-13.

## Purpose

Maintain the mechanism contract consumed by anvil and other integrations.
Thirteen delivered items are in the [archive](../archive/modules/06-downstream-integration-surface.aps.md)
and [completed index](../completed.aps.md). Two items remain active: the full
adapter-fixture scope and the proposed next-minor event contract.

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

- **Intent:** Downstream Rust consumers can store an opaque, versioned event and
  get proof that it is durably retained, without kindling learning their policy.
- **Expected Outcome:** The next kindling minor (0.5.0 unless the changelog says
  otherwise) ships, through `kindling-runtime`:
  - **`generic-event@1`:** a generic outer envelope that preserves an opaque,
    size-bounded consumer payload with a consumer-declared schema name and
    version. kindling validates only the envelope (size, scope, schema
    identifier), never the payload's meaning. The existing closed
    `ObservationKind` set and its behaviour are unchanged.
  - **Keyed direct-retained append:** a path distinct from ordinary spooled
    append. The caller supplies an opaque correlation key, unique across the
    owner-local profile. Append atomically reserves the key with the canonical
    caller scope and payload digest, and returns a receipt only after the row is
    durable in the retained store. A same-key retry of identical content returns
    the original receipt without a second row; a different scope or payload
    under the same key is a conflict.
  - **Exact lookup by key after restart:** the receipt and lookup return the
    same event ID, scope and digest.
  - **Closed failure result:** quota, low disk, unavailable runtime, rejected
    payload, failed commit and failed lookup. This path never reports spool
    acceptance as a retained receipt and never falls back to the spool.
  - **Capability negotiation:** a consumer can tell before writing that the
    attached daemon supports both features. Releases through 0.4.x answer
    "unsupported" and are documented as incompatible with this contract.
  - Ordinary spooled append, replay and existing redaction behaviour are
    unchanged.
- **Validation:** `cargo test -p kindling-runtime --all-features` and
  `cargo clippy -p kindling-runtime --all-features -- -D warnings`, a
  package-content check and a clean scratch-crate install of the published
  version. Runtime tests cover a crash after commit but before the receipt, a
  same-key retry leaving exactly one row, conflicting-key rejection, and a
  scope/digest mismatch on lookup. Negative tests prove that quota, low disk,
  outage, failed commit and failed lookup never return a retained receipt or
  fall back to the spool.
- **Performance contract:** Inherit the KINTEG-013/014 budgets. The keyed append
  adds one durable commit and must not regress warm ordinary append p95 below
  1 ms, or outage append p95 below 5 ms.
- **Out of scope:** Any anvil-specific schema, event kinds or policy (anvil owns
  its `anvil.governance.v1` payload inside the envelope); retention or prune
  semantics beyond today's; cloud or sync.
- **Requested by:** anvil KFIT-005 (`eddacraft/anvil-001`,
  `plans/modules/kindling-product-fit.aps.md`), whose Track B (KFIT-007, -009,
  -010, -011, -013) waits on this release. anvil validates it with a
  cross-repository fixture that commits one keyed opaque event, restarts the
  runtime and retrieves the exact row.
- **Dependencies:** None upstream; coordinate the release with C14 (publishing
  crates from `release.yml`, module 08) if that lands first.
- **Repositories:** `eddacraft/kindling`
- **Confidence:** medium
- **Status:** Draft
