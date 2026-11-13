# Kindling performance hardening

> Reconciled 2026-11-13: KINTEG-013 is delivered in v0.4.0/v0.4.1 (PR #143). Benchmark defects remain separately tracked in GitHub; this design is retained as historical rationale.
> Current status and archive: [plan index](../index.aps.md).

**Status:** Approved for KINTEG-013 by the 2026-08-03 implementation request.

## Problem

During a sustained daemon outage, `SpooledClient::append_observation` currently
counts the spool by parsing every NDJSON row, attempts a flush that reads and
rewrites the backlog, then retries the unavailable daemon for the new row. The
cost of buffering a new row therefore grows with backlog depth.

## Decision

Keep the existing NDJSON durability and single-producer contract, but maintain
the live pending count in memory after one initial read. After a connectivity
failure, use a short in-memory retry backoff. New observations arriving during
that backoff append directly behind the backlog. When the backoff expires, a
lightweight health request probes recovery before the spool is read. If the
daemon remains unavailable, the new observation is appended without touching
the backlog; if healthy, ordered flush resumes before the new row is delivered.

The on-disk spool remains authoritative across process restarts. A newly
constructed client initializes its cache from disk, and passive status continues
to parse the file. Successful rewrites refresh the cache. The backoff is not
persisted, so a restarted process immediately probes recovery.

## Preserved contracts

- Stable ids are assigned before either delivery or spooling.
- Replay remains ordered and stops at the first failure.
- Daemon-side id deduplication preserves exactly-once-ish application.
- Retention still drops only an oldest prefix under configured caps.
- The spool remains single-producer and has no cross-process lock.
- kindling supplies storage and query mechanisms, not downstream aggregation
  or governance policy.

## Evidence

The regression test observes replay-attempt counters through `spool_status()`;
an outage burst must not produce one replay attempt per append. The performance
harness records early and late windows separately so a backlog slope is visible
instead of hidden in one aggregate percentile.

Batching replay-attempt sidecar persistence is an internal I/O optimization, so
a call-count TDD test would couple the contract to a private helper. Existing
status tests prove the public cumulative counter, the spool suite proves replay
semantics, and the replay benchmark is the replacement evidence for that edit.
