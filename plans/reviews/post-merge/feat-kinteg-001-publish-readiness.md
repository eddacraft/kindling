# Post-merge: feat/kinteg-001-publish-readiness

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
- [ ] KINTEG-008: implement `kindling-runtime` facade (plan only in this PR)
