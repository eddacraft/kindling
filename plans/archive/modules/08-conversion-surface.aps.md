# Conversion Surface — Delivered Work

| ID   | Owner  | Status   |
| ---- | ------ | -------- |
| CONV | @aneki | Complete |

**Reconciled:** 2026-11-13.

## Purpose

C1/C3/C4 are released in v0.4.0 and retained in v0.4.1. C0/C2/C12 and the
remaining Draft work stay in the [active module](../../modules/08-conversion-surface.aps.md).
See the [completed index](../../completed.aps.md) and
[reconciliation report](../../reconciliation/2026-11-13-aps-cleanup.md).
Notes below are historical; the release prerequisite for C1 is now met.

## Work Items

### C1: install.sh demo prompt

- **Status:** Complete
- **Intent:** install.sh demo prompt available to installed-binary users.
- **Expected Outcome:** The released behaviour matches the original conversion execution checklist.
- **Validation:** Historical implementation and install receipts below; original checkpoints in the [execution plan](../../execution/2026-06-22-conversion-surface-delivery.md).
- **Delivery evidence (2026-11-13):** PR #152 is an ancestor of v0.4.0; published v0.4.0/v0.4.1 binaries include the implementation.

- **C1** (PR #152): `install.sh` offers `kindling demo --reset` after setup. It
  skips the prompt when no terminal is available and honours
  `KINDLING_NO_DEMO=1`. A real `curl | sh` run needs a published Rust release
  (ISS-003).

### C3: Workspace auto-detect (search)

- **Status:** Complete
- **Intent:** Workspace auto-detect (search) available to installed-binary users.
- **Expected Outcome:** The released behaviour matches the original conversion execution checklist.
- **Validation:** Historical implementation and install receipts below; original checkpoints in the [execution plan](../../execution/2026-06-22-conversion-surface-delivery.md).
- **Delivery evidence (2026-11-13):** PR #154 is an ancestor of v0.4.0; published v0.4.0/v0.4.1 binaries include the implementation.

- **C3** (PR #154): `search` defaults `repoId` to the git root, `--repo` wins,
  `--all-repos` widens. Scope extended to `log` and `capsule open` so
  CLI-logged data is findable. On Windows the detected root uses native
  separators to match hook-captured `repoId`; hook project routing is
  unchanged. Only the pure separator function is tested on Windows (CI has no
  Windows run of the `kindling` crate).

### C4: kindling stats

- **Status:** Complete
- **Intent:** kindling stats available to installed-binary users.
- **Expected Outcome:** The released behaviour matches the original conversion execution checklist.
- **Validation:** Historical implementation and install receipts below; original checkpoints in the [execution plan](../../execution/2026-06-22-conversion-surface-delivery.md).
- **Delivery evidence (2026-11-13):** PR #153 is an ancestor of v0.4.0; published v0.4.0/v0.4.1 binaries include the implementation.

- **C4** (PR #153): `kindling stats [--json]` via
  `SqliteKindlingStore::memory_stats` / `KindlingService::stats`. In-process
  only (no daemon endpoint); reusable by C6.
