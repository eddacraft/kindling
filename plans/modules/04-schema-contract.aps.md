# Schema Contract

| ID     | Owner  | Status |
| ------ | ------ | ------ |
| SCHEMA | @aneki | Done   |

## Purpose

Extract the SQLite schema from the TypeScript migration files into a standalone, versioned contract that both the TypeScript store and the Rust crate can implement against. This prevents schema drift as the Rust implementation grows and makes the cross-language contract explicit and discoverable.

## Background

This work was requested by the anvil agents after reviewing what the Rust crate needs to be a compatible writer. The TypeScript store is correct and does not need to change — it just needs its schema documented externally so the Rust side can implement against a known contract.

## In Scope

- `schema/` directory at repo root containing raw SQL and version metadata
- `schema/schema.sql` — the canonical, human-readable schema (DDL only, no migration logic)
- `schema/version.json` — schema version number and compatibility metadata
- FTS5 tokenizer config documented explicitly (currently `tokenize='porter unicode61'`)
- `PRAGMA user_version` set to the current migration number so Rust can discover schema version with a single read
- Brief `schema/README.md` explaining the contract and how to update it

## Out of Scope

- Changes to TypeScript store logic or migration runner
- Publishing `kindling-schema` as a separate npm package (deferred until multiple independent consumers exist)
- Moving migration ownership to the schema directory (migrations stay in `kindling-store-sqlite`)
- Read path changes (FTS queries, capsule lookups stay TypeScript for now)

## Interfaces

**Depends on:**

- Current migration state (schema at migration 004)

**Exposes:**

- `schema/schema.sql` — cross-language implementation contract
- `schema/version.json` — machine-readable version for Rust crate compatibility checks
- `PRAGMA user_version` — runtime schema version, readable from any SQLite client

**Consumed by:**

- `02-rust-hook-binary` (HOOK-002: SQLite store layer) — Rust store implements against schema.sql
- `03-rust-cli` — Rust CLI reads and queries against the same schema

## Ready Checklist

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one task defined

## Tasks

### SCHEMA-001: Extract schema to schema/schema.sql

- **Intent:** Single source of truth for the SQLite DDL, extracted from migration files
- **Expected Outcome:** `schema/schema.sql` contains all `CREATE TABLE`, `CREATE INDEX`, and `CREATE VIRTUAL TABLE` statements reflecting the current schema at migration 005, with inline comments on non-obvious columns and constraints
- **Validation:** Running `schema.sql` against a fresh SQLite database produces an identical structure to a database that has run all 5 migrations
- **Status:** Done

### SCHEMA-002: Set PRAGMA user_version in migrations

- **Intent:** Make schema version discoverable via a single SQLite read from Rust or any other language
- **Expected Outcome:** Migration 005 sets `PRAGMA user_version = 5`; each future migration increments it; documented in `schema/README.md`
- **Validation:** `sqlite3 <db> 'PRAGMA user_version;'` returns `5` on any migrated database
- **Status:** Done
- **Dependencies:** SCHEMA-001

### SCHEMA-003: Document FTS5 tokenizer config

- **Intent:** Pin the FTS5 tokenizer as part of the contract so Rust-side search queries are compatible
- **Expected Outcome:** `schema/schema.sql` includes the `tokenize='porter unicode61'` config in the FTS virtual table definitions with a comment explaining why it must match exactly; `schema/README.md` calls this out as a breaking-change surface
- **Validation:** FTS table definitions in `schema.sql` match production migration output exactly
- **Status:** Done
- **Dependencies:** SCHEMA-001

### SCHEMA-004: Add schema/version.json

- **Intent:** Machine-readable version metadata for Rust crate compatibility checks at build time or startup
- **Expected Outcome:** `schema/version.json` contains `{ "version": 5, "minCompatible": 1, "ftsTokenizer": "porter unicode61" }`; Rust crate can parse this at startup to assert it was compiled against the right schema
- **Validation:** File is valid JSON; `version` matches `PRAGMA user_version` value from SCHEMA-002
- **Status:** Done
- **Dependencies:** SCHEMA-002, SCHEMA-003

### SCHEMA-005: Add schema/README.md

- **Intent:** Document the contract, how to update it, and what constitutes a breaking change
- **Expected Outcome:** `schema/README.md` explains: (1) what the schema directory is for, (2) how to update `schema.sql` when adding a migration, (3) what counts as a breaking change (column removal, type change, FTS tokenizer change), (4) `PRAGMA user_version` convention, (5) how the Rust crate uses these files
- **Validation:** A new contributor can read the README and know how to add a migration without breaking Rust compatibility
- **Status:** Done
- **Dependencies:** SCHEMA-001, SCHEMA-002, SCHEMA-003, SCHEMA-004
