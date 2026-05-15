# ADR 0001: Keep SQLite as Primary Store (Reject DuckDB Swap)

- Status: Accepted
- Date: 2026-05-14
- Deciders: Kindling maintainers
- Tags: storage, retrieval, embedded-db

## Context

Kindling persists observations (tool calls, diffs, commands, errors, workflow
events) and capsules (bounded units of meaning) into an embedded database.
Today this is SQLite, accessed through `better-sqlite3` on Node and `sql.js`
in the browser, with FTS5 powering full-text retrieval and WAL mode enabling
concurrent readers and a single writer across processes (the
`@eddacraft/kindling-server` HTTP layer fronts multi-agent writes).

A proposal surfaced to swap SQLite for [DuckDB](https://github.com/duckdb/duckdb)
on the grounds that DuckDB is a modern embedded engine with strong analytical
capabilities, SQL feature parity, and a WASM build. This ADR records the
decision on whether to make that swap.

### Workload shape

Kindling's workload is dominated by:

- High-frequency, small writes (one row per observation, often many per
  second during an active session).
- Short transactions with strong durability expectations.
- Retrieval driven by full-text search (FTS5) plus deterministic ranking,
  not by large analytical scans.
- Multi-process concurrent access: adapters (OpenCode, Claude Code,
  PocketFlow) may write while CLI tools and the HTTP server read.
- A browser variant (`@eddacraft/kindling-store-sqljs`) for in-page usage.

This is an OLTP profile with a search index on top — not an OLAP profile.

## Decision

**Keep SQLite as the primary embedded store. Do not replace it with DuckDB.**

Reconsider only if Kindling grows a first-class analytics surface (e.g.
cross-session insights, large-window aggregations over millions of
observations) that SQLite cannot serve efficiently. In that scenario, the
preferred path is to add DuckDB as a secondary read-only analytics engine
over the same SQLite file via DuckDB's `sqlite_scanner` extension, rather
than to migrate the source of truth.

## Rationale

### Why SQLite stays

1. **Write profile matches.** SQLite with WAL is purpose-built for many
   small write transactions from multiple processes. DuckDB's storage is
   columnar and optimised for bulk loads and analytical scans; single-row
   inserts and short transactions are not its strength.
2. **Concurrency model fits.** SQLite's WAL gives Kindling multi-reader /
   single-writer semantics across processes, which is exactly what the
   adapter + server + CLI topology needs. DuckDB historically restricts
   write access to a single process at a time, which would force every
   writer through `kindling-server` and remove the option of direct
   in-process writes.
3. **FTS5 is the retrieval backbone.** Kindling's retrieval contract
   depends on FTS5's tokenisers, ranking, and snippet generation. DuckDB's
   `fts` extension exists but is less mature, lacks feature parity, and
   would require re-implementing the deterministic ranking we rely on.
4. **Browser story is already solved.** `sql.js` gives us a working
   in-browser store with the same schema. `duckdb-wasm` exists but is
   substantially heavier and would diverge the schema/feature set between
   Node and browser builds.
5. **Migration cost is real.** A swap touches `kindling-store-sqlite`,
   `kindling-store-sqljs`, every migration in
   `packages/kindling-store-sqlite/migrations/`, the retrieval provider,
   and the server. None of that work buys us capability we are
   currently constrained on.

### Why DuckDB is tempting

- Excellent analytical SQL (window functions, complex aggregations, joins
  across large tables) — useful if Kindling ever exposes cross-session
  analytics.
- Columnar storage compresses well for append-only observation logs.
- Native Parquet/Arrow interop would simplify exporting observation
  history for offline analysis.

These are real strengths, but they address a workload Kindling does not
have today.

## Consequences

### Positive

- No migration; existing schema, migrations, FTS5 indexes, and adapter
  contracts remain stable.
- Multi-process write concurrency is preserved.
- Browser parity via `sql.js` is preserved.
- Retrieval determinism and explainability — anchored in FTS5 behaviour —
  remain unchanged.

### Negative / Accepted tradeoffs

- Large-window analytical queries over the full observation history will
  remain slower than a columnar engine would offer.
- If a future feature genuinely needs OLAP performance, we will need to
  introduce a second engine rather than consolidating on one.

### Follow-ups

- Revisit this ADR if any of the following becomes true:
  - Cross-session analytics becomes a first-class product surface.
  - Observation volume per deployment exceeds what SQLite + FTS5 can serve
    interactively (rough trigger: p95 retrieval > 250 ms on representative
    corpora after index tuning).
  - DuckDB ships mature multi-process write support and an FTS implementation
    comparable to FTS5.
- If/when revisited, prefer the "DuckDB as analytics sidecar over the
  SQLite file" path before considering a source-of-truth migration.

## Alternatives Considered

1. **Full swap to DuckDB (rejected).** Discussed above; trades a strong
   OLTP+FTS fit for OLAP strengths Kindling does not currently need.
2. **Dual-write to SQLite and DuckDB (rejected for now).** Doubles write
   cost and operational surface area without a concrete consumer for the
   DuckDB side.
3. **DuckDB as a read-only analytics layer over SQLite (deferred).**
   Viable future option via `sqlite_scanner`; revisit when an analytics
   use case lands.

## References

- `docs/architecture.md` — layered architecture and storage role
- `docs/retrieval-contract.md` — retrieval determinism and ranking
- `packages/kindling-store-sqlite/` — current implementation
- DuckDB: https://github.com/duckdb/duckdb
- DuckDB `sqlite_scanner`: https://duckdb.org/docs/extensions/sqlite
