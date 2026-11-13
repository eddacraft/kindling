# Kindling scoped-read index migration

> Reconciled 2026-11-13: KINTEG-014 is delivered in v0.4.0/v0.4.1 (PR #143). This is implementation rationale, not a second active task list.
> Current status and archive: [plan index](../index.aps.md).

**Status:** Approved for KINTEG-014 by the 2026-08-03 implementation request.

## Evidence and decision

The bounded list API orders and pages on `(ts ASC, id ASC)`. Its repo and
session indexes currently cover only `(scope_id, ts DESC)`, and a representative
`EXPLAIN QUERY PLAN` reports a temporary B-tree for the final ordering term.

Migration 006 rebuilds those two partial indexes as `(scope_id, ts ASC, id ASC)`.
SQLite can scan the same indexes in reverse for existing descending-time reads,
so separate ascending and descending copies are unnecessary. The migration is
transactional and records `PRAGMA user_version = 6`; fresh canonical databases
contain the final shape directly.

Writable v5 databases migrate when opened. Read-only v5 databases remain
compatible and readable, but retain their old performance until opened once by
a writable v6 store. Versions older than v5 are not opportunistically rewritten
by this migration because their table/column provenance is not established here.

## Non-goals

- No total counts or server-side aggregation.
- No downstream governance vocabulary or policy.
- No change to list filters, cursor encoding, limits, or result ordering.
- No new search engine; ranked text retrieval continues to use FTS5.
