# Kindling — Plan Index

| Field   | Value       |
| ------- | ----------- |
| Status  | In Progress |
| Owner   | @aneki      |
| Created | 2026-03-14  |
| Updated | 2026-06-18  |

## Problem

Kindling is functional (596 tests passing, 10 packages building) and the TypeScript packages are published to npm at v0.1.2. The remaining work is to port Kindling to Rust as the **only** implementation. Non-Rust consumers reach Kindling via a long-running local daemon (`kindling serve`) over a Unix domain socket, accessed by a thin TypeScript HTTP client distributed as `@eddacraft/kindling` on npm. The current TypeScript implementation packages are deprecated and removed after the cutover.

## Success Criteria

- [x] All packages published to npm under `@eddacraft` scope
- [ ] Single statically-linked `kindling` binary distributed via cargo, brew, curl|sh, and npm postinstall
- [ ] `kindling serve` daemon: auto-spawn on first call, idle shutdown after 30 min default, UDS transport (TCP fallback on Windows)
- [ ] All 7 Claude Code hook types complete in <10ms warm, <100ms cold
- [ ] Anvil emits observations directly via `kindling-client` or `kindling-service` — no TS bridge
- [ ] `pnpm add @eddacraft/kindling` installs the binary and exposes a typed thin client with no native deps
- [ ] All deprecated TS implementation packages removed from this repo at `1.0.0`

## Constraints

- Single-operator project — sole consumer is also the maintainer; no external migration coordination required
- Claude Code hook interface (stdin JSON, stdout JSON) must not change
- Existing 596 tests must continue to pass throughout, until the corresponding TS package is deprecated and removed
- `schema/schema.sql` and `schema/version.json` remain the cross-language schema contract; both implementations read from them during the transition

## Modules

| Module                                                                | Purpose                                                          | Status      | Dependencies       |
| --------------------------------------------------------------------- | ---------------------------------------------------------------- | ----------- | ------------------ |
| [01-npm-publish](./modules/01-npm-publish.aps.md)                     | Package metadata, READMEs, publish scripts, CI                   | Done        | —                  |
| [02-rust-hook-binary](./modules/02-rust-hook-binary.aps.md)           | Rust binary for Claude Code hook invocations                     | Superseded  | by 05              |
| [03-rust-cli](./modules/03-rust-cli.aps.md)                           | Full Rust CLI replacing Commander.js                             | Superseded  | by 05              |
| [04-intent-capture-events](./modules/04-intent-capture-events.aps.md) | Kindling-native intent event primitive + export                  | Done        | —                  |
| [04-schema-contract](./modules/04-schema-contract.aps.md)             | Cross-language SQLite schema contract for Rust+TS                | Done        | —                  |
| [05-rust-port](./modules/05-rust-port.aps.md)                         | Rust-canonical Kindling + thin TS client over local daemon (UDS) | In Progress | 04-schema-contract |

See `plans/specs/2026-05-03-rust-canonical-thin-client-design.md` for the current design (daemon, transport, distribution, TS deprecation strategy). The earlier dual-maintain spec at `plans/specs/2026-04-15-rust-port-design.md` is superseded but retained for historical context.

## Schedule

| Phase   | Modules                  | Target                                                                           |
| ------- | ------------------------ | -------------------------------------------------------------------------------- |
| Now     | 05-rust-port (Phase 1)   | Foundation crates: workspace, types, store, filter                               |
| Next    | 05-rust-port (Phase 2)   | Service + daemon + hook + Rust client; Anvil unblocks                            |
| Then    | 05-rust-port (Phase 3)   | CLI + umbrella binary + cross-platform builds + cargo/brew/curl distribution     |
| Then    | 05-rust-port (Phase 4)   | Thin TS client SDK on npm; deprecate TS implementation packages and Anvil bridge |
| Backlog | 04-intent-capture-events | Ship intent capture primitive + export (independent of the Rust port)            |

## Risks

| Risk                                                | Impact | Mitigation                                                                       |
| --------------------------------------------------- | ------ | -------------------------------------------------------------------------------- |
| Rust cross-compilation edge cases                   | Medium | `cargo-zigbuild` from a single Linux runner; CI matrix smoke-tests every target  |
| npm postinstall download fails behind corp proxy    | Medium | Honour `npm_config_proxy` and standard env vars; document offline binary install |
| Daemon process orphaned / stale PID files pile up   | Medium | PID file with stale-PID cleanup on next spawn; `kindling serve --health` for ops |
| Cold-spawn latency exceeds 100ms on slow disks      | Low    | Measure on dogfood; spool fallback only if measured as a real problem            |
| Schema drift between binary and client expectations | Medium | `/v1/health` reports schemaVersion; client checks on first call, fails loud      |

## Open Questions

- [x] Is `@eddacraft/kindling` npm scope claimable? — yes, published at v0.1.2
- [ ] Single per-user daemon vs. one daemon per project? (spec leans single per-user)
- [ ] Idle shutdown default — 30 min, or longer? (spec leans 30 min, re-tune after dogfooding)
- [ ] Wire format — JSON or MessagePack for v1? (spec leans JSON for debuggability)
- [ ] Hook spool fallback if daemon spawn fails — defer until measured? (spec defers)

## Decisions

- **D-001:** ~~Hybrid Rust approach (not full rewrite)~~ — _superseded by D-003_
- **D-002:** ~~Phase the Rust work (hooks first, CLI second)~~ — _superseded by D-003_
- **D-003:** ~~Dual-maintain Rust + TypeScript with Rust-canonical types~~ — _superseded by D-005_
- **D-004:** Supersede modules 02 and 03 with module 05 — _decided 2026-04-15_ — The hybrid phasing no longer models the work correctly. 02 and 03 remain in the repo for historical reference but are marked Superseded in this index.
- **D-005:** Rust-canonical Kindling with thin TS HTTP client over local daemon — _decided 2026-05-03_ — Rust becomes the only implementation. Non-Rust consumers reach Kindling via `kindling serve` (long-running per-user daemon) over a Unix domain socket. `@eddacraft/kindling` is repurposed as a thin HTTP client with an npm postinstall that downloads the platform binary. All other TS implementation packages are deprecated and removed after the cutover. Driven by: sole-operator project means no external migration coordination, every realistic TS consumer can hit a localhost daemon, dual-maintain pays a real tax for a use case nobody asked for. See `plans/specs/2026-05-03-rust-canonical-thin-client-design.md`.
