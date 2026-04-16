# Kindling — Plan Index

| Field   | Value       |
| ------- | ----------- |
| Status  | In Progress |
| Owner   | @aneki      |
| Created | 2026-03-14  |
| Updated | 2026-04-15  |

## Problem

Kindling is functional (596 tests passing, 9 packages building) but not yet published or optimized for production use. The remaining work falls into three phases: get the TypeScript packages published to npm, ship the intent capture primitive, then port the production surface to Rust so Anvil can integrate directly without a TypeScript bridge.

## Success Criteria

- [ ] All packages published to npm under `@eddacraft` scope
- [ ] Claude Code plugin installable without Node.js/C++ toolchain
- [ ] Hook invocations complete in <10ms (currently ~50-90ms)
- [ ] Single-binary distribution for Linux, macOS, Windows

## Constraints

- Small team — work must be sequenced, not parallelized across milestones
- TypeScript adapters and browser store must remain (npm ecosystem consumers)
- Claude Code hook interface (stdin JSON, stdout JSON) must not change
- Existing 596 tests must continue to pass throughout

## Modules

| Module                                                                | Purpose                                                 | Status     | Dependencies           |
| --------------------------------------------------------------------- | ------------------------------------------------------- | ---------- | ---------------------- |
| [01-npm-publish](./modules/01-npm-publish.aps.md)                     | Package metadata, READMEs, publish scripts, CI          | Ready      | —                      |
| [02-rust-hook-binary](./modules/02-rust-hook-binary.aps.md)           | Rust binary for Claude Code hook invocations            | Superseded | by 05                  |
| [03-rust-cli](./modules/03-rust-cli.aps.md)                           | Full Rust CLI replacing Commander.js                    | Superseded | by 05                  |
| [04-intent-capture-events](./modules/04-intent-capture-events.aps.md) | Kindling-native intent event primitive + export         | Ready      | 01                     |
| [04-schema-contract](./modules/04-schema-contract.aps.md)             | Cross-language SQLite schema contract for Rust+TS       | Done       | —                      |
| [05-rust-port](./modules/05-rust-port.aps.md)                         | Dual-maintain Rust port; Rust-canonical types via ts-rs | Ready      | 01, 04-schema-contract |

See `plans/specs/2026-04-15-rust-port-design.md` for the rationale behind superseding 02 and 03 with 05.

## Schedule

| Phase | Modules                  | Target                                                           |
| ----- | ------------------------ | ---------------------------------------------------------------- |
| Next  | 01-npm-publish           | Merge open PRs, publish to npm                                   |
| Next  | 04-intent-capture-events | Ship intent capture primitive + export                           |
| Then  | 05-rust-port (Phase 1-2) | Foundation crates + hook binary; Anvil unblocks                  |
| Later | 05-rust-port (Phase 3-4) | CLI + server + distribution; TS packages consume generated types |

## Risks

| Risk                                        | Impact | Mitigation                                                 |
| ------------------------------------------- | ------ | ---------------------------------------------------------- |
| `@eddacraft/kindling` npm scope unavailable | High   | Check availability early, have fallback scope              |
| Rust cross-compilation edge cases           | Medium | Use `cross` or `cargo-zigbuild`, CI matrix for all targets |
| Two build systems (cargo + pnpm)            | Medium | Keep Rust binary self-contained, no circular deps          |
| TypeScript/Rust JSON schema drift           | Medium | Generate TS types from Rust structs via `ts-rs` crate      |

## Open Questions

- [ ] Is `@eddacraft/kindling` npm scope claimable?
- [ ] Should `kindling-types` include Anvil-specific observation kinds, or stay generic? (see spec, open question 1 — leaning generic)
- [ ] Does the Rust workspace live at `crates/` in this repo, or in a separate repo? (see spec, open question 2 — leaning `crates/` here)
- [ ] When does `kindling-store-sqljs` start consuming generated types — Phase 1 or Phase 4? (see spec, open question 3)

## Decisions

- **D-001:** ~~Hybrid Rust approach (not full rewrite)~~ — _superseded by D-003_
- **D-002:** ~~Phase the Rust work (hooks first, CLI second)~~ — _superseded by D-003_
- **D-003:** Dual-maintain Rust + TypeScript with Rust-canonical types — _decided 2026-04-15_ — Rust becomes the source of truth for domain types via `ts-rs`; TS packages continue shipping to npm as a thin projection. Driven by Anvil going nearly 100% Rust, which made the TS bridge the primary integration tax. See `plans/specs/2026-04-15-rust-port-design.md`.
- **D-004:** Supersede modules 02 and 03 with module 05 — _decided 2026-04-15_ — The hybrid phasing no longer models the work correctly. 02 and 03 remain in the repo for historical reference but are marked Superseded in this index.
