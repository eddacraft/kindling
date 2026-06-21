# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

kindling is a local memory and continuity engine for AI-assisted development. It captures observations (tool calls, diffs, commands, errors) from AI workflows, organizes them into capsules (bounded units of meaning), and makes context retrievable with deterministic, explainable results. All data is stored locally using embedded SQLite with FTS5.

**kindling is Rust-canonical.** The engine is a Cargo workspace in `crates/`; the binary is `kindling` (published as the `eddacraft-kindling` crate — the bare `kindling` name is taken on crates.io). The npm `@eddacraft/kindling` package is a thin HTTP-over-UDS client for that binary; the older TypeScript implementation packages were removed (they live on only as the already-published, deprecated 0.1.x npm versions).

## Commands

```bash
# --- Rust engine (canonical) ---
cargo build
cargo test
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test -p kindling-types --features ts-rs   # regenerate + check ts-rs bindings

# --- npm packages (thin client + adapters) ---
pnpm install
pnpm run build
pnpm run test
pnpm run type-check
pnpm run lint
```

## Architecture

### Crates (the engine, `crates/`)

```
eddacraft-kindling   → the `kindling` CLI binary + Claude Code hooks (lib + bin)
kindling-client      → daemon-backed Rust SDK (HTTP/1 over UDS); default for integrations
kindling-service     → in-process orchestration (embedded, zero-IPC)
kindling-server      → daemon runtime (`kindling serve`); per-project routing
kindling-store       → SQLite persistence (FTS5 + WAL); schema in `schema/`
kindling-provider    → deterministic local retrieval (FTS5 BM25 + recency)
kindling-types       → shared domain types (+ optional ts-rs bindings)
```

Dependency flow: `types ← store ← provider`; `service → types,store,provider`; `server → types,store,service`; `client → types`; the binary → all. Most integrations use `kindling-client` (daemon) or `kindling-service` (embedded). The daemon makes concurrent multi-tool access safe.

### npm packages (`packages/`)

```
@eddacraft/kindling                    → thin HTTP-over-UDS client for the Rust daemon (no native deps)
@eddacraft/kindling-adapter-opencode   → OpenCode session integration (over the thin client)
@eddacraft/kindling-adapter-pocketflow → PocketFlow workflow integration (over the thin client)
```

The adapters depend only on `@eddacraft/kindling`. Claude Code integration is built into the `kindling` binary (hooks), not a TS package.

### Domain Model

**Observations** are atomic units of captured context:

- `tool_call`, `command`, `file_diff`, `error`, `message`
- `node_start`, `node_output`, `node_error`, `node_end` (workflow events)

**Capsules** are bounded units that group observations:

- Types: `session`, `pocketflow_node`
- Lifecycle: open → close (with optional summary generation)
- Scope: `sessionId`, `repoId`, `agentId`, `userId`, `taskId` (`taskId` is carried for provenance and is not retrieval-filterable)

**Retrieval** is three-tiered:

1. Pins (user-controlled, non-evictable)
2. Current Summary (active session context)
3. Provider Hits (ranked FTS results)

### Key Abstractions

**KindlingService** (`kindling-service` crate) orchestrates the in-process engine:

- `open_capsule()`, `close_capsule()` - lifecycle management
- `append_observation()` - capture events (masks secrets at the service boundary)
- `retrieve()` - deterministic search with provenance
- `pin()`, `unpin()`, `forget()` - priority content + redaction

**Client** (`kindling-client` crate) is the daemon-backed SDK with the same surface over HTTP/1-over-UDS; it auto-spawns the daemon and re-exports the domain types.

**Store** (`kindling-store` crate): SQLite persistence with FTS5 + WAL. The schema is the cross-language contract in `schema/schema.sql` + `schema/version.json`.

**PocketFlow integration** (`@eddacraft/kindling-adapter-pocketflow`, npm): `KindlingNode`/`KindlingFlow` extend PocketFlow's Node/Flow, auto-create a `pocketflow_node` capsule per node, and record `node_start`/`node_output`/`node_error`/`node_end` — written through the `@eddacraft/kindling` thin client to the daemon.

### Code Patterns

Domain types live in `crates/kindling-types/src/` (`observation.rs`, `capsule.rs`, `retrieval.rs`, `common.rs`). They serialize as camelCase JSON; the optional `ts-rs` feature emits TypeScript bindings under `crates/kindling-types/bindings/` (CI fails on drift), which the thin TS client consumes.

The npm packages are ESM-only (`"type": "module"`) with `.js` extensions in imports.

## Branching Workflow

This repository uses a single permanent branch model with short-lived work
branches.

- `main` is the default branch, integration branch, and stable release branch.
- normal feat, fix, docs, and chore branches are created from `main`.
- hotfix branches are created from `main` or the active `release/*` branch.

Keep `main` as the only permanent worktree. Treat all other worktrees as
disposable and remove them once the branch is merged, replaced, or paused.

Release guidance:

- small releases may tag directly from `main` after release prep lands
- larger releases should use a short-lived `release/*` branch cut from `main`
- tagging `vX.Y.Z` on `main` and creating a GitHub Release triggers
  `.github/workflows/publish.yml` (publishes the npm packages) and
  `.github/workflows/release.yml` (uploads the prebuilt `kindling` binaries).
  The Rust crates are published to crates.io separately via `scripts/publish.sh`.

See the detailed guides for the full policy:

- `docs/guides/branching-strategy.md`
- `docs/guides/worktree-policy.md`
- `docs/guides/release-runbook.md`

Never push directly to `main` — always use pull requests.

## PocketFlow (Vendored)

The project vendors PocketFlow at `packages/kindling-adapter-pocketflow/vendor/pocketflow/`. Key concepts:

- **Node**: prep → exec → post lifecycle
- **Flow**: orchestrates nodes via action-based transitions
- **Shared Store**: global state accessible by all nodes
- **BatchNode/BatchFlow**: process arrays of items
- Design patterns: Agent, Workflow, RAG, MapReduce

The `.cursorrules` file in the vendor directory contains extensive PocketFlow guidance for agentic coding workflows.
