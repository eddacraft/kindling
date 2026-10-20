# Conversion Surface

| ID   | Owner  | Status      |
| ---- | ------ | ----------- |
| CONV | @aneki | In Progress |

**Last reviewed:** 2026-10-21 (wave 1 C1–C4 merged; C12 still blocked on the v0.4.0 GitHub Release, see ISS-003; D-010 drops npm, C13 drafted)

## Purpose

Public visitors and non-Anvil users need a clear path to try, understand, and adopt kindling without Claude Code or anvil. The conversion surface branch (`feat/conversion-surface`) is merging to main; remaining gaps are release ops, CLI depth, distribution polish, and docs sync.

Execution plan: [../execution/2026-06-22-conversion-surface-delivery.md](../execution/2026-06-22-conversion-surface-delivery.md).

## In Scope

- Merge `feat/conversion-surface` (demo, browse, VS Code adapter, homebrew updates, onboarding docs)
- Homebrew (macOS + Linux glibc) via automated tap PR
- ~~npm publish of `@eddacraft/kindling-adapter-vscode`~~ (_superseded by D-010_); VSIX on GitHub Release
- OpenCode plugin bundled into and installed by the `kindling` binary (C13)
- Asciinema demo embedded in README
- Follow-up CLI depth (`stats`, `wrap`, `tui`, search filters) and docs sync (C1–C11)

## Out of Scope

- anvil integration or KINTEG downstream contract work
- musl targets in Homebrew (use `install.sh`)
- Semantic/embedding retrieval or cloud-hosted kindling

## Interfaces

**Depends on:**

- `05-rust-port` (daemon, thin client, distribution) — conversion surface ships on top of the Rust cutover

**Exposes:**

- `kindling demo`, `kindling browse` CLI commands
- Editor extension VSIX (`kindling-<version>.vsix`) on each GitHub Release (`@eddacraft/kindling-adapter-vscode` is a private source package, not on npm)
- Public onboarding docs (`docs/quickstart/`, `docs/integrations.md`, adapter cookbook)

## Ready Checklist

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] Execution plan written
- [x] At least one task defined

## Work Items

| ID  | Title                               | Wave | Status      | Branch (suggested)                        |
| --- | ----------------------------------- | ---- | ----------- | ----------------------------------------- |
| C0  | Merge conversion surface + release  | 0    | Merged      | `feat/conversion-surface`                 |
| C12 | Post-release verification           | 0    | In Progress | —                                         |
| C1  | install.sh demo prompt              | 1    | Merged      | `feat/install-demo-prompt` (#152)         |
| C2  | VSIX + Cursor/Windsurf docs         | 1    | Merged      | `feat/vscode-vsix-and-cursor-docs` (#155) |
| C3  | Workspace auto-detect (search)      | 1    | Merged      | `feat/cli-repo-autodetect` (#154)         |
| C4  | kindling stats                      | 1    | Merged      | `feat/cli-stats` (#153)                   |
| C5  | kindling wrap                       | 2    | Draft       | `feat/cli-wrap`                           |
| C6  | kindling tui                        | 2    | Draft       | `feat/cli-tui`                            |
| C7  | Search filters                      | 2    | Draft       | `feat/cli-search-filters`                 |
| C8  | VS Code wrap/tasks docs             | 2    | Draft       | `docs/vscode-wrap-tasks`                  |
| C9  | why-kindling one-pager              | 3    | Draft       | `docs/why-kindling-and-sync`              |
| C10 | External docs sync script           | 3    | Draft       | `docs/why-kindling-and-sync`              |
| C11 | Composite GitHub Action             | 4    | Draft       | `feat/kindling-action`                    |
| C13 | OpenCode plugin via `kindling init` | 2    | Draft       | `feat/opencode-plugin`                    |

### C12 findings (2026-10-19)

Checked against the registries and the GitHub API from the release-prep branch:

- GitHub Releases stop at v0.1.2 (pre-release). The v0.3.0 tag exists but was
  never published as a Release, so `release.yml` (and the since-removed npm
  `publish.yml`) have not run for any Rust-canonical version. `releases/latest` returns 404, so
  `curl … install.sh | sh` fails to resolve a version. Tracked as ISS-003.
- npm `@eddacraft/kindling` `latest` is 0.1.2 (the retired TypeScript
  implementation). The thin client has never been published. The seven retired
  packages are deprecated on the registry (PORT-020). Since D-010 (2026-10-21)
  nothing is published to npm: all four `packages/*` are `private`, the
  editor extension ships only as the Release VSIX, and `@eddacraft/kindling`
  plus the OpenCode and PocketFlow adapters are to be deprecated
  (`scripts/deprecate.sh npm-channel`, PORT-020).
- crates.io carries all workspace crates at 0.3.0; `kindling-bench` is `publish = false`.
- Not verifiable from the branch: the `HOMEBREW_TAP_TOKEN` secret, the
  Homebrew tap PR, `brew install` on macOS/Linux, and the asciinema demo.
  (`NPM_TOKEN` is no longer needed after D-010 and can be removed.)

Re-run the Task 0.2 checklist once the v0.4.0 GitHub Release is published.

### Wave 1 notes (2026-10-21)

- **C1** (PR #152): `install.sh` offers `kindling demo --reset` after setup. It
  skips the prompt when no terminal is available and honours
  `KINDLING_NO_DEMO=1`. A real `curl | sh` run needs a published Rust release
  (ISS-003).
- **C2** (PR #155): `pnpm --filter @eddacraft/kindling-adapter-vscode run package`
  builds `kindling-<version>.vsix`; `release.yml` attaches it on published
  releases only. Unproven until a real release runs; a manual Cursor/Windsurf
  install smoke test is still owed.
- **C3** (PR #154): `search` defaults `repoId` to the git root, `--repo` wins,
  `--all-repos` widens. Scope extended to `log` and `capsule open` so
  CLI-logged data is findable. On Windows the detected root uses native
  separators to match hook-captured `repoId`; hook project routing is
  unchanged. Only the pure separator function is tested on Windows (CI has no
  Windows run of the `kindling` crate).
- **C4** (PR #153): `kindling stats [--json]` via
  `SqliteKindlingStore::memory_stats` / `KindlingService::stats`. In-process
  only (no daemon endpoint); reusable by C6.
- Found while testing: the thin-client and adapter test suites leave
  `kindling serve` daemons running (ISS-004).

### C13: OpenCode plugin bundled and installed by `kindling init --opencode` (Draft)

- **Intent:** OpenCode users get capture without npm (D-010): the `kindling`
  binary carries a prebuilt OpenCode plugin and installs it.
- **Expected Outcome:** A single bundled JS file (the OpenCode adapter plus the
  thin client, built with esbuild the way the VSIX bundles the client) is
  embedded in the binary; `kindling init --opencode` writes it to OpenCode's
  plugin location for the project or user and reports where it went. Rerunning
  is idempotent and upgrades a stale copy.
- **Validation:** First verify OpenCode's current plugin loading rules
  (discovery paths, module format, the plugin API and event names) against
  OpenCode's own docs and source, since they have changed before. Then verify
  that the thin client's HTTP-over-UDS transport (Node `http` with
  `socketPath`, plus auto-spawn via `child_process`) works under Bun, which
  OpenCode runs plugins on. Done when a fresh project captures an OpenCode
  session into `kindling search` with only the binary installed.
- **Status:** Draft. Not started; captured by D-010.
- **Dependencies:** C0; PORT-019 (adapter over the thin client, Merged).

## Dependencies

```
C0 → C12, C1, C2, C3, C4, C10, C11, C13
C1, C5 → C8
C4 → C6 (soft)
C3 → C7
C5 → C11
```

## Constraints

- UK English in all public-facing prose; no em dashes
- Non-Anvil positioning: kindling stands alone; anvil is a one-line footer at most
- Homebrew: macOS + Linux glibc only (anvil tap pattern); musl via `install.sh`
- `HOMEBREW_TAP_TOKEN` required before first release with formula automation
- Existing Claude Code hook interface unchanged

## Risks

| Risk                              | Mitigation                                          |
| --------------------------------- | --------------------------------------------------- |
| Release lacks darwin/linux assets | Gate C0 on `release.yml` cross-build green          |
| Tap PR fails (missing secret)     | Document `HOMEBREW_TAP_TOKEN` in C0 checklist       |
| TUI flaky in CI                   | Unit-test filter logic; manual smoke for ratatui    |
| VS Code terminal API limits       | Ship `wrap` first; docs-only for terminal in editor |
