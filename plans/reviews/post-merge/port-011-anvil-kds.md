# Post-merge: PORT-011 (anvil KDS proof)

**Completed:** 2026-06-24  
**Anvil:** eddacraft/anvil-001 PR #2897 (KDS-001 + KDS-003), PR #2906 (KDS-002)  
**Kindling:** [#124](https://github.com/eddacraft/kindling/issues/124)

## Kindling repo (done)

- [x] PORT-011 → **Merged** in `plans/modules/05-rust-port.aps.md`
- [x] Index success criterion: anvil emits via `kindling-client` (opt-in daemon sink)
- [x] `plans/reviews/post-merge/feat-kinteg-001-publish-readiness.md` PORT-011 checked

## Anvil follow-on (not PORT-011 scope)

- [ ] KDS-004 — usage views from daemon store (blocked on kindling KINTEG-003 / #2910)
- [ ] KDS-005 — retire NDJSON writer (KINTEG-009 Done #126; still blocked on KDS-004 / #2910)
- [ ] E2E with real `kindling` binary on PATH + `ANVIL_KINDLING_SINK=daemon` (dogfood)

## Kindling unblockers (D-009)

- [x] KINTEG-009 spool cap — PR #126 (merged 2026-06-26); publish `kindling-client` ≥ 0.3.0 user-gated
- [ ] KINTEG-003 list API — next (unblocks anvil KDS-004 / #2910)
