# Rust Port

| ID   | Owner  | Status      |
| ---- | ------ | ----------- |
| PORT | @aneki | In Progress |

**Last reviewed:** 2026-11-13.

## Purpose

Close the remaining acceptance and lifecycle work for the Rust-canonical port.
The engine, daemon, CLI, hooks and thin client ship in v0.4.0 and v0.4.1.
Fifteen delivered items and the superseded npm binary-delivery item are in the
[archive](../archive/modules/05-rust-port.aps.md), indexed by
[completed.aps.md](../completed.aps.md). Superseded HOOK/CLI plans are historical;
they do not authorise parallel implementations.

## Current implementation and boundaries

- Eight published workspace crates: types, store, provider, service, server,
  client, runtime and eddacraft-kindling; kindling-bench is not published.
  Filtering, hooks and CLI code are folded into the service and binary crates.
- One binary exposes hook, daemon and CLI commands; Unix uses UDS and Windows
  uses authenticated TCP. Project routing remains per project.
- The canonical schema lives under `schema/`; vendored copies are checked in CI.
- Cargo, install.sh, Homebrew and GitHub Release assets are the distribution
  channels. `@eddacraft/kindling` is a private source client bundled by consumers,
  including the release VSIX. npm distribution was retired by D-010.
- The original design is [Rust canonical thin client](../specs/2026-05-03-rust-canonical-thin-client-design.md).
  Schema and npm foundation work are archived dependencies, not open blockers.
- New downstream contracts belong to module 06; conversion work to module 08.
  Native acceptance and release/lifecycle decisions below remain outstanding.

## Ready Checklist

- [x] Released implementation distinguished from remaining acceptance
- [x] Historical records and dependencies indexed
- [ ] Outstanding platform, plugin and lifecycle receipts supplied

## Work Items

### PORT-010: Cross-platform CI hook builds

- **Intent:** Hook (and full binary) builds for all target platforms from Phase 2 onward
- **Expected Outcome:** GitHub Actions produces release artefacts for Linux (x86_64, aarch64, both gnu + musl), macOS (x86_64, aarch64), Windows (x86_64); `cargo-zigbuild` handles cross-targets from a single Linux runner; artefacts attached to GitHub Releases
- **Validation:** CI matrix green; each artefact runs `kindling --version` successfully on the target platform (smoke-test step in CI)
- **Status:** Merged
- **Reconciliation (2026-11-13):** Seven target archives and checksums exist in v0.4.0/v0.4.1. The cross-build workflow executes Linux binaries but only checks macOS/Windows file formats; native execution acceptance is not established. Release workflow overall conclusions are failure, so a green matrix is not inferred.
- **Historical status:** Merged — `feat/rust-port-release-ci`, PR #70 (rebase `89df01b`+`bb77090`). `.github/workflows/release.yml`: cargo-zigbuild 7-target cross-build from one Linux runner; smoke `kindling --version` (native + QEMU arm64 via `docker/setup-qemu-action`, format-verify macOS/windows); tar.gz/zip + sha256 packaging; release-asset upload. QEMU binfmt fix (`docker/setup-qemu-action@v3`) added after first run hit `exec format error`.
- **Delivered dependencies:** PORT-009 (Complete; [archive](../archive/modules/05-rust-port.aps.md)).

### PORT-014: Native distribution channels

- **Intent:** Install paths for Rust-native and platform-native consumers
- **Expected Outcome:** `cargo install kindling` publishes to crates.io; Homebrew tap at `eddacraft/tap` with formula; `curl -sSL install.kindling.dev | sh` installs the latest release binary for the detected platform (Linux/macOS); install script verifies SHA256 against the GitHub Release manifest
- **Validation:** Fresh install on Linux, macOS, and Windows via each method; post-install Claude Code session exercises all 7 hook types end-to-end
- **Status:** In Progress
- **Reconciliation (2026-11-13):** Cargo, install.sh and GitHub Release publication are real; homebrew-tap#1 merged 2026-10-22. Linux 0.4.0 install receipts are in C12. Fresh installs and all seven hooks on the requested native platform matrix remain unverified. Use cargo install eddacraft-kindling and the repository install.sh; install.kindling.dev was an original proposal, not a verified channel.
- **Historical status:** Merged (in-repo slice) — `feat/rust-port-distribution`, PR #75 (`c735af2`). Landed: cargo-publish readiness (schema vendored into `crates/kindling-store/schema/` + `crates/kindling-client/schema/` to fix the `include_str!` outside-crate-dir blocker, `version="0.0.1"` on all path deps, `scripts/sync-vendored-schema.sh` + `vendored-schema` CI drift gate), `scripts/publish.sh` (dep-ordered), `packaging/{install.sh,homebrew/kindling.rb,README.md}`, `cargo publish --dry-run -p kindling-types` green. CREDENTIAL-GATED remainder deferred to user (tracked, not blocking the port): actual `cargo publish` (crates.io token), `eddacraft/tap` repo, `install.kindling.dev` domain, a real GitHub Release, cross-platform fresh-install validation.
- **Dependencies:** PORT-010
- **Delivered dependencies:** PORT-013 (Complete; [archive](../archive/modules/05-rust-port.aps.md)).

### PORT-015: Plugin hook cutover

- **Intent:** `plugins/kindling-claude-code` invokes the Rust binary instead of the Node.js hook scripts
- **Expected Outcome:** Plugin `hooks.json` calls `kindling hook <type>` (resolved via PATH or bundled binary location); Node.js hook scripts removed from the plugin; plugin install instructions updated
- **Validation:** Fresh plugin install on a clean machine: hooks fire end-to-end via the Rust binary; observation rows present in the per-project DB
- **Status:** Merged
- **Reconciliation (2026-11-13):** The plugin cutover is included in v0.4.0/v0.4.1. A clean-machine plugin session proving hook observations in the routed database is still owed; release publication alone is not that receipt.
- **Historical status:** Merged — `feat/rust-port-plugin-cutover`, PR #76 (`a743f7d`). hooks.json's 7 commands now `kindling hook <type>` (PATH-resolved); the 7 Node hook scripts removed. README lists the `kindling` binary as a prerequisite. Note: the `/memory` slash-command scripts were subsequently cut to the Rust binary too (`4512ec8`, the PORT-020 prerequisite), and `hooks/lib/{init,filter}.js` + the bundled TS engine removed — the plugin now has zero dependency on the old TS implementation packages. End-to-end fresh-install validation needs the published binary (user, post-release).
- **Dependencies:** PORT-014

### PORT-020: Deprecate and remove TS implementation packages

- **Intent:** Retire the TS implementation surface that the Rust daemon now owns
- **Expected Outcome:** `@eddacraft/kindling-core`, `-store-sqlite`, `-store-sqljs`, `-provider-local`, `-server`, `-cli` published with a `console.warn` deprecation in their entry point and an `npm deprecate` notice pointing at `@eddacraft/kindling`; anvil TS bridge `@eddacraft/anvil-kindling-integration` deprecated and removed in tandem with the anvil cutover (tracked in eddacraft repo); package source removed from this repo at `1.0.0` cut
- **Validation:** `npm view <pkg>` shows deprecation notice for each retired package; this repo no longer contains the deprecated package source after the `1.0.0` tag; anvil builds without `@eddacraft/anvil-kindling-integration`
- **Status:** In Progress
- **Reconciliation (2026-11-13):** Source removal is in the published releases and current npm notices were read back on 2026-11-13. The seven earlier notices still recommend the now-retired thin client; repointing them remains optional maintenance. No 1.0.0 tag exists. Preserve the maintainer's deferred 1.0.0 decision and separate anvil TS-bridge follow-up.
- **Historical status:** In Progress — in-repo source removal done (`feat/next-release-prep`): the seven retired TS implementation package directories (`-core`, `-store-sqlite`, `-store-sqljs`, `-provider-local`, `-server`, `-cli`, `-adapter-claude-code`) and stale `tsconfig.base.json` path entries are gone; `pnpm install --frozen-lockfile` succeeds. Prior deprecation slice (warn-at-entry + `scripts/deprecate.sh`) landed earlier. `npm deprecate` VERIFIED on the registry 2026-10-19: all seven retired packages (`-core`, `-store-sqlite`, `-store-sqljs`, `-provider-local`, `-server`, `-cli`, `-adapter-claude-code`) show a deprecation notice at 0.1.2. D-010 npm-channel deprecation VERIFIED on the registry 2026-10-22: `@eddacraft/kindling`, `@eddacraft/kindling-adapter-opencode` and `@eddacraft/kindling-adapter-pocketflow` (0.1.2 is the only version of each ever published) carry the notice pointing at the binary install. REMAINING: (1) optionally `scripts/deprecate.sh retired-ts` to re-point the seven earlier notices away from the now-deprecated thin client; (2) the formal `1.0.0` tag cut naming the removal complete — DEFERRED by maintainer decision (2026-10-20); the 0.4.0 release does not make that call. Anvil `command.invoked` daemon path landed (PORT-011); full TS-bridge removal in anvil repo is separate follow-up.
- **Delivered dependencies:** PORT-019 and PORT-011 (Complete; [archive](../archive/modules/05-rust-port.aps.md)).

### PORT-023: CLI routes to the same project database as hooks

- **Intent:** A CLI run from any directory inside a repository reads and writes the same per-project database the hooks write, so hook-captured memory is visible and CLI writes are not fragmented across subdirectory databases.
- **Root cause:** Three project-root resolutions exist. The hook path (`hook/project.rs::project_root`) resolves `KINDLING_REPO_ROOT` (only when cwd is inside it), then `git rev-parse --show-toplevel`, then cwd, and hashes that into `~/.kindling/projects/<hash>/`. The CLI embedded path (`lib.rs::resolve_db_path`) and daemon path (`lib.rs::build_client` → `Client::new()` → `ClientConfig::defaults()`) both hash `std::env::current_dir()` directly. A CLI run from `/repo/src` therefore hashes `/repo/src` while the hook hashes `/repo`. This contradicts the documented identity in `CONTEXT.md` (Project root) and `docs/architecture.md` (Project root: git toplevel of cwd).
- **Expected Outcome:**
  - `resolve_db_path(None)` derives its default from `hook::project::project_root(cwd)` rather than raw cwd.
  - `build_client(None)` builds `ClientConfig::defaults()`, sets `project_root = project_root(cwd)` and uses `Client::with_config`, so `--via-daemon` routes to the same project as the hook.
  - Explicit overrides keep their precedence: `--db` (embedded early return; still rejected with `--via-daemon`), `KINDLING_DB_PATH` (inside `kindling_store::resolve_db_path`), and `KINDLING_REPO_ROOT` (step 1 of `project_root`, only when cwd is inside it).
  - Outside a Git repository the resolved root is still the (lexically normalised) cwd, so non-Git behaviour is unchanged.
  - The resolver is reached through one crate-level helper so embedded and daemon paths cannot drift again.
- **Validation:**
  - `cargo test -p eddacraft-kindling` — includes new end-to-end tests that run the real binary (no `--db`, temp `HOME`, `KINDLING_DB_PATH`/`KINDLING_REPO_ROOT` unset) from a Git repository root and from a nested subdirectory, embedded and `--via-daemon`, and assert both invocations resolve to the root's project database and see the same data; plus one asserting `KINDLING_DB_PATH` still wins from a subdirectory.
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo fmt --all -- --check`
- **Existing data:** No automatic migration. Hooks already write to the Git-root hash, so the CLI is re-pointed at the existing hook database. Databases the CLI created under a subdirectory hash are no longer the default for that directory; they remain on disk and are reachable with `--db` or `KINDLING_DB_PATH` (export then import to merge). On Windows the CLI previously hashed the backslash cwd while Git reports forward slashes, so CLI-only data written from a repository root moves too; the CHANGELOG upgrade note covers both cases.
- **Docs:** CHANGELOG (Unreleased → Fixed, with the upgrade note) and ENGINEERING-HISTORY (Unreleased); update the `resolve_db_path` / `build_client` doc comments. `CONTEXT.md` and `docs/architecture.md` already document the intended behaviour; correct them only if they say otherwise.
- **Status:** Ready
- **Files:** `crates/kindling/src/lib.rs`, `crates/kindling/src/hook/project.rs` (visibility only, if needed), `crates/kindling/tests/`, `CHANGELOG.md`, `ENGINEERING-HISTORY.md`
- **Dependencies:** None
- **Constraints / non-goals:** Contained to the `eddacraft-kindling` crate. Do not change `kindling-client`'s `ClientConfig::defaults()` cwd fallback (the SDK default; callers wanting Git-root routing set `project_root` explicitly). Do not change the project-root hash, the `git_toplevel` raw-output contract, or the store path layout. No database migration tooling. `KINDLING_SOCK` support in the CLI daemon path is out of scope.
- **Risk:** standard — routing change affects which DB a CLI invocation opens; covered by end-to-end tests and the upgrade note.
- **Issue:** [GH-205](https://github.com/eddacraft/kindling/issues/205)
