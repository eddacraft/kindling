# Rust-Canonical Kindling with Thin TS Client — Design Spec

| Field      | Value                                                       |
| ---------- | ----------------------------------------------------------- |
| Status     | Decided                                                     |
| Owner      | @aneki                                                      |
| Created    | 2026-05-03                                                  |
| Supersedes | `plans/specs/2026-04-15-rust-port-design.md` (dual-maintain) |
| APS Module | `plans/modules/05-rust-port.aps.md`                         |

## Context

The previous spec (2026-04-15) committed to dual-maintaining Rust and TypeScript implementations of Kindling, with Rust as the canonical type source via `ts-rs`. That assumed the TypeScript implementation had real consumers worth keeping.

Reassessment of the consumer set:

- **Anvil** is nearly 100% Rust. Native Rust integration is the goal, no TS bridge.
- **Other planned consumers** are TypeScript projects owned by the same operator. They need *access* to Kindling, not in-process embedding — an HTTP client over a local socket is sufficient for every realistic use case.
- **Browser / sql.js consumers** do not exist. The `@eddacraft/kindling-store-sqljs` package was speculative.
- **Adapter packages** (Claude Code, OpenCode, PocketFlow) are thin wrappers — they translate framework events into observation calls. They can wrap an HTTP client just as easily as an in-process service.

Dual-maintain pays a real tax (schema sync, type drift risk, two test suites, two CI matrices) for a use case nobody asked for. Collapse to one implementation.

## Decision

**Rust is the only implementation.** Non-Rust consumers reach Kindling via a long-running local daemon (`kindling serve`) over a Unix domain socket (Linux/macOS) or localhost TCP (Windows).

- One binary: `kindling`. Subcommands: `serve`, `hook`, plus the CLI verbs (`search`, `list`, `pin`, …).
- One npm package: `@eddacraft/kindling`. Postinstall downloads the platform binary; module exports a thin HTTP client.
- All existing TS implementation packages (`-store-sqlite`, `-store-sqljs`, `-provider-local`, `-server`, `-cli`, `-core`) are deprecated and removed from the npm registry after the cutover.

`ts-rs` stays in the picture, but only to generate `.d.ts` types for the thin TS client — not to keep two implementations aligned.

## Approaches Considered

| Approach                                                  | TS consumer story                          | Maintenance     | Distribution                                  |
| --------------------------------------------------------- | ------------------------------------------ | --------------- | --------------------------------------------- |
| **(A) Rust-canonical, thin TS HTTP client** ✅            | `pnpm add @eddacraft/kindling` → daemon spawned on first call | One implementation | Single binary + thin npm wrapper |
| (B) Dual-maintain Rust + TS (prior spec)                  | First-class TS implementation              | Two impls forever, ts-rs sync gate | Two artifacts, both first-class |
| (C) Rust-only, no TS surface                              | TS consumers write their own HTTP client   | Cleanest        | Binary only — every TS consumer rebuilds the wheel |

**Why A over C:** The thin client is ~50KB of TypeScript that auto-spawns the daemon and exposes a typed API. Shipping it once eliminates per-consumer boilerplate and gives the TS adapter packages something to wrap.

**Why A over B:** Dual-maintain only makes sense if the TS implementation has real consumers that can't tolerate IPC latency. None exist. Local UDS round-trip is sub-millisecond — invisible compared to the SQLite write itself.

## Runtime Model

### Daemon

- **Single per-user process.** Owns all SQLite databases under `~/.kindling/projects/<project-hash>/`.
- **WAL mode** SQLite, single writer per database. Daemon serialises writes; concurrent readers fan out.
- **Project routing** — every request carries `X-Kindling-Project: <hash>` (or `projectId` in body); daemon opens / caches one connection per project.
- **Idle shutdown** — daemon exits after N minutes with no in-flight requests. Default 30 min, configurable.

### Transport

- **Unix domain socket** at `~/.kindling/kindling.sock`, mode `0600` (filesystem permissions = authn).
- **TCP fallback** on `127.0.0.1` for Windows; opt-in elsewhere via flag.
- **HTTP/1** over the socket — debuggable with `curl --unix-socket`, no protocol code in clients.
- No TLS, no tokens — localhost-only, single user.

### API surface (v1)

```
GET    /v1/health                      → { version, schemaVersion, projects: [...] }
POST   /v1/capsules                    → open capsule
PATCH  /v1/capsules/:id/close          → close capsule
POST   /v1/observations                → append observation
POST   /v1/retrieve                    → ranked retrieval
POST   /v1/pins                        → pin
DELETE /v1/pins/:id                    → unpin
```

Request/response bodies are JSON shapes generated from `kindling-types` via `ts-rs`. Schema version returned in `/v1/health` lets clients fail loud on mismatch.

### Auto-spawn protocol (clients)

1. Client opens UDS connection at default path.
2. On `ECONNREFUSED` or missing socket → client `exec`s `kindling serve --daemonize`.
3. Client polls socket for up to 1s (10ms intervals).
4. Once socket appears → make request normally.
5. Subsequent calls hit the warm daemon (sub-ms).

Cold-spawn cost: ~50ms one-time per session. Acceptable for hooks (Claude Code's first hook fires on `SessionStart` which has slack); invisible for TS consumers (called from long-lived processes).

### Hook special case

Claude Code hooks run as subprocesses. They become thin clients:

- `kindling hook session-start` reads context from stdin, makes one HTTP-over-UDS call, writes response to stdout, exits.
- Binary itself starts in microseconds. All work is in the daemon.
- Cold session: first hook spawns daemon (~50ms). Warm: <5ms total.

### Anvil integration

Two equally valid integration paths; per-call site choice:

- **In-process:** `use kindling_service::KindlingService;` — zero IPC. Use when Anvil owns the database lock for the session (headless, single-process workflows).
- **Via daemon:** `use kindling_client::Client;` — same shape. Use when Claude Code (or any other tool) might write concurrently.

Default Anvil to the daemon path for any session that could overlap with interactive Claude Code.

## Distribution Model

One binary, multiple install channels, all converge on `~/.local/bin/kindling` (or platform equivalent):

| Channel | Audience | Command |
| --- | --- | --- |
| crates.io | Rust devs | `cargo install kindling` |
| Homebrew tap | macOS/Linux devs | `brew install eddacraft/tap/kindling` |
| Install script | Anyone, CI | `curl -sSL install.kindling.dev \| sh` |
| GitHub Releases | Manual / scripted | Raw binaries per platform |
| **npm postinstall** | TS projects | `pnpm add @eddacraft/kindling` |

The npm postinstall is the unlock for TS consumers: one `pnpm add` installs the client SDK *and* downloads the platform binary into `node_modules/@eddacraft/kindling/bin/`. Same model as `esbuild`, `swc`, `biome`. If the binary is already on PATH, the postinstall is a no-op.

Cross-platform release matrix:

- Linux x86_64 (gnu + musl)
- Linux aarch64 (gnu + musl)
- macOS x86_64
- macOS aarch64
- Windows x86_64

Built via `cargo-zigbuild` on a single Linux runner (or matrix per OS, decide in PORT-010).

## TS Surface Strategy

### Single npm package

`@eddacraft/kindling` (the existing primary package, repurposed):

- **Removes** all current implementation code (better-sqlite3, FTS, server, CLI).
- **Adds** thin HTTP-over-UDS client + auto-spawn logic.
- **Adds** types generated from Rust via `ts-rs`.
- **Adds** postinstall script that downloads the platform binary if not on PATH.

Public API stays roughly the same shape (`KindlingService` → `Kindling` client class with the same method names) so consumer migration is a few imports + a `new Kindling()` instead of `new KindlingService({ store, provider })`.

### Deprecated packages

These get a `0.x.0` deprecation release with a `console.warn` on import, then removal at next major:

- `@eddacraft/kindling-core`
- `@eddacraft/kindling-store-sqlite`
- `@eddacraft/kindling-store-sqljs`
- `@eddacraft/kindling-provider-local`
- `@eddacraft/kindling-server`
- `@eddacraft/kindling-cli`

Adapters (`@eddacraft/kindling-adapter-claude-code`, `-opencode`, `-pocketflow`) get rewritten to depend on `@eddacraft/kindling` (the new thin client).

The Anvil TS bridge (`@eddacraft/anvil-kindling-integration`) gets deprecated and removed once Anvil cuts over to direct Rust integration.

### Browser / WASM

Out of scope. The `kindling-store-sqljs` package goes away. If a real browser consumer appears later, revisit by exposing the daemon API over a different transport (WebSocket, BroadcastChannel + SharedWorker, or recompile the daemon to WASM and run it in a SharedWorker).

## Crate Layout

10 crates in `crates/`:

| Crate                  | Purpose |
| ---------------------- | ------- |
| `kindling-types`       | Domain types, `ts-rs` derives |
| `kindling-store`       | SQLite persistence (rusqlite + bundled FTS5) |
| `kindling-filter`      | Secret masking, truncation |
| `kindling-provider`    | Local FTS retrieval, BM25 normalisation |
| `kindling-service`     | In-process API (`open_capsule`, `append_observation`, `retrieve`, …) |
| `kindling-server`      | UDS daemon (axum), auto-spawn, idle shutdown |
| `kindling-client`      | Rust HTTP-over-UDS client (used by hook, CLI, Anvil) |
| `kindling-hook`        | Thin hook subcommand wrapping `kindling-client` |
| `kindling-cli`         | clap subcommands wrapping `kindling-service` or `-client` |
| `kindling`             | Umbrella binary — dispatches to hook / CLI / server |

The workspace root `Cargo.toml` lives at the repo root with `members = ["crates/*"]`; member crates live under `crates/`. Existing `packages/` directory shrinks to: `@eddacraft/kindling` (thin client) + adapter packages, all of which become consumers of the daemon.

## Migration Path

1. Land Rust foundation (Phase 1) without touching `packages/`.
2. Land daemon + hook + Anvil integration (Phase 2). At this point Anvil is unblocked and Claude Code plugin can switch to the Rust hook.
3. Land CLI + distribution (Phase 3). `kindling` binary on cargo, brew, curl|sh.
4. Rewrite `@eddacraft/kindling` as the thin client + postinstall package (Phase 4). Ship as `0.2.0`.
5. Deprecate the old TS implementation packages (Phase 4). Final removal at `1.0.0`.
6. Anvil TS bridge deprecation (Phase 4, after Anvil's cutover lands in EddaCraft repo).

## Open Questions

1. **Single daemon for all projects, or one daemon per project?** Lean single per-user daemon — simpler, cheaper, avoids spawn storms.
2. **Idle shutdown default — 30 min or longer?** Lean 30 min. Re-tune after dogfooding.
3. **Wire format — JSON or MessagePack?** Lean JSON for v1 (debuggable). Reconsider only if benchmarks justify it.
4. **Hook spool fallback** — if the daemon fails to spawn within timeout, should the hook write to a small spool file the daemon drains on next start? Edge case; defer until measured as a problem.
5. **Server-side filtering vs client-side filtering** — daemon should own secret filtering so consumers can't accidentally bypass it. Confirmed.
6. **Postinstall behaviour offline** — if the binary download fails, should `pnpm add` fail or warn-and-defer? Lean warn-and-defer: client throws on first call with a clear error.
7. **OIDC / multi-user case** — out of scope for v1 (localhost, single user). Revisit when a real shared-host use case appears.

## Risks

| Risk                                                  | Impact | Mitigation                                                   |
| ----------------------------------------------------- | ------ | ------------------------------------------------------------ |
| npm postinstall download fails behind corp proxy      | Medium | Honour `npm_config_proxy` / standard env vars; document offline binary install path |
| Daemon process gets orphaned / piles up               | Medium | PID file with stale-PID cleanup on next spawn; `kindling serve --health` for ops |
| Cold-spawn latency exceeds 100ms on slow disks        | Low    | Measure on dogfood; spool fallback if needed (open question 4) |
| Existing TS consumers (just you) need migration       | Low    | You own them; coordinate cutover with `0.2.0` release        |
| Schema drift between binary and client expectations   | Medium | `/v1/health` reports schemaVersion; client checks on first call and fails loud |

## Success Criteria

- [ ] `kindling serve` runs as a long-lived daemon with auto-spawn and idle shutdown
- [ ] All 7 Claude Code hook types complete in <10ms warm, <100ms cold
- [ ] Anvil emits observations directly via `kindling-client` or `kindling-service` — no TS bridge
- [ ] `pnpm add @eddacraft/kindling` installs the binary and exposes a typed client
- [ ] Single statically-linked binary distributed via cargo, brew, curl|sh, npm
- [ ] All deprecated TS implementation packages removed by `1.0.0`
