# Post-merge: feat/kinteg-009-spool-retention

> Reconciled 2026-11-13: KINTEG-009 and KINTEG-003 are Complete; kindling-client 0.3.0 was published 2026-06-25 and both APIs are included in v0.4.0/v0.4.1. The old user-gated publication and KDS blockers below are historical.
> Current item records: [completed index](../../completed.aps.md).
> Original checklists below are retained as dated handoff evidence.

**PR:** #126  
**Merged:** 2026-06-26  
**APS:** KINTEG-009 (D-009 spool cap)

## Done (in-repo)

- [x] `SpoolConfig` `max_bytes` / `max_age_ms` (default `None`/unbounded)
- [x] `SpoolEntry.spooled_at` age basis
- [x] Oldest-prefix trim inside `flush()` lock; `SpoolStatus::dropped_count`
- [x] Unit + integration tests (13 spool tests)

## User-gated (maintainer)

- [ ] Bump workspace + `kindling-client` to **0.3.0** and `scripts/publish.sh`
      (`SpoolConfig` field addition is a breaking API change for direct struct literals)

## Anvil follow-on

- [ ] Wire 64 MiB / 7d caps in `KindlingDaemonSink` (anvil KDS-005)
- [ ] KDS-004 still blocked on KINTEG-003 / #2910
