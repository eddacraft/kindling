# Rust Hook Binary

| ID   | Owner  | Status     |
| ---- | ------ | ---------- |
| HOOK | @aneki | Superseded |

> **Superseded by [`05-rust-port`](./05-rust-port.aps.md) on 2026-04-15.**
> Hook work is absorbed into Phase 2 of module 05 (PORT-007..PORT-009).
> See `plans/specs/2026-04-15-rust-port-design.md` for rationale.
> This file stays as historical reference; do not pick up HOOK-\* tasks directly.

## Purpose

Replace the Node.js hook scripts with a single statically-linked Rust binary (`kindling-hook`) that handles all Claude Code hook invocations. This solves two problems: startup latency (~50-90ms → <10ms per hook invocation) and native compilation pain (better-sqlite3 requires platform-specific prebuilds or a C++ toolchain).

## In Scope

- Rust binary handling all 7 hook types (session-start, post-tool-use, post-tool-use-failure, user-prompt-submit, subagent-stop, pre-compact, stop)
- Embedded SQLite via rusqlite with `bundled` feature (static linking)
- JSON stdin/stdout interface matching existing hook contract
- Migration runner compatible with existing schema
- Content filtering (secret masking, truncation)
- Per-project database isolation (same path convention as Node.js: `~/.kindling/projects/<hash>/`)
- Cross-platform builds (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64)
- Install script (`curl | sh` pattern)

## Out of Scope

- CLI commands (module 03)
- HTTP API server (module 03)
- Retrieval/search logic (module 03)
- TypeScript adapter changes (hook interface is unchanged)
- Browser/WASM store

## Interfaces

**Depends on:**

- 01-npm-publish (packages published, stable schema)
- Existing SQLite schema and migration files

**Exposes:**

- `kindling-hook <subcommand>` binary (drop-in replacement for Node.js hook scripts)
- Same stdin JSON / stdout JSON contract
- Same database format (existing DBs work without migration)

## Ready Checklist

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one task defined

## Work Items

### HOOK-001: Scaffold Rust project

- **Intent:** Rust workspace initialized with dependencies
- **Expected Outcome:** `cargo build` succeeds with rusqlite (bundled), serde, serde_json, clap
- **Validation:** `cargo build --release` completes
- **Status:** Ready

### HOOK-002: SQLite store layer

- **Intent:** Rust equivalent of the TypeScript SQLite store
- **Expected Outcome:** Can open database, run migrations, insert observations, attach to capsules, create/close capsules
- **Validation:** `cargo test` passes store tests against a temp database
- **Status:** Ready
- **Dependencies:** HOOK-001

### HOOK-003: Content filtering

- **Intent:** Secret masking and content truncation matching Node.js behavior
- **Expected Outcome:** API keys, tokens, and passwords are redacted; content truncated at limits; excluded paths filtered
- **Validation:** `cargo test` passes filter tests with known secret patterns
- **Status:** Ready
- **Dependencies:** HOOK-001

### HOOK-004: Hook handlers

- **Intent:** All 7 hook types handled via stdin JSON
- **Expected Outcome:** Binary reads Claude Code hook context from stdin, performs the correct DB operation, returns JSON response
- **Validation:** Integration tests pipe JSON fixtures through the binary and verify DB state
- **Status:** Ready
- **Dependencies:** HOOK-002, HOOK-003

### HOOK-005: Context injection

- **Intent:** SessionStart and PreCompact hooks inject prior context
- **Expected Outcome:** SessionStart returns pins + recent observations; PreCompact returns pins + latest summary
- **Validation:** Integration test verifies injected context JSON structure
- **Status:** Ready
- **Dependencies:** HOOK-004

### HOOK-006: Cross-platform CI builds

- **Intent:** Binary builds for all target platforms
- **Expected Outcome:** GitHub Actions produces release binaries for Linux (x86_64, aarch64, musl), macOS (x86_64, aarch64), Windows (x86_64)
- **Validation:** CI matrix completes, artifacts downloadable
- **Status:** Draft
- **Dependencies:** HOOK-004

### HOOK-007: Install script and plugin update

- **Intent:** Users can install the binary and the plugin uses it
- **Expected Outcome:** `curl -sSL install.kindling.dev | sh` installs the binary; plugin `hooks.json` updated to use it
- **Validation:** Fresh install on Linux/macOS followed by Claude Code session with hooks firing
- **Status:** Draft
- **Dependencies:** HOOK-006

### HOOK-008: Compatibility validation

- **Intent:** Rust binary produces identical behavior to Node.js scripts
- **Expected Outcome:** Same hook inputs produce same DB state and JSON outputs as the Node.js implementation
- **Validation:** Snapshot tests comparing Node.js and Rust outputs for all hook types
- **Status:** Draft
- **Dependencies:** HOOK-004
