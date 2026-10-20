# Branching Strategy

## Overview

kindling uses a single permanent branch model that supports active development
across short-lived work branches while keeping releases stable and predictable:

- `main` is the default branch, integration branch, and release branch.

The key rule is branch hygiene: all work happens on short-lived branches and
lands through PRs to `main`. `main` must stay publishable at all times.

## Branches

| Branch                                 | Purpose                                                                        | Protection                                              |
| -------------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------- |
| `main`                                 | Default integration branch and stable branch for releases. Always publishable. | PRs only. Full CI gate.                                 |
| `release/x.y` or `release/x.y.z`       | Temporary release stabilisation branch cut from `main`.                        | PRs or maintainer-only pushes during release hardening. |
| `feat/*`, `fix/*`, `docs/*`, `chore/*` | Short-lived work branches created from `main`.                                 | Disposable.                                             |
| `hotfix/*`                             | Urgent production fix branch created from `main` or the active release branch. | Disposable.                                             |

## Workflow

```text
feat/*  ──PR──► main
fix/*   ──PR──► main
docs/*  ──PR──► main

main ──cut──► release/x.y.z ──PR──► main
main ──branch──► hotfix/* ──PR──► main
```

## Normal Development

1. Create feature, fix, docs, and chore branches from `main`.
2. Merge completed work into `main` through PRs.
3. Keep branches small and short-lived.
4. Use APS plans (in `plans/`) and work-item IDs for planning. Branch structure
   should reflect code flow, not roadmap ownership.

## Release Flow

1. Keep `main` release-ready by requiring CI before merge.
2. For low-risk releases, tag directly from `main` after version and changelog
   updates land.
3. For higher-risk releases, cut `release/x.y` or `release/x.y.z` from `main`.
4. Allow only release hardening on `release/*`: bug fixes, packaging, docs,
   changelog, and version bumps.
5. Merge `release/*` into `main`, then tag the release.
6. Tagging `vX.Y.Z` on `main` and creating a GitHub Release triggers
   `.github/workflows/release.yml`, which attaches the binaries and the VSIX to
   the Release and opens the Homebrew tap PR. The crates are published to
   crates.io with `scripts/publish.sh`. Nothing is published to npm.

See the [release runbook](release-runbook.md) for the full step-by-step.

## Hotfix Flow

1. Branch `hotfix/*` from `main` or the active `release/*` branch.
2. Merge the fix into the release target first.
3. Tag the patch release if needed.

## Cadence Rules

1. Keep work branches under a few days where practical.
2. During heavy development, merge small PRs frequently rather than batching.
3. Do not allow `release/*` branches to live for weeks.
4. If a PR feels too large to review comfortably, split it before merging.

## Divergence Guardrails

1. Avoid long-lived release-only changes.
2. Keep release hardening scoped to `release/*` and merge it back to `main` as
   soon as it is stable.
3. Rebase or recreate stale work branches before opening PRs.

## Branch Naming

- `feat/recall-skill`
- `fix/plugin-review-feedback`
- `docs/branching-strategy`
- `chore/dependency-bumps`
- `release/0.3.0`
- `hotfix/sqlite-migration-rollback`

## CI Tiers

### PRs to `main`

- Build
- Type check
- Lint
- Unit tests (Linux, current LTS Node)

### Release (`release.yml`)

- Build-only cross-build check on PRs and pushes that touch the Rust crates.
- On a published GitHub Release: attaches the 7 binary archives and the VSIX,
  then opens the Homebrew tap PR.

## Why this model

kindling is maintained by a single operator, so a permanent integration branch
adds coordination overhead without much safety. Short-lived branches preserve
parallel work while keeping the repository's source of truth on `main`.

## Related Docs

- [Release Runbook](release-runbook.md)
- [Worktree Policy](worktree-policy.md)
