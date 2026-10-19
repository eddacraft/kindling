# Conversion Surface

| ID   | Owner  | Status      |
| ---- | ------ | ----------- |
| CONV | @aneki | In Progress |

**Last reviewed:** 2026-10-19 (C12 started; blocked on the v0.4.0 GitHub Release, see ISS-003)

## Purpose

Public visitors and non-Anvil users need a clear path to try, understand, and adopt kindling without Claude Code or anvil. The conversion surface branch (`feat/conversion-surface`) is merging to main; remaining gaps are release ops, CLI depth, distribution polish, and docs sync.

Execution plan: [../execution/2026-06-22-conversion-surface-delivery.md](../execution/2026-06-22-conversion-surface-delivery.md).

## In Scope

- Merge `feat/conversion-surface` (demo, browse, VS Code adapter, homebrew updates, onboarding docs)
- Homebrew (macOS + Linux glibc) via automated tap PR
- npm publish of `@eddacraft/kindling-adapter-vscode`; VSIX on GitHub Release
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
- `@eddacraft/kindling-adapter-vscode` npm package
- Public onboarding docs (`docs/quickstart/`, `docs/integrations.md`, adapter cookbook)

## Ready Checklist

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] Execution plan written
- [x] At least one task defined

## Work Items

| ID  | Title                              | Wave | Status      | Branch (suggested)                 |
| --- | ---------------------------------- | ---- | ----------- | ---------------------------------- |
| C0  | Merge conversion surface + release | 0    | Merged      | `feat/conversion-surface`          |
| C12 | Post-release verification          | 0    | In Progress | —                                  |
| C1  | install.sh demo prompt             | 1    | Draft       | `feat/install-demo-prompt`         |
| C2  | VSIX + Cursor/Windsurf docs        | 1    | Draft       | `feat/vscode-vsix-and-cursor-docs` |
| C3  | Workspace auto-detect (search)     | 1    | Draft       | `feat/cli-repo-autodetect`         |
| C4  | kindling stats                     | 1    | Draft       | `feat/cli-stats`                   |
| C5  | kindling wrap                      | 2    | Draft       | `feat/cli-wrap`                    |
| C6  | kindling tui                       | 2    | Draft       | `feat/cli-tui`                     |
| C7  | Search filters                     | 2    | Draft       | `feat/cli-search-filters`          |
| C8  | VS Code wrap/tasks docs            | 2    | Draft       | `docs/vscode-wrap-tasks`           |
| C9  | why-kindling one-pager             | 3    | Draft       | `docs/why-kindling-and-sync`       |
| C10 | External docs sync script          | 3    | Draft       | `docs/why-kindling-and-sync`       |
| C11 | Composite GitHub Action            | 4    | Draft       | `feat/kindling-action`             |

### C12 findings (2026-10-19)

Checked against the registries and the GitHub API from the release-prep branch:

- GitHub Releases stop at v0.1.2 (pre-release). The v0.3.0 tag exists but was
  never published as a Release, so `publish.yml` and `release.yml` have not run
  for any Rust-canonical version. `releases/latest` returns 404, so
  `curl … install.sh | sh` fails to resolve a version. Tracked as ISS-003.
- npm `@eddacraft/kindling` `latest` is 0.1.2 (the retired TypeScript
  implementation). The thin client has never been published. The seven retired
  packages are deprecated on the registry (PORT-020). `@eddacraft/kindling-adapter-vscode`
  is not on npm yet; it is not private, so the first release publish will pick it up
  via `pnpm publish -r`.
- crates.io carries all workspace crates at 0.3.0; `kindling-bench` is `publish = false`.
- Not verifiable from the branch: `HOMEBREW_TAP_TOKEN` and `NPM_TOKEN` secrets,
  the Homebrew tap PR, `brew install` on macOS/Linux, and the asciinema demo.

Re-run the Task 0.2 checklist once the v0.4.0 GitHub Release is published.

## Dependencies

```
C0 → C12, C1, C2, C3, C4, C10, C11
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
