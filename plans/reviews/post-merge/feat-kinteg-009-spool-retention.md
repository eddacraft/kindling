# Post-merge: feat/kinteg-009-spool-retention

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
