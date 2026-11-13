<!-- This document indexes active work; modules own item status. -->

# kindling — Plan Index

| Field   | Value       |
| ------- | ----------- |
| Status  | In Progress |
| Owner   | @aneki      |
| Created | 2026-03-14  |
| Updated | 2026-11-13  |

## Current truth

kindling's Rust-canonical engine, daemon, CLI, hooks, client and runtime facade
ship as binaries, a VSIX and crates. Latest published release is v0.4.1
(2026-11-10); eight publishable workspace crates are live at 0.4.1. The thin
TypeScript client is private source bundled into integrations. npm distribution
and the old TypeScript engine are retired by D-010.

[completed.aps.md](./completed.aps.md) indexes 44 completed items and the archived
task detail. Fourteen superseded items are historical, not completed deliveries.
Twenty-nine items remain active across five modules. See the
[full reconciliation](./reconciliation/2026-11-13-aps-cleanup.md) for all 87 dispositions.

## Success criteria

- [x] Rust engine, daemon, hooks and CLI included in published releases
- [x] Binary archives and VSIX attached to v0.4.0/v0.4.1; Rust crates published
- [x] Direct anvil daemon integration released in anvil v0.9.0-beta
- [x] Old TypeScript implementation source removed and npm packages deprecated
- [x] Public vision and contribution lanes live on main and included in v0.4.0
- [ ] Native-platform install/hook acceptance and editor smoke checks recorded
- [ ] Remaining release ops, demo embed and external-docs acceptance completed
- [ ] Deferred PORT-020 1.0.0 declaration and anvil bridge lifecycle resolved
- [ ] Intent-event contract restored or explicitly retired with migration guidance
- [ ] Curated roadmap, contributor opportunities and reviewed projection established

## Modules

| Module                                                                                        | Purpose                                                             | Status      | Active items | Delivered dependencies                                               |
| --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- | ----------- | ------------ | -------------------------------------------------------------------- |
| [05-rust-port](./modules/05-rust-port.aps.md)                                                 | Native/plugin acceptance and deferred lifecycle closeout            | In Progress | 4            | Schema, Rust engine/client and PORT-011 delivered                    |
| [06-downstream-integration-surface](./modules/06-downstream-integration-surface.aps.md)       | Complete adapter fixtures; design the next-minor event contract     | In Progress | 2            | KINTEG-001..006/008..014 delivered                                   |
| [07-intent-capture-events](./modules/07-intent-capture-events.aps.md)                         | Resolve current ownership/availability after TS retirement (GH-215) | Blocked     | 6            | Historical TS implementation removed; current Rust equivalent absent |
| [08-conversion-surface](./modules/08-conversion-surface.aps.md)                               | Release acceptance, CLI depth, distribution and docs                | In Progress | 12           | Rust runtime and C1/C3/C4 delivered                                  |
| [09-public-direction-and-contribution](./modules/09-public-direction-and-contribution.aps.md) | Roadmap, contributor opportunities and one-way projection           | In Progress | 5            | PUBDIR-001/002 live                                                  |

## Next decisions and acceptance

- PORT-010/014/015 and C2/C12 need native execution, fresh-install/plugin and
  Cursor/Windsurf receipts. macOS/Windows format checks are not native execution.
- C0 merged and released, but the demo recording/embed and external-docs
  publication checklist are not closed. Release workflow failures remain visible.
- KINTEG-007 ships Claude Code fixtures only; its full OpenCode/PocketFlow fixture
  outcome is outstanding. KINTEG-015 remains Draft for a future minor; do not
  advertise generic-event@1 or keyed retained append as available in 0.4.x.
- KINTENT-001..006 are Blocked on [GH-215](https://github.com/eddacraft/kindling/issues/215).
  Historical merges do not establish the feature in the supported Rust release.
- PORT-020 preserves the deferred 1.0.0 decision. Current deprecation notices are
  verified; optional notice repointing and the anvil bridge follow-up remain.
- PUBDIR-003..007 and C5..C11/C13/C14 remain Draft. Delivered dependencies do not
  automatically authorise their implementation.

## Archive locations

| Module or delivered subset        | Status                        | Items  | Detail                                                                                                |
| --------------------------------- | ----------------------------- | ------ | ----------------------------------------------------------------------------------------------------- |
| npm publishing                    | Complete (historical channel) | 6      | [01-npm-publish](./archive/modules/01-npm-publish.aps.md)                                             |
| Rust hook phase                   | Superseded                    | 8      | [02-rust-hook-binary](./archive/modules/02-rust-hook-binary.aps.md)                                   |
| Rust CLI phase                    | Superseded                    | 5      | [03-rust-cli](./archive/modules/03-rust-cli.aps.md)                                                   |
| Schema contract                   | Complete                      | 5      | [04-schema-contract](./archive/modules/04-schema-contract.aps.md)                                     |
| Rust port delivered subset        | Complete; PORT-018 Superseded | 15 + 1 | [05-rust-port](./archive/modules/05-rust-port.aps.md)                                                 |
| Downstream delivered subset       | Complete                      | 13     | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md)       |
| Conversion delivered subset       | Complete                      | 3      | [08-conversion-surface](./archive/modules/08-conversion-surface.aps.md)                               |
| Public direction delivered subset | Complete                      | 2      | [09-public-direction-and-contribution](./archive/modules/09-public-direction-and-contribution.aps.md) |

## Constraints and known risks

The Claude Code stdin/stdout hook contract, local-first storage and canonical
schema remain boundaries. Design and review records are dated rationale;
current modules and source own supported behaviour. Later defects such as
[GH-186](https://github.com/eddacraft/kindling/issues/186) remain open independently
of delivered-item archives. [issues.md](./issues.md) records planning discoveries.

The installed APS CLI is 0.9.0, while this project pins 0.3.0 and retains a
vendored CLI footprint. Non-strict lint is available; tooling migration is not
part of this plan-only cleanup. There is no active Ready item after reconciliation.

## Decisions

- **D-001:** ~~Hybrid Rust approach (not full rewrite)~~ — _superseded by D-003_
- **D-002:** ~~Phase the Rust work (hooks first, CLI second)~~ — _superseded by D-003_
- **D-003:** ~~Dual-maintain Rust + TypeScript with Rust-canonical types~~ — _superseded by D-005_
- **D-004:** Supersede modules 02 and 03 with module 05 — _decided 2026-04-15_ — The hybrid phasing no longer models the work correctly. 02 and 03 are archived for historical reference and marked Superseded; their old task statuses do not authorise execution.
- **D-006:** Triage anvil's integration wishlist (2026-06-22) into module 06 rather than scattering it across 05 — _decided 2026-06-22_ — anvil's downstream KDS module sent 10 asks; auditing them against the tree showed several already shipped (client+spool published at 0.1.0, `/v1/health` handshake, export `bundleVersion` + `--dry-run`) and one mis-framed (no `kindling-spool` crate exists — the spool is `kindling-client::spool`). The genuine new work (publish 0.2.0, daemon dedup, structured query API, capability handshake + kind registry, spool/cold-start observability, redaction evidence, fixtures) is grouped as module 06 so the contract anvil consumes evolves as one coherent surface. KINTEG-001 (publish) is user-gated and gates anvil's consumption of the rest.
- **D-008:** Add `kindling-runtime` as the anvil-first integration facade — _decided 2026-06-24_ — anvil is the primary Rust consumer; requiring a separate `kindling` binary on PATH plus manual `kindling-client` + `spool` + `kindling-server` wiring is the wrong default. A new facade crate composes in-process daemon spawn (pattern already proven in client tests), attach-or-start on the standard socket, and spool-on-by-default at the runtime layer. Does not replace the daemon model or merge the seven crates; CLI/npm adapters stay on the thin client. Tracked as KINTEG-008. See `plans/specs/2026-06-24-kindling-runtime-design.md`.
- **D-005:** Rust-canonical kindling with thin TS HTTP client over local daemon — _decided 2026-05-03_ — Rust becomes the only implementation. Non-Rust consumers reach kindling via `kindling serve` (long-running per-user daemon) over a Unix domain socket. `@eddacraft/kindling` is repurposed as a thin HTTP client ~~with an npm postinstall that downloads the platform binary~~ (_npm distribution superseded by D-007, then D-010_). All other TS implementation packages are deprecated and removed after the cutover. Driven by: sole-operator project means no external migration coordination, every realistic TS consumer can hit a localhost daemon, dual-maintain pays a real tax for a use case nobody asked for. See `plans/specs/2026-05-03-rust-canonical-thin-client-design.md`.
- **D-009:** Architecture for anvil's KDS-004/005 unblockers — a list/enumerate read API (KINTEG-003) and a spool retention cap (KINTEG-009) — _decided 2026-06-26_ — Resolved by a planning council (architect + delivery-lead + adversarial-reviewer). **Read API:** `POST /v1/observations/list` (POST+JSON like every data route), keyset cursor over the stable `(ts ASC, id ASC)` order (the only scheme complete under concurrent appends), half-open `[since, until)` bounds, `kinds` as a list, server-clamped `limit`, redacted excluded by default with a v1 `includeRedacted` flag, **no server-side aggregation** (anvil's `top`/`unused`/`flags`/`principals` are policy — kindling stays mechanism), **no schema bump** (read-only, no DDL; stays v5; old daemon → 404). anvil pins `until = now_ms()` at enumeration start as the snapshot boundary. **Spool cap:** `max_bytes`/`max_age_ms` on `SpoolConfig` (now `#[non_exhaustive]` + builder; client **0.2 → 0.3**), **defaulting to `None`/unbounded** (the council split here — non-None defaults silently trim existing callers on upgrade; 7d/64 MiB are anvil's policy, wired explicitly); add `spooled_at` to `SpoolEntry` for a real age basis; trim the **oldest contiguous prefix only**, **only inside the `flush()` lock**, accepting that trim is intentional bounded retention loss (not infinite retention) — preserving drain order and never dropping an un-drained entry ahead of a kept newer one. Two independent PRs, KINTEG-009 first. See `plans/specs/2026-06-26-kindling-read-api-and-spool-cap-design.md`.
- **D-007:** ~~npm ships the binary via per-platform `optionalDependencies`, not a postinstall download~~ — _decided 2026-06-22_ — _superseded by D-010_ — PR #104 (PORT-018). The esbuild/swc model (one `@eddacraft/kindling-<os>-<arch>[-musl]` package per host; the package manager auto-installs only the matching one) avoids postinstall network entirely, works under `--ignore-scripts`, and is lockfile-deterministic. The specifiers are injected at publish time rather than committed, so `pnpm install --frozen-lockfile` stays green before the platform packages exist on the registry. Supersedes the "npm postinstall that downloads the platform binary" mechanism noted in D-005; D-005's Rust-canonical direction is unchanged.
- **D-010:** Stop publishing to npm; distribute via Rust installs, crates.io and GitHub Release assets — _decided 2026-10-21_ — Distribution is `cargo install eddacraft-kindling`, the Homebrew tap, `install.sh`, the crates on crates.io, and GitHub Release assets (the 7 binaries and the VS Code / Cursor / Windsurf VSIX). The four `packages/*` are marked `private` and kept as source: the thin client is bundled into the VSIX and will be bundled into a future OpenCode plugin that the binary installs (CONV C13); the PocketFlow TS adapter stays in-repo. `publish.yml`, the per-platform `@eddacraft/kindling-<platform>` package machinery and the thin client's platform-package lookup are removed. The npm `latest` of `@eddacraft/kindling` and the OpenCode and PocketFlow adapters is still the retired 0.1.x TypeScript engine, so they are deprecated with a notice pointing at the binary install (`scripts/deprecate.sh npm-channel`, user-gated). Rust PocketFlow support is out of scope. Supersedes the npm distribution parts of D-005 and all of D-007; D-005's Rust-canonical direction stands. Operator decision; the `NPM_TOKEN` secret and the `npm` environment are no longer used.

## Historical maintenance

Repo-hygiene work tracked here for provenance (outside the module plan):

- **PR #106** (merged 2026-06-21) — dev-toolchain Dependabot sweep: cleared all 16 open alerts (vitest/vite/esbuild/rollup/postcss/minimatch/…) via targeted bumps + same-major `pnpm.overrides`. All dev-only; none ship in any published package (the thin client has no runtime deps). One `ajv` advisory (eslint-pinned, `$data` unused) accepted.
- **PR #107** — CodeQL `js/unnecessary-use-of-cat` (medium): the "no native modules" test read `package.json` by spawning `cat`; switched to `readFileSync`.
- **PR #108** — added `.github/secret_scanning.yml` ignoring `crates/kindling-service/tests/fixtures/**` so the redaction engine's synthetic test secrets stop tripping GitHub's generic (non-provider) secret scanning. True false positives — nothing to rotate.
- **v0.4.0 release prep** (2026-10-19) — bumps the Rust workspace and npm packages 0.3.0 → 0.4.0, adds the `CHANGELOG.md` section and `docs/releases/v0.4.0.md`. Found that no GitHub Release was ever published for v0.2.0 or v0.3.0, so npm `@eddacraft/kindling` `latest` is still the retired 0.1.2 TypeScript implementation and `install.sh` cannot resolve a latest release (ISS-003). Closed GitHub issues #137/#138/#139 (fixed by KINTEG-010/011/012) and #124 (PORT-011 landed).
- **PR #148** — CodeQL `rust/cleartext-logging` #11–#14: taint through `mask_secrets` / `mask_secrets_with_evidence` into `assert!` / CLI `println!`. All false positives after redaction (or used-in-tests). Disposition in `SECURITY.md`; CodeQL model pack at `.github/codeql/extensions/kindling-rust/`.
