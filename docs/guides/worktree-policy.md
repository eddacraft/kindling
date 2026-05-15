# Worktree Policy

## Overview

Use git worktrees as lightweight execution spaces for active branches.

Kindling keeps one permanent anchor worktree and treats everything else as
disposable.

## Permanent Worktrees

Keep exactly one long-lived worktree:

1. `main`

Suggested directories:

- `../kindling.main`

This is the stable anchor for integration and release work.

## Disposable Worktrees

Create disposable worktrees for active streams only:

- `feat/*`
- `fix/*`
- `docs/*`
- `chore/*`
- `release/*`
- `hotfix/*`
- short-lived spikes

Suggested directory pattern:

- `../wt-<branch-slug>`

Examples:

- `../wt-recall-skill`
- `../wt-plugin-sqlite`
- `../wt-release-0.3.0`
- `../wt-hotfix-fts-tokenizer`

## Why disposable is the default

Disposable worktrees reduce drift and maintenance overhead.

Permanent feature worktrees tend to accumulate:

- stale branches
- hidden divergence from `main`
- rebasing overhead
- unfinished work that feels active but is not moving

The branch or PR is the unit of work. The worktree is just the workspace.

## Branch Creation Rules

1. Create normal work branches from `main`.
2. Create release branches from `main`.
3. Create hotfix branches from `main` or the active `release/*` branch.
4. Merge completed work into its target branch, then remove the worktree.

## Age Limits

Use these limits as hygiene rules rather than hard technical constraints:

- feat, fix, docs, chore: target under 5 active days
- release worktree: target under 3 days of stabilisation
- spike worktree: target under 2 days before convert-or-close

Any disposable worktree older than 7 days should be reviewed and either:

- merged
- split into smaller branches
- rebased and continued with intent
- closed and removed

## WIP Limits

1. Keep no more than 4–5 disposable worktrees open at once.
2. If you reach the limit, do not create another until one is merged, paused,
   or removed.
3. If a stream is blocked and you are not returning within 48 hours, remove the
   worktree and keep the branch reference only if needed.

## Cleanup Rules

Remove disposable worktrees when:

1. the branch is merged
2. the branch is abandoned
3. the branch is superseded by a replacement branch
4. the branch is blocked with no near-term next action

Delete merged disposable branches and remove their worktrees on the same day.

## Review Rhythm

Review open worktrees at least twice a week. Check for:

1. merged branches that still have a worktree
2. stale branches with no recent progress
3. branches that should be split or rebased
4. streams that should be merged into `main`

## Relationship to Branching

Worktree policy supports the [branching strategy](branching-strategy.md):

1. `main` remains stable and publishable.
2. Disposable worktrees support parallel execution without turning every stream
   into a permanent branch.
