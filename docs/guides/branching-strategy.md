# Branching Strategy

## Overview

Kindling uses a two-branch model that supports active development across
multiple parallel streams while keeping releases stable and predictable:

- `main` is the stable release branch.
- `dev` is the active integration branch.

The key rule is cadence: `dev` is a short-horizon integration branch, not a
long-lived alternate product line. The model only works if release promotion
is frequent and every `main`-only fix is merged back quickly.

## Branches

| Branch                                 | Purpose                                                                        | Protection                                              |
| -------------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------- |
| `main`                                 | Stable branch for npm releases. Always publishable.                            | PRs only. Full release CI gate.                         |
| `dev`                                  | Active integration branch for day-to-day work from multiple streams.           | PRs required. Standard CI.                              |
| `release/x.y` or `release/x.y.z`       | Temporary release stabilisation branch cut from `dev`.                         | PRs or maintainer-only pushes during release hardening. |
| `feat/*`, `fix/*`, `docs/*`, `chore/*` | Short-lived work branches created from `dev`.                                  | Disposable.                                             |
| `hotfix/*`                             | Urgent production fix branch created from `main` or the active release branch. | Disposable.                                             |

## Workflow

```text
feat/*  ──PR──► dev ──PR──► main
fix/*   ──PR──► dev ──PR──► main
docs/*  ──PR──► dev ──PR──► main

dev ──cut──► release/x.y.z ──PR──► main ──merge back──► dev
main ──branch──► hotfix/* ──PR──► main ──merge back──► dev
```

## Normal Development

1. Create feature, fix, docs, and chore branches from `dev`.
2. Merge completed work into `dev` continuously.
3. Keep branches small and short-lived.
4. Use APS plans (in `plans/`) and work-item IDs for planning. Branch structure
   should reflect code flow, not roadmap ownership.

## Release Flow

1. Promote `dev` to `main` frequently.
2. For low-risk releases, open a direct `dev → main` release PR.
3. For higher-risk releases, cut `release/x.y` or `release/x.y.z` from `dev`.
4. Allow only release hardening on `release/*`: bug fixes, packaging, docs,
   changelog, and version bumps.
5. Merge `release/*` into `main`, tag the release, then merge the release branch
   back into `dev` immediately.
6. Tagging `vX.Y.Z` on `main` and creating a GitHub Release triggers
   `.github/workflows/publish.yml`, which publishes all packages to npm.

See the [release runbook](release-runbook.md) for the full step-by-step.

## Hotfix Flow

1. Branch `hotfix/*` from `main` or the active `release/*` branch.
2. Merge the fix into the release target first.
3. Tag the patch release if needed.
4. Merge the same fix back into `dev` on the same day.

## Cadence Rules

1. Promote `dev → main` at least weekly while there is active development.
2. During heavy development, prefer promotion every 2–3 days.
3. Do not allow `release/*` branches to live for weeks.
4. If the `dev → main` PR feels too large to review comfortably, promotion is
   already overdue.
5. If a fix lands on `main`, it is not complete until `dev` has it too.

## Divergence Guardrails

1. `main` and `dev` must stay close enough that promotion remains routine.
2. Stop queuing new release work if `main...dev` grows beyond a small,
   reviewable change set.
3. Avoid long-lived release-only changes on `main`.

## Branch Naming

- `feat/recall-skill`
- `fix/plugin-review-feedback`
- `docs/branching-strategy`
- `chore/dependency-bumps`
- `release/0.3.0`
- `hotfix/sqlite-migration-rollback`

## CI Tiers

### PRs to `dev` (lightweight)

- Build
- Type check
- Lint
- Unit tests (Linux, current LTS Node)

### PRs to `main` (release gate)

All of the above plus:

- Cross-platform smoke tests (macOS and Windows) — verifies prebuilt binary
  targets before publish.

### Publish (`publish.yml`)

- Triggered by GitHub Releases (tag push on `main`).
- Re-runs CI as a gate, then publishes all workspace packages with npm
  provenance.

## Why this model

Kindling regularly has multiple active streams in flight (DX hardening, plugin
work, Rust port planning, adapter changes). `dev` provides a safe integration
target before release while `main` stays publishable at any time. The process
fails when promotion waits too long, because release fixes accumulate on `main`
and structural work continues on `dev`.

## Related Docs

- [Release Runbook](release-runbook.md)
- [Worktree Policy](worktree-policy.md)
