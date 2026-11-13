# Post-merge: feat/kinteg-001-publish-readiness

> Reconciled 2026-11-13: KINTEG-001 is Complete (crates.io 0.2.0, published 2026-06-23). Runtime publication followed at 0.3.0; 0.4.0/0.4.1 are live. PORT-011 is released in anvil v0.9.0-beta.
> Current item records: [completed index](../../completed.aps.md).
> Original checklists below are retained as dated handoff evidence.

**Completed:** 2026-06-24 — KINTEG-001 Done (all seven workspace crates at 0.2.0 on crates.io).

## Credential-gated (maintainer)

- [x] `cargo login` then `scripts/publish.sh` to publish all seven crates at 0.2.0
- [x] Scratch crate: `cargo add kindling-client@0.2.0 --features spool` resolves from registry
- [x] Confirm `docs.rs/kindling-client/0.2.0` shows `SpooledClient` (all-features build)

## Verification (automated in CI)

- [x] `cargo test -p eddacraft-kindling --test publish_readiness` passes
- [x] `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings` pass

## Follow-up (not this PR)

- [x] PORT-011: anvil integration proof with raw `kindling-client` — **Merged**
      (anvil PR #2897 KDS-001/003 + #2906 KDS-002, 2026-06-24); see
      [`plans/execution/PORT-011-anvil-handoff.md`](../../execution/PORT-011-anvil-handoff.md)
- [x] KINTEG-008: `kindling-runtime` facade — **Merged** (PR #129, 2026-06-26); publish user-gated
