# Post-merge: feat/kinteg-001-publish-readiness

## Credential-gated (maintainer)

- [ ] `cargo login` then `scripts/publish.sh` to publish all seven crates at 0.2.0
- [ ] Scratch crate: `cargo add kindling-client@0.2.0 --features spool` resolves
- [ ] Confirm `docs.rs/kindling-client` shows `SpooledClient` (all-features build)

## Verification (automated in CI)

- [ ] `cargo test -p eddacraft-kindling --test publish_readiness` passes
- [ ] `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings` pass

## Follow-up (not this PR)

- [ ] PORT-011: anvil integration proof with raw `kindling-client`
- [ ] KINTEG-008: implement `kindling-runtime` facade (plan only in this PR)
