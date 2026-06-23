# Conversion Surface

| ID   | Owner  | Status |
| ---- | ------ | ------ |
| CONV | @aneki | Ready  |

**Last reviewed:** 2026-06-23

## Purpose

Public visitors and non-Anvil users need a clear path to try, understand, and adopt kindling without Claude Code or anvil. Much of the surface is built on `feat/conversion-surface` but not merged; remaining gaps are CLI depth, distribution polish, and docs sync.

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

| ID  | Title                              | Wave | Status | Branch (suggested)                 |
| --- | ---------------------------------- | ---- | ------ | ---------------------------------- |
| C0  | Merge conversion surface + release | 0    | Ready  | `feat/conversion-surface`          |
| C12 | Post-release verification          | 0    | Draft  | —                                  |
| C1  | install.sh demo prompt             | 1    | Draft  | `feat/install-demo-prompt`         |
| C2  | VSIX + Cursor/Windsurf docs        | 1    | Draft  | `feat/vscode-vsix-and-cursor-docs` |
| C3  | Workspace auto-detect (search)     | 1    | Draft  | `feat/cli-repo-autodetect`         |
| C4  | kindling stats                     | 1    | Draft  | `feat/cli-stats`                   |
| C5  | kindling wrap                      | 2    | Draft  | `feat/cli-wrap`                    |
| C6  | kindling tui                       | 2    | Draft  | `feat/cli-tui`                     |
| C7  | Search filters                     | 2    | Draft  | `feat/cli-search-filters`          |
| C8  | VS Code wrap/tasks docs            | 2    | Draft  | `docs/vscode-wrap-tasks`           |
| C9  | why-kindling one-pager             | 3    | Draft  | `docs/why-kindling-and-sync`       |
| C10 | External docs sync script          | 3    | Draft  | `docs/why-kindling-and-sync`       |
| C11 | Composite GitHub Action            | 4    | Draft  | `feat/kindling-action`             |

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
