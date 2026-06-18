# Rust CLI

| ID  | Owner  | Status     |
| --- | ------ | ---------- |
| CLI | @aneki | Superseded |

> **Superseded by [`05-rust-port`](./05-rust-port.aps.md) on 2026-04-15.**
> CLI and server work is absorbed into Phase 3 of module 05 (PORT-011..PORT-014);
> type sync moves to Phase 4 (PORT-015..PORT-016).
> See `plans/specs/2026-04-15-rust-port-design.md` for rationale.
> This file stays as historical reference; do not pick up CLI-\* tasks directly.

## Purpose

Extend the Rust binary from a hook-only tool to a full CLI replacing the TypeScript Commander.js CLI. This consolidates all kindling operations into a single binary: hooks, CLI commands, and the HTTP API server. Users get `kindling` as a single download with no runtime dependencies.

## In Scope

- All 12 CLI commands (init, log, capsule open/close, status, search, list, pin, unpin, export, import, serve)
- FTS5 retrieval with BM25 normalization (port from kindling-provider-local)
- HTTP API server (port from Fastify to axum)
- Sync subcommands (init, add-submodule, push)
- JSON and text output modes
- `cargo install kindling` distribution
- Homebrew tap

## Out of Scope

- TypeScript core types (remain for npm consumers)
- Adapter logic (remains TypeScript)
- Browser WASM store (remains sql.js)
- Semantic search / embeddings (future work)

## Interfaces

**Depends on:**

- 02-rust-hook-binary (Rust workspace, SQLite store layer, content filtering)

**Exposes:**

- `kindling <command>` binary with all CLI functionality
- HTTP API at `127.0.0.1:8080` (same endpoints as Fastify server)
- `cargo install kindling` and Homebrew tap

## Ready Checklist

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [ ] Tasks need refinement after module 02 completes

## Tasks

### CLI-001: FTS retrieval provider

- **Intent:** Port the local FTS provider to Rust
- **Expected Outcome:** FTS5 search with BM25 normalization, tiered retrieval (pins, summary, candidates)
- **Validation:** `cargo test` passes retrieval tests matching TypeScript behavior
- **Status:** Draft

### CLI-002: CLI commands

- **Intent:** All 12 commands available via clap
- **Expected Outcome:** `kindling status`, `kindling search`, `kindling list`, etc. all work with JSON and text output
- **Validation:** Integration tests for each command
- **Status:** Draft

### CLI-003: HTTP API server

- **Intent:** Replace Fastify with axum in the same binary
- **Expected Outcome:** `kindling serve` starts HTTP API with same endpoints
- **Validation:** Existing API integration tests pass against the Rust server
- **Status:** Draft

### CLI-004: Distribution

- **Intent:** Multiple install methods available
- **Expected Outcome:** `cargo install kindling`, Homebrew tap, and GitHub release binaries
- **Validation:** Install via each method on a clean machine
- **Status:** Draft

### CLI-005: TypeScript type sync

- **Intent:** Rust struct changes propagate to TypeScript types
- **Expected Outcome:** `ts-rs` generates TypeScript type definitions from Rust structs
- **Validation:** Generated types match existing `kindling-core` types
- **Status:** Draft
