# kindling Schema Contract

This directory contains the **SQLite schema contract** for kindling: the canonical database layout. The Rust store (`kindling-store`) implements it, and any external tool that reads a kindling database can rely on it. The retired TypeScript store (`@eddacraft/kindling-store-sqlite`, now removed from this repo) was the original second implementation, which is why some comments still say "cross-language".

`kindling-store` embeds a **vendored copy** of these files under `crates/kindling-store/schema/`, because `cargo publish` only packages files inside the crate directory. Run `scripts/sync-vendored-schema.sh` after every change here. The `vendored-schema` CI job fails when the copy drifts.

## Files

| File           | Purpose                                                       |
| -------------- | ------------------------------------------------------------- |
| `schema.sql`   | Canonical DDL — all tables, indexes, triggers, and FTS config |
| `version.json` | Machine-readable version metadata for build/startup checks    |
| `README.md`    | This document                                                 |

## `PRAGMA user_version`

Every migrated database stores the current schema version as `PRAGMA user_version`. Any SQLite client can read it with:

```sql
PRAGMA user_version;
```

The value matches `version` in `version.json`. The TypeScript migration runner sets it; the Rust crate reads it at startup to verify compatibility.

## How to add a migration

1. Update `schema/schema.sql` so that it describes the schema **after** every migration has run, which is what a fresh database gets. Bump `PRAGMA user_version = <N>;` and add a row to the `schema_migrations` seed.
2. Update `schema/version.json`: set `"version"` to `<N>`. If the change is breaking (see below), also bump `"minCompatible"`.
3. In `crates/kindling-store/src/db.rs`, add a forward migration (a `MIGRATION_<NNN>_<NAME>` SQL constant, run inside `BEGIN IMMEDIATE … COMMIT`) and apply it in `ensure_schema` for databases at the previous version. Keep it additive and idempotent.
4. Run `scripts/sync-vendored-schema.sh` and commit the vendored copy.
5. Add a test: `crates/kindling-store/tests/golden_file.rs` covers the fresh-database DDL, and the store integration tests cover upgrades from the previous version.
6. If the change affects the wire types or the observation `kind` `CHECK` list, regenerate the bindings with `cargo test -p kindling-types --features ts-rs`.

A schema bump is a public contract change. Open an issue first (see [CONTRIBUTING.md](../CONTRIBUTING.md#discuss-first)).

## Breaking changes

The following changes are **breaking** for cross-language consumers:

- **Column removal or rename** — the Rust crate queries columns by name.
- **Column type change** — e.g. `INTEGER` → `TEXT` for an existing column.
- **FTS tokenizer change** — different tokenizers produce different token streams, so search results will diverge. The current tokenizer is `porter unicode61` (Porter stemming + Unicode normalization). This is documented in `schema.sql` and pinned in `version.json` as `ftsTokenizer`.
- **CHECK constraint change** — e.g. adding/removing allowed `kind` values changes what rows are valid.

When making a breaking change, bump `minCompatible` in `version.json` to the migration version that introduced the break. The Rust crate checks `minCompatible` at startup and refuses to open a database it cannot safely read.

## How the Rust crate uses these files

1. At **build time**, `kindling-store` embeds its vendored `schema.sql` and `version.json` with `include_str!`.
2. When it **opens a database**:
   - A fresh database gets `schema.sql` applied.
   - An existing database whose `PRAGMA user_version` is `0` or below `minCompatible` is refused (`SchemaTooOld`).
   - A database newer than the embedded version is refused (`SchemaTooNew`).
   - A database at an older supported version is migrated forward.
3. The daemon reports its schema version in the `/v1/health` capability handshake. Clients use that to detect skew.
