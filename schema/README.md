# kindling Schema Contract

This directory contains the **SQLite schema contract** for kindling: the canonical database layout. The Rust store (`kindling-store`) implements it, and any external tool that reads a kindling database can rely on it. The retired TypeScript store (`@eddacraft/kindling-store-sqlite`, now removed from this repo) was the original second implementation, which is why some comments still say "cross-language".

`kindling-store` embeds a **vendored copy** of these files under `crates/kindling-store/schema/`, and `kindling-client` embeds `version.json` under `crates/kindling-client/schema/`, because `cargo publish` only packages files inside the crate directory. Run `scripts/sync-vendored-schema.sh` after every change here. The `vendored-schema` CI job fails when the copy drifts.

## Files

| File           | Purpose                                                       |
| -------------- | ------------------------------------------------------------- |
| `schema.sql`   | Canonical DDL — all tables, indexes, triggers, and FTS config |
| `version.json` | Machine-readable version metadata for build/startup checks    |
| `fixtures/`    | Golden vectors for `generic-event@1` digest and detector      |
| `README.md`    | This document                                                 |

## Version stamps (`minReader`, revision, and `wireVersion`)

`version.json` separates three distinct version stamps so additive schema revisions do not break already-shipped binaries:

- **`minReader`** (`PRAGMA user_version`): the lowest schema revision a binary must know to open the database safely. Writers stamp `PRAGMA user_version` to `minReader` on fresh databases and never lower an existing database's `user_version`. Any SQLite client can read it with `PRAGMA user_version;`.
- **`version`** (schema revision): the latest applied schema revision, recorded as `MAX(schema_migrations.version)` (currently `8`). External tools that inspect the exact schema revision MUST read `MAX(schema_migrations.version)`, not `PRAGMA user_version`.
- **`wireVersion`**: the HTTP `/v1/health` `schemaVersion` contract stamp (currently `6`). Additive features (advertised via `Capability.features`) never raise `wireVersion`; only a breaking wire-protocol change bumps it.

### Additive revision rule

A schema revision is **additive** (and may keep `PRAGMA user_version` at `minReader`) **only if** every older writer's `INSERT`, `UPDATE`, and `DELETE` stays valid on every table, including new ones — for example, a new standalone table with no foreign-key, trigger, or `CHECK` coupling to existing tables. Any change that violates this rule MUST raise `minReader` (and `PRAGMA user_version`).

A trigger on an existing table is additive only when it cannot make an older writer's statement fail. Revision 8's `observation_meta` triggers qualify: they copy columns that are `NOT NULL` on `observations` into a table keyed by `rowid`, with `INSERT OR REPLACE`, so every valid write to `observations` still succeeds and keeps the copy current. `crates/kindling-store/tests/schema_revision_8.rs` pins this with 0.4.1-style writes.

## How to add a migration

1. Update `schema/schema.sql` so that it describes the schema **after** every migration has run, which is what a fresh database gets. Add a row to the `schema_migrations` seed. Keep `PRAGMA user_version = <minReader>;` unchanged for additive revisions, or bump `PRAGMA user_version` if the revision is non-additive.
2. Update `schema/version.json`: bump `"version"` to `<N>`. Bump `"minReader"` only if the revision is non-additive; bump `"wireVersion"` only on a breaking wire change; bump `"minCompatible"` on a breaking read change.
3. In `crates/kindling-store/src/db.rs`, add a forward migration (a `MIGRATION_<NNN>_<NAME>` SQL constant, run inside `BEGIN IMMEDIATE … COMMIT`) and apply it in `ensure_schema`. Keep it additive and idempotent (`CREATE … IF NOT EXISTS`, `INSERT OR IGNORE` for `schema_migrations`), and do not write `PRAGMA user_version` in an additive migration.
4. Run `scripts/sync-vendored-schema.sh` and `node packages/kindling/scripts/sync-types.mjs` and commit the vendored copies.
5. Add tests: cover migration idempotence (including racing opens), open rules for every `user_version` tier, and backward compatibility with older binaries.
6. If the change affects the wire types or the observation `kind` `CHECK` list, regenerate the bindings with `cargo test -p kindling-types --features ts-rs` and sync them with `node packages/kindling/scripts/sync-types.mjs`.

A schema bump is a public contract change. Open an issue first (see [CONTRIBUTING.md](../CONTRIBUTING.md#discuss-first)).

## Breaking changes

The following changes are **breaking** for consumers:

- **Column removal or rename** — the Rust crate queries columns by name.
- **Column type change** — e.g. `INTEGER` → `TEXT` for an existing column.
- **FTS tokenizer change** — different tokenizers produce different token streams, so search results will diverge. The current tokenizer is `porter unicode61` (Porter stemming + Unicode normalization). This is documented in `schema.sql` and pinned in `version.json` as `ftsTokenizer`.
- **CHECK constraint change** — e.g. adding/removing allowed `kind` values changes what rows are valid.

When making a breaking change, bump `minCompatible` (and `minReader` as appropriate) in `version.json` to the migration version that introduced the break. The Rust crate checks these bounds at startup and refuses to open a database it cannot safely read.

## How the Rust crate uses these files

1. At **build time**, `kindling-store` embeds its vendored `schema.sql` and `version.json` with `include_str!`.
2. When it **opens a database**:
   - A fresh database gets `schema.sql` applied (writable open only; read-only fails with `UninitializedDatabase`).
   - An existing database whose `PRAGMA user_version` is `0` or below `minCompatible` is refused (`SchemaTooOld`).
   - A database whose `PRAGMA user_version` exceeds `version` is refused (`SchemaTooNew`).
   - On writable open, `user_version = 5` runs migration 006 (raising `user_version` to 6), and any database missing `schema_migrations` row 7 runs migration 007, and any database missing row 8 runs migration 008, neither altering `user_version`.
   - On read-only open, no migration runs.
3. The daemon reports `wireVersion` as `schemaVersion` and `version` as `schemaRevision` in the `/v1/health` capability handshake. Clients validate `schemaVersion` against their supported wire range.
