# Kindling Schema Contract

This directory contains the **cross-language SQLite schema contract** for Kindling. Both the TypeScript store (`@eddacraft/kindling-store-sqlite`) and the Rust crate implement against the files here.

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

1. Create `packages/kindling-store-sqlite/migrations/<NNN>_<name>.sql`.
2. Include `PRAGMA user_version = <NNN>;` in the migration file.
3. Register the migration in `packages/kindling-store-sqlite/src/db/migrate.ts`.
4. Update `schema/schema.sql` to reflect the new DDL state (the file should always represent the schema **after** all migrations have run).
5. Update `schema/version.json` — set `"version"` to `<NNN>`.
6. If you changed the FTS tokenizer or removed/renamed columns, update `"minCompatible"` (see below).

## Breaking changes

The following changes are **breaking** for cross-language consumers:

- **Column removal or rename** — the Rust crate queries columns by name.
- **Column type change** — e.g. `INTEGER` → `TEXT` for an existing column.
- **FTS tokenizer change** — different tokenizers produce different token streams, so search results will diverge. The current tokenizer is `porter unicode61` (Porter stemming + Unicode normalization). This is documented in `schema.sql` and pinned in `version.json` as `ftsTokenizer`.
- **CHECK constraint change** — e.g. adding/removing allowed `kind` values changes what rows are valid.

When making a breaking change, bump `minCompatible` in `version.json` to the migration version that introduced the break. The Rust crate checks `minCompatible` at startup and refuses to open a database it cannot safely read.

## How the Rust crate uses these files

1. At **build time**, the Rust crate can embed `version.json` and assert it was compiled against the expected schema version.
2. At **startup**, the crate reads `PRAGMA user_version` from the database and compares it to the embedded version. If the database version is below `minCompatible`, the crate returns an error.
3. The DDL in `schema.sql` serves as the reference for Rust struct definitions and query construction.
