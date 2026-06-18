# Rust Port — Design Spec

| Field         | Value                                                         |
| ------------- | ------------------------------------------------------------- |
| Status        | Superseded                                                    |
| Owner         | @aneki                                                        |
| Created       | 2026-04-15                                                    |
| Superseded    | 2026-05-03                                                    |
| Superseded by | `plans/specs/2026-05-03-rust-canonical-thin-client-design.md` |
| Supersedes    | `02-rust-hook-binary`, `03-rust-cli`                          |
| APS Module    | `plans/modules/05-rust-port.aps.md`                           |

> **Note (2026-05-03):** This spec proposed _dual-maintain_ Rust + TS with Rust-canonical types via `ts-rs`. After review, the plan changed: Rust becomes the **only** implementation, accessed by non-Rust consumers via a long-running local daemon (`kindling serve`) over a Unix domain socket. The TypeScript surface collapses to a single thin HTTP client package. See the superseding spec for the daemon model, transport, distribution, and TS package deprecation strategy.

## Context

kindling's existing Rust plan (modules 02 and 03) was a **hybrid** approach: move the data plane (hooks, CLI, server) to Rust while keeping TypeScript for domain types, adapters, and the npm surface. It was sized around one assumption — **hooks are the bottleneck** — because at the time anvil was a mixed TS/Rust system and kindling was consumed from both.

That calculus has changed. anvil is now **nearly 100% Rust** (11+ Rust crates). The only seam between anvil and kindling is a TypeScript bridge package, `@eddacraft/anvil-kindling-integration`, which:

- Maps anvil's 11 observation kinds down to kindling's 3 generic kinds (`message`, `command`, `error`)
- Writes via the TypeScript SQLite store in-process
- Forces anvil's Rust crates to bounce through Node.js to emit observations

That bridge is no longer an implementation detail — it is the **primary integration tax** on the production path. Replacing it is the driver for this rework.

## Decision

**Go with dual-maintain Rust + TypeScript, Rust-canonical.**

- **Rust kindling** — first-class for eddacraft (anvil integration, hooks, CLI, server). Production path.
- **TS kindling** — first-class for OSS consumers (npm, browser/WASM, adapter authors). Community path.
- **Rust is the source of truth** for domain types. `ts-rs` generates TypeScript type definitions. kindling's TS packages evolve to consume generated types.

This **replaces** modules 02 and 03 with a single umbrella plan (module 05) that covers all four phases end-to-end.

## Approaches Considered

| Approach                        | anvil compatibility                                    | Maintenance                                       | Distribution                     |
| ------------------------------- | ------------------------------------------------------ | ------------------------------------------------- | -------------------------------- |
| **(A) Full Rust rewrite**       | Native `use kindling::*` from anvil crates. No bridge. | One language. Lose npm consumers + browser store. | Single binary, no Node.js needed |
| **(B) Dual Rust + TS** ✅       | Native Rust for anvil, TS for npm/browser consumers    | Double surface, type drift risk                   | Two artifacts, both first-class  |
| **(C) Existing hybrid (02/03)** | Still need the TS bridge for anvil integration         | Moderate, but bridge stays awkward                | Mixed                            |

**Why (B) and not (A):** kindling is an open-source project. Most of the potential userbase sits in the npm/TypeScript ecosystem — adapter authors, browser/WASM consumers, developers who don't want a Rust toolchain. Walking away from that narrows the OSS audience significantly. The cost of maintaining the TS surface is bounded because Rust becomes the source of truth for types and the TS packages become mostly a thin projection.

**Why (B) and not (C):** The existing hybrid keeps the TS bridge awkward forever. anvil doesn't get to drop its Node.js dependency for observation capture without this rework.

## Design

### Rust Crate Structure

```
crates/
  kindling-types/       — Domain types (Observation, Capsule, Retrieval, ScopeIds) with ts-rs derives
  kindling-store/       — SQLite persistence (rusqlite, bundled, FTS5, WAL) against schema/schema.sql
  kindling-provider/    — Local FTS retrieval (BM25, tiered: pins → summary → candidates)
  kindling-service/     — Orchestration (open/close capsule, append, retrieve, pin)
  kindling-filter/      — Content filtering (secret masking, truncation)
  kindling-hook/        — Claude Code hook handlers (stdin/stdout JSON)
  kindling-server/      — HTTP API (axum, same endpoints as Fastify)
  kindling-cli/         — CLI commands (clap, all 12 commands)
  kindling/             — Umbrella crate (re-exports, binary entry point)
```

### Component Mapping

| Component      | TS Package                          | Rust Crate          | Notes                                      |
| -------------- | ----------------------------------- | ------------------- | ------------------------------------------ |
| Domain types   | `kindling-core`                     | `kindling-types`    | Source of truth; `ts-rs` exports `.d.ts`   |
| SQLite store   | `kindling-store-sqlite`             | `kindling-store`    | `rusqlite` with bundled SQLite; schema.sql |
| FTS provider   | `kindling-provider-local`           | `kindling-provider` | BM25 normalization port                    |
| Service layer  | `kindling-core` (`KindlingService`) | `kindling-service`  | Full orchestration                         |
| Content filter | (inline in adapter)                 | `kindling-filter`   | Secret masking, truncation                 |
| Hook handlers  | `kindling-adapter-claude-code`      | `kindling-hook`     | Supersedes module 02                       |
| HTTP server    | `kindling-server`                   | `kindling-server`   | `axum` replaces Fastify                    |
| CLI            | `kindling-cli`                      | `kindling-cli`      | `clap` replaces Commander.js               |

### What Stays TypeScript

- `@eddacraft/kindling-core` — becomes a thin orchestration wrapper over generated types
- `@eddacraft/kindling-store-sqljs` — browser/WASM store (no Rust equivalent needed; runs in-browser)
- Adapter packages — OpenCode, PocketFlow (TS consumers of the service API)
- npm distribution for OSS users

### anvil Integration — End State

```rust
// anvil crates — direct dependency, no bridge
use kindling_service::KindlingService;
use kindling_types::{Observation, ObservationKind};

let svc = KindlingService::new(config)?;
svc.append_observation(observation)?;
```

The `@eddacraft/anvil-kindling-integration` TypeScript bridge is **deprecated after Phase 2** and removed after anvil cuts over.

## Phasing

| Phase | Crates                     | Deliverable                                                   |
| ----- | -------------------------- | ------------------------------------------------------------- |
| **1** | types, store, filter       | Core data model + persistence. Enables everything downstream. |
| **2** | provider, service, hook    | anvil can `use kindling_service`. Hook binary ships.          |
| **3** | cli, server, umbrella      | Full `kindling` binary. Single download, zero deps.           |
| **4** | ts-rs + TS package updates | TS packages consume generated types. anvil drops TS bridge.   |

anvil unblocks at the end of Phase 2 (the TS bridge can start being removed). Hook latency wins ship at the end of Phase 2. Distribution wins ship at the end of Phase 3. The OSS type-drift risk is closed in Phase 4.

## Risks

| Risk                                        | Impact | Mitigation                                                                                                       |
| ------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------- |
| Type drift between Rust and TS              | Medium | `ts-rs` in CI; fail PR if generated `.d.ts` diverges from committed TS types                                     |
| Schema contract drift                       | Medium | Module 04 (`schema-contract`) already pins this; both sides implement against `schema/schema.sql`                |
| Rust cross-compilation matrix pain          | Medium | `cross` or `cargo-zigbuild`; CI matrix for Linux (x86_64/aarch64/musl), macOS (x86_64/aarch64), Windows (x86_64) |
| Two build systems (cargo + pnpm) coexisting | Low    | Keep crates self-contained; TS packages depend only on generated types                                           |
| Browser store diverges from canonical types | Low    | `kindling-store-sqljs` consumes the same generated types; schema.sql is shared                                   |
| Double maintenance burden ("two Kindlings") | Medium | Rust carries the logic; TS becomes a thin projection after Phase 4                                               |

## Trade-offs

**Why dual, not single:** Committing to Rust-only means losing the browser/WASM store (sql.js runs in the browser; rusqlite does not) and making the npm surface second-class. Committing to TS-only means anvil keeps paying the bridge tax forever.

**Why Rust-canonical, not shared IDL:** A neutral IDL (JSON Schema, Protobuf) is defensible but adds a third build step for questionable gain when Rust is already the production source of truth. Revisit if a third implementation appears.

**Why supersede 02/03 instead of keeping them:** The hybrid phasing (hooks first, CLI second, TS stays canonical) doesn't survive the direction change. Keeping the superseded plans around as sub-modules would require pretending HOOK-_ and CLI-_ tasks still model the work correctly — they don't. Archive them, reference them from the new plan for anyone reading history.

## Open Questions

1. **Should `kindling-types` include the anvil-specific observation kinds?** The TS bridge today maps anvil's 11 kinds down to kindling's 3 generic kinds (`message`, `command`, `error`). Options:
   - **(a) kindling stays generic** — anvil's mapping logic moves to anvil, now in Rust. kindling remains a generic capture/retrieval primitive.
   - **(b) kindling becomes anvil-aware** — add the 11 kinds to `ObservationKind`. Cleaner for anvil, but bleeds anvil semantics into an OSS project.
   - **Leaning (a)** — keep kindling generic. anvil owns its mapping. Revisit if a second Rust consumer appears that wants richer kinds.

2. **Build system integration.** Where does `cargo` live in the monorepo? Options:
   - **(a)** New `crates/` directory at repo root, alongside `packages/`
   - **(b)** Separate repo for the Rust workspace
   - **Leaning (a)** — one repo, two build systems, fewer coordination seams. TS packages in `packages/`, Rust crates in `crates/`. CI runs both.

3. **When does `kindling-store-sqljs` consume generated types?** The browser store still needs to round-trip the same shapes. Phase 4 default, but it could be Phase 1 if we want to validate the `ts-rs` pipeline early.

## Supersedes

- `plans/modules/02-rust-hook-binary.aps.md` — Hook work absorbed into Phase 2 of module 05
- `plans/modules/03-rust-cli.aps.md` — CLI work absorbed into Phase 3 of module 05

Both modules should be marked `Superseded` in the index; their files stay in the repo as historical reference.

## Related

- `schema/schema.sql`, `schema/version.json` — cross-language schema contract (module 04-schema-contract, already Done)
- `docs/system-spec.md` — moved to eddacraft; system context lives there now
- [anvil repo] `@eddacraft/anvil-kindling-integration` — TS bridge this plan removes
