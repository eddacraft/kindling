# Conversion Surface

| ID   | Owner  | Status      |
| ---- | ------ | ----------- |
| CONV | @aneki | In Progress |

**Last reviewed:** 2026-11-13 (v0.4.1 is the latest published release; C1/C3/C4 archived; C0/C2/C12 retain unmet acceptance).

## Purpose

Public visitors and non-Anvil users need a clear path to try, understand, and adopt kindling without Claude Code or anvil. The conversion implementation is released. Remaining gaps are release acceptance, CLI depth, distribution polish, and docs sync.

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

## Delivery boundary

C1/C3/C4 are Complete and indexed in [completed.aps.md](../completed.aps.md);
their details are in the [archive](../archive/modules/08-conversion-surface.aps.md).
C0's merge and binary release are delivered, but the execution checklist still
owes a recorded/embedded demo and verified external docs publication. C2's VSIX
and docs are released; a Cursor/Windsurf install smoke receipt is missing.
C12 remains In Progress until the remaining install/editor checks are supplied.

## Work Items

| ID  | Title                               | Wave | Status      | Branch (suggested)                        |
| --- | ----------------------------------- | ---- | ----------- | ----------------------------------------- |
| C0  | Merge conversion surface + release  | 0    | In Progress | `feat/conversion-surface`                 |
| C12 | Post-release verification           | 0    | In Progress | —                                         |
| C2  | VSIX + Cursor/Windsurf docs         | 1    | Merged      | `feat/vscode-vsix-and-cursor-docs` (#155) |
| C5  | kindling wrap                       | 2    | Draft       | `feat/cli-wrap`                           |
| C6  | kindling tui                        | 2    | Draft       | `feat/cli-tui`                            |
| C7  | Search filters                      | 2    | Draft       | `feat/cli-search-filters`                 |
| C8  | VS Code wrap/tasks docs             | 2    | Draft       | `docs/vscode-wrap-tasks`                  |
| C9  | why-kindling one-pager              | 3    | Draft       | `docs/why-kindling-and-sync`              |
| C10 | External docs sync script           | 3    | Draft       | `docs/why-kindling-and-sync`              |
| C11 | Composite GitHub Action             | 4    | Draft       | `feat/kindling-action`                    |
| C13 | OpenCode plugin via `kindling init` | 2    | Draft       | `feat/opencode-plugin`                    |
| C14 | crates.io trusted publishing        | 3    | Draft       | `ci/crates-trusted-publishing`            |

### Historical C12 findings (2026-10-19)

The findings and progress below are dated evidence; the current reconciliation
above and below supersedes their pending-release and pending-tap statements.

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

### Historical C12 progress (2026-10-22)

- The v0.4.0 GitHub Release was published on 2026-10-21, and
  `releases/latest` resolves to v0.4.0. All 7 binary archives and
  `kindling-0.4.0.vsix` are attached, each with a `.sha256` sidecar.
- `HOMEBREW_TAP_TOKEN` is set, but the `open homebrew-tap PR` job still
  failed. `generate-homebrew-formula.sh` captured its own log line in the
  checksum values, and `sed` choked on the URL. Fixed in #160. The v0.4.0
  formula was generated locally with the fixed script and opened as
  eddacraft/homebrew-tap#1, which awaits merge.
- npm deprecation notices are live on `@eddacraft/kindling` and the OpenCode
  and PocketFlow adapters (PORT-020).
- crates.io: all 7 crates published at 0.4.0 (`scripts/publish.sh`,
  2026-10-22). They are owned solely by the maintainer's personal crates.io
  account, as every earlier version was; there is no eddacraft user or team on
  crates.io. Left as is; see C14.
- Verified on Linux x86_64 (2026-10-22), each reporting `kindling 0.4.0`:
  `install.sh` from `main` (resolves `releases/latest`, downloads the
  `x86_64-unknown-linux-gnu` asset), `cargo install eddacraft-kindling`
  (compiles the 0.4.0 crate graph from crates.io), and
  `brew install eddacraft/tap/kindling` from the `kindling-v0.4.0` branch of
  the tap (formula URL + sha256 for the glibc tarball check out).
- Still owed: merge eddacraft/homebrew-tap#1 (then `brew install` from the
  tap's `main`), the Cursor/Windsurf VSIX smoke test, and removing the unused
  `NPM_TOKEN` secret.

### C12 reconciliation (2026-11-13)

- Latest published GitHub Release is [v0.4.1](https://github.com/eddacraft/kindling/releases/tag/v0.4.1)
  (2026-11-10), with seven binary archives, a VSIX and eight SHA256 sidecars.
  v0.4.0 remains published. Assets do not prove every native install journey.
- [homebrew-tap#1](https://github.com/eddacraft/homebrew-tap/pull/1) merged on
  2026-10-22. Remove that merge from the outstanding checklist; a fresh install
  from tap main and the macOS/Linux matrix still need receipts.
- crates.io carries all eight publishable workspace crates at 0.4.1. Runtime
  publication is no longer pending. npm remains retired; current deprecation
  notices were read back, not changed, in this reconciliation.
- The release workflows for both v0.4.0 and v0.4.1 conclude failure even though
  their binary and VSIX assets are present. Do not call release automation green;
  Homebrew automation fixes after v0.4.1 are merged, unreleased maintenance.
- Still owed: native install coverage in PORT-010/014, fresh Claude Code session
  coverage in PORT-015, the Cursor/Windsurf VSIX smoke receipt, the demo embed,
  external docs publication evidence, and confirmation that unused NPM_TOKEN
  was removed. Secret values were not inspected and no release action was taken.

### C14: crates.io trusted publishing (Draft)

Publish the crates from `release.yml` via crates.io Trusted Publishing (OIDC)
instead of a personal token in `scripts/publish.sh`, and add an eddacraft
identity as a crate owner. Prerequisite: an eddacraft identity that crates.io
can recognise, i.e. a GitHub team in the `eddacraft` org (owner syntax
`github:eddacraft:<team>`) or a dedicated org account. None exists today.
Yanking existing versions does not change owners or the publisher record, so
this is purely forward-looking. Also covers `eddacraft-tui` and `aps-cli`,
which are in the same position.

### C2 release and acceptance

PR #155 is an ancestor of v0.4.0. Both v0.4.0 and v0.4.1 have the packaged
VSIX and checksum attached; Cursor/Windsurf installation remains unverified.
This is Merged with released implementation, pending the manual acceptance
receipt. Thin-client/adapter daemon leaks remain tracked as ISS-004.

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
- **Dependencies:** C0; PORT-019 (adapter over the thin client, Complete in the PORT archive).

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
