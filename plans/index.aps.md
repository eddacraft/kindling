# kindling — Plan Index

| Field   | Value                                                                                                  |
| ------- | ------------------------------------------------------------------------------------------------------ |
| Status  | In Progress                                                                                            |
| Owner   | @aneki                                                                                                 |
| Created | 2026-03-14                                                                                             |
| Updated | 2026-10-21 (D-010: npm publishing stopped; conversion wave 1 merged; PUBDIR-001/002 merged as PR #151) |

## Problem

kindling's Rust-canonical engine, daemon, hook binary, CLI, client, runtime facade,
and thin TypeScript client are implemented. Non-Rust consumers reach the daemon
through the `@eddacraft/kindling` thin client, which is now an in-repo source
package bundled into integrations rather than an npm package (D-010); the
deprecated TypeScript implementation source has been removed. Remaining work is
lifecycle closeout for the retired npm packages, post-v0.3 integration changes,
and conversion-surface depth.

## Success Criteria

- [x] ~~All packages published to npm under `@eddacraft` scope~~ (true for 0.1.x) — _superseded by D-010_
- [x] Single statically-linked `kindling` binary distributed via cargo (`cargo install eddacraft-kindling` @ 0.2.0 on crates.io); brew and curl|sh validation remain user-gated (npm is no longer a channel, D-010)
- [x] `kindling serve` daemon: auto-spawn on first call, idle shutdown after 30 min default, UDS transport (authenticated TCP fallback on Windows)
- [x] All 7 Claude Code hook types complete in <10ms warm, <100ms cold
- [x] anvil emits observations directly via `kindling-client` — no TS bridge (`command.invoked` via `KindlingDaemonSink`, anvil PR #2897; opt-in `ANVIL_KINDLING_SINK=daemon`, default `ndjson` unchanged)
- [ ] ~~`pnpm add @eddacraft/kindling` installs the binary and exposes a typed thin client with no native deps~~ — _superseded by D-010_ (never published; the typed thin client exists in-repo and is bundled into the VSIX)
- [ ] The retired npm packages (`@eddacraft/kindling`, `-adapter-opencode`, `-adapter-pocketflow`) carry a deprecation notice pointing at the binary install (`scripts/deprecate.sh npm-channel`; user-gated)
- [x] All deprecated TS implementation packages removed from this repo (source dirs deleted; `npm deprecate` verified 2026-10-19; formal `1.0.0` tag remains user-gated)
- [ ] Public north star, contribution acceptance lanes, curated roadmap, and contributor-ready issue surface are established without making GitHub a second planning authority

## Constraints

- Single-operator project — sole consumer is also the maintainer; no external migration coordination required
- Claude Code hook interface (stdin JSON, stdout JSON) must not change
- Existing 596 tests must continue to pass throughout, until the corresponding TS package is deprecated and removed
- `schema/schema.sql` and `schema/version.json` remain the cross-language schema contract; both implementations read from them during the transition

## Modules

| Module                                                                                        | Purpose                                                                                                                                | Status      | Dependencies       |
| --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ----------- | ------------------ |
| [01-npm-publish](./modules/01-npm-publish.aps.md)                                             | Package metadata, READMEs, publish scripts, CI                                                                                         | Done        | —                  |
| [02-rust-hook-binary](./modules/02-rust-hook-binary.aps.md)                                   | Rust binary for Claude Code hook invocations                                                                                           | Superseded  | by 05              |
| [03-rust-cli](./modules/03-rust-cli.aps.md)                                                   | Full Rust CLI replacing Commander.js                                                                                                   | Superseded  | by 05              |
| [04-schema-contract](./modules/04-schema-contract.aps.md)                                     | Cross-language SQLite schema contract for Rust+TS                                                                                      | Done        | —                  |
| [05-rust-port](./modules/05-rust-port.aps.md)                                                 | Rust-canonical kindling + thin TS client over local daemon (UDS)                                                                       | In Progress | 04-schema-contract |
| [06-downstream-integration-surface](./modules/06-downstream-integration-surface.aps.md)       | Harden the daemon/client contract anvil consumes (publish, dedup, query, handshake, observability, redaction evidence, runtime facade) | In Progress | 05-rust-port       |
| [07-intent-capture-events](./modules/07-intent-capture-events.aps.md)                         | kindling-native intent event primitive + export (independent of the Rust port)                                                         | Done        | —                  |
| [08-conversion-surface](./modules/08-conversion-surface.aps.md)                               | Public first impressions: merge built work, release ops, CLI depth, adapter/docs distribution                                          | In Progress | 05-rust-port       |
| [09-public-direction-and-contribution](./modules/09-public-direction-and-contribution.aps.md) | Public north star, contribution boundaries, curated roadmap, contributor opportunities, and one-way APS-to-Issues projection           | In Progress | —                  |

See `plans/specs/2026-05-03-rust-canonical-thin-client-design.md` for the current design (daemon, transport, distribution, TS deprecation strategy). The earlier dual-maintain spec at `plans/specs/2026-04-15-rust-port-design.md` is superseded but retained for historical context.

## Schedule

| Phase | Modules                              | Target                                                                                                                                                                                     |
| ----- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Now   | 05-rust-port                         | Close PORT-020: retired-package deprecation verified (2026-10-19); remaining: deprecate the thin client and adapters on npm (D-010), the formal `1.0.0` tag, the anvil TS-bridge follow-up |
| Now   | 06-downstream-integration-surface    | KINTEG-013/014 merged; v0.4.0 release prep in review, then publish the GitHub Release (binaries + VSIX) and crates.io                                                                      |
| Now   | 08-conversion-surface                | Wave 1 (C1–C4) merged; C12 still blocked on publishing the v0.4.0 Release (ISS-003); C13 (bundled OpenCode plugin) drafted after D-010                                                     |
| Next  | 09-public-direction-and-contribution | Vision and lanes merged (#151); next publish the curated roadmap and a small contributor-opportunity set before automating sync                                                            |
| Done  | 05-rust-port (implementation)        | Rust engine, daemon, hooks, CLI, distribution scaffolding, thin client, adapters, and anvil KDS integration                                                                                |
| Done  | 06-downstream-integration-surface    | KINTEG-001..012 implementation merged; release and registry publication remain lifecycle work                                                                                              |
| Done  | 07-intent-capture-events             | Intent capture primitive + export shipped (independent of the Rust port; KINTENT-001..006 merged)                                                                                          |

## Risks

| Risk                                                 | Impact | Mitigation                                                                       |
| ---------------------------------------------------- | ------ | -------------------------------------------------------------------------------- |
| Rust cross-compilation edge cases                    | Medium | `cargo-zigbuild` from a single Linux runner; CI matrix smoke-tests every target  |
| ~~npm postinstall download fails behind corp proxy~~ | n/a    | Retired: no npm channel (D-010); binaries come from GitHub Releases or cargo     |
| Daemon process orphaned / stale PID files pile up    | Medium | PID file with stale-PID cleanup on next spawn; `kindling serve --health` for ops |
| Cold-spawn latency exceeds 100ms on slow disks       | Low    | Measure on dogfood; spool fallback only if measured as a real problem            |
| Schema drift between binary and client expectations  | Medium | `/v1/health` reports schemaVersion; client checks on first call, fails loud      |

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
- **D-006:** Triage anvil's integration wishlist (2026-06-22) into module 06 rather than scattering it across 05 — _decided 2026-06-22_ — anvil's downstream KDS module sent 10 asks; auditing them against the tree showed several already shipped (client+spool published at 0.1.0, `/v1/health` handshake, export `bundleVersion` + `--dry-run`) and one mis-framed (no `kindling-spool` crate exists — the spool is `kindling-client::spool`). The genuine new work (publish 0.2.0, daemon dedup, structured query API, capability handshake + kind registry, spool/cold-start observability, redaction evidence, fixtures) is grouped as module 06 so the contract anvil consumes evolves as one coherent surface. KINTEG-001 (publish) is user-gated and gates anvil's consumption of the rest.
- **D-008:** Add `kindling-runtime` as the anvil-first integration facade — _decided 2026-06-24_ — anvil is the primary Rust consumer; requiring a separate `kindling` binary on PATH plus manual `kindling-client` + `spool` + `kindling-server` wiring is the wrong default. A new facade crate composes in-process daemon spawn (pattern already proven in client tests), attach-or-start on the standard socket, and spool-on-by-default at the runtime layer. Does not replace the daemon model or merge the seven crates; CLI/npm adapters stay on the thin client. Tracked as KINTEG-008. See `plans/specs/2026-06-24-kindling-runtime-design.md`.
- **D-005:** Rust-canonical kindling with thin TS HTTP client over local daemon — _decided 2026-05-03_ — Rust becomes the only implementation. Non-Rust consumers reach kindling via `kindling serve` (long-running per-user daemon) over a Unix domain socket. `@eddacraft/kindling` is repurposed as a thin HTTP client ~~with an npm postinstall that downloads the platform binary~~ (_npm distribution superseded by D-007, then D-010_). All other TS implementation packages are deprecated and removed after the cutover. Driven by: sole-operator project means no external migration coordination, every realistic TS consumer can hit a localhost daemon, dual-maintain pays a real tax for a use case nobody asked for. See `plans/specs/2026-05-03-rust-canonical-thin-client-design.md`.
- **D-009:** Architecture for anvil's KDS-004/005 unblockers — a list/enumerate read API (KINTEG-003) and a spool retention cap (KINTEG-009) — _decided 2026-06-26_ — Resolved by a planning council (architect + delivery-lead + adversarial-reviewer). **Read API:** `POST /v1/observations/list` (POST+JSON like every data route), keyset cursor over the stable `(ts ASC, id ASC)` order (the only scheme complete under concurrent appends), half-open `[since, until)` bounds, `kinds` as a list, server-clamped `limit`, redacted excluded by default with a v1 `includeRedacted` flag, **no server-side aggregation** (anvil's `top`/`unused`/`flags`/`principals` are policy — kindling stays mechanism), **no schema bump** (read-only, no DDL; stays v5; old daemon → 404). anvil pins `until = now_ms()` at enumeration start as the snapshot boundary. **Spool cap:** `max_bytes`/`max_age_ms` on `SpoolConfig` (now `#[non_exhaustive]` + builder; client **0.2 → 0.3**), **defaulting to `None`/unbounded** (the council split here — non-None defaults silently trim existing callers on upgrade; 7d/64 MiB are anvil's policy, wired explicitly); add `spooled_at` to `SpoolEntry` for a real age basis; trim the **oldest contiguous prefix only**, **only inside the `flush()` lock**, accepting that trim is intentional bounded retention loss (not infinite retention) — preserving drain order and never dropping an un-drained entry ahead of a kept newer one. Two independent PRs, KINTEG-009 first. See `plans/specs/2026-06-26-kindling-read-api-and-spool-cap-design.md`.
- **D-007:** ~~npm ships the binary via per-platform `optionalDependencies`, not a postinstall download~~ — _decided 2026-06-22_ — _superseded by D-010_ — PR #104 (PORT-018). The esbuild/swc model (one `@eddacraft/kindling-<os>-<arch>[-musl]` package per host; the package manager auto-installs only the matching one) avoids postinstall network entirely, works under `--ignore-scripts`, and is lockfile-deterministic. The specifiers are injected at publish time rather than committed, so `pnpm install --frozen-lockfile` stays green before the platform packages exist on the registry. Supersedes the "npm postinstall that downloads the platform binary" mechanism noted in D-005; D-005's Rust-canonical direction is unchanged.
- **D-010:** Stop publishing to npm; distribute via Rust installs, crates.io and GitHub Release assets — _decided 2026-10-21_ — Distribution is `cargo install eddacraft-kindling`, the Homebrew tap, `install.sh`, the crates on crates.io, and GitHub Release assets (the 7 binaries and the VS Code / Cursor / Windsurf VSIX). The four `packages/*` are marked `private` and kept as source: the thin client is bundled into the VSIX and will be bundled into a future OpenCode plugin that the binary installs (CONV C13); the PocketFlow TS adapter stays in-repo. `publish.yml`, the per-platform `@eddacraft/kindling-<platform>` package machinery and the thin client's platform-package lookup are removed. The npm `latest` of `@eddacraft/kindling` and the OpenCode and PocketFlow adapters is still the retired 0.1.x TypeScript engine, so they are deprecated with a notice pointing at the binary install (`scripts/deprecate.sh npm-channel`, user-gated). Rust PocketFlow support is out of scope. Supersedes the npm distribution parts of D-005 and all of D-007; D-005's Rust-canonical direction stands. Operator decision; the `NPM_TOKEN` secret and the `npm` environment are no longer used.

## Maintenance

Repo-hygiene work tracked here for provenance (outside the module plan):

- **PR #106** (merged 2026-06-21) — dev-toolchain Dependabot sweep: cleared all 16 open alerts (vitest/vite/esbuild/rollup/postcss/minimatch/…) via targeted bumps + same-major `pnpm.overrides`. All dev-only; none ship in any published package (the thin client has no runtime deps). One `ajv` advisory (eslint-pinned, `$data` unused) accepted.
- **PR #107** — CodeQL `js/unnecessary-use-of-cat` (medium): the "no native modules" test read `package.json` by spawning `cat`; switched to `readFileSync`.
- **PR #108** — added `.github/secret_scanning.yml` ignoring `crates/kindling-service/tests/fixtures/**` so the redaction engine's synthetic test secrets stop tripping GitHub's generic (non-provider) secret scanning. True false positives — nothing to rotate.
- **v0.4.0 release prep** (2026-10-19) — bumps the Rust workspace and npm packages 0.3.0 → 0.4.0, adds the `CHANGELOG.md` section and `docs/releases/v0.4.0.md`. Found that no GitHub Release was ever published for v0.2.0 or v0.3.0, so npm `@eddacraft/kindling` `latest` is still the retired 0.1.2 TypeScript implementation and `install.sh` cannot resolve a latest release (ISS-003). Closed GitHub issues #137/#138/#139 (fixed by KINTEG-010/011/012) and #124 (PORT-011 landed).
- **PR #148** — CodeQL `rust/cleartext-logging` #11–#14: taint through `mask_secrets` / `mask_secrets_with_evidence` into `assert!` / CLI `println!`. All false positives after redaction (or used-in-tests). Disposition in `SECURITY.md`; CodeQL model pack at `.github/codeql/extensions/kindling-rust/`.
