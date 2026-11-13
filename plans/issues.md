# Issues & Questions Tracker

Development-time planning discoveries; GitHub owns routine bugs. Reconciled
2026-11-13 against source, publication evidence and the current workspace.

## Issues

### ISS-004: Thin-client and adapter tests leak daemons

| Field      | Value                       |
| ---------- | --------------------------- |
| Status     | Open                        |
| Severity   | Low                         |
| Discovered | C2 (VSIX smoke, 2026-10-21) |
| Module     | 05-rust-port                |

**Context:** After a local `pnpm run test`, about 30 `kindling serve`
processes (`--kindling-home /tmp/kindling-{thin,oc,pf,occmd}-*`) were still
running from the thin-client, opencode and pocketflow suites.

**Impact:** Local test runs pile up daemons and temp directories. Test teardown
should stop each spawned daemon and remove its temporary home.

**Reconciliation (2026-11-13):** Remains Open. The thin-client afterEach removes
temporary homes but does not stop the auto-spawned daemon. No new leak test was
run during this documentation cleanup.

### ISS-005: Intent-event completion claim outlived its implementation

| Field      | Value                          |
| ---------- | ------------------------------ |
| Status     | Open                           |
| Severity   | Medium                         |
| Discovered | APS reconciliation, 2026-11-13 |
| Module     | 07-intent-capture-events       |

**Context:** KINTENT-001..006 were merged in the retired TypeScript engine, but
the current Rust CLI and thin client have no append-only intent-event contract.
The module is Blocked. [GH-215](https://github.com/eddacraft/kindling/issues/215)
owns the restore-or-retire decision and migration guidance. This reconciliation
neither restores the feature nor treats historical merges as a shipped surface.

## Questions

No new product decision is made by the cleanup. The GH-215 and deferred
PORT-020 1.0.0 decisions remain with the maintainer.

## Resolved

### ISS-001: API rate limits lower than expected

| Field      | Value    |
| ---------- | -------- |
| Status     | Open     |
| Severity   | Medium   |
| Discovered | AUTH-002 |
| Module     | AUTH     |

**Context:** During load testing, discovered the API rate-limits at 100 req/min, not 1000 as documented.

**Impact:** Will need retry logic or batching for bulk operations.
-->

### ISS-002: Workspace project count exceeds pre-release estimate

| Field      | Value                  |
| ---------- | ---------------------- |
| Status     | Resolved               |
| Severity   | Low                    |
| Discovered | release-prep preflight |
| Module     | 08-conversion-surface  |

**Context:** After merging conversion surface, `pnpm install` reports 7 workspace projects (4 packages + vscode adapter + plugin + `examples/adapter-minimal`), not the 5 estimated in the release plan. All install cleanly with `--frozen-lockfile`.

**Resolution (2026-11-13):** `pnpm -r list --depth -1 --json` lists seven
projects including the root, four packages, the plugin and the minimal example.
This is the current workspace inventory; the release plan's old estimate is not
an installation defect.

### ISS-003: No GitHub Release published since v0.1.2

| Field      | Value                 |
| ---------- | --------------------- |
| Status     | Resolved              |
| Severity   | High                  |
| Discovered | C12 (release-prep)    |
| Module     | 08-conversion-surface |

**Context:** The v0.3.0 tag was pushed and the crates were published to crates.io,
but no GitHub Release was created, and none exists for 0.2.0 either.
`release.yml` (binaries, VSIX, Homebrew tap PR) triggers on
`release: published`, so it has not run for a Rust-canonical version. (The npm
`publish.yml` that also triggered on it was removed by D-010.)

**Impact:** npm `@eddacraft/kindling` `latest` is still the retired 0.1.2
TypeScript implementation (since D-010 it is deprecated rather than replaced;
see PORT-020); no prebuilt binaries are attached to any release;
`install.sh` fails because `releases/latest` is 404; the Homebrew formula was
never updated. The v0.4.0 release prep is written so that publishing its GitHub
Release closes all of this in one go (runbook section 4).

**Resolution (2026-10-22):** v0.4.0 was published as a GitHub Release on
2026-10-21. `release.yml` attached the binaries and the VSIX, and
`releases/latest` now resolves. The Homebrew tap PR job failed on a script bug
(fixed in #160), so the tap PR was opened by hand (eddacraft/homebrew-tap#1).
The remaining release work is tracked under C12.

**Reconciliation (2026-11-13):** v0.4.1 is now latest, with binaries and VSIX;
homebrew-tap#1 is merged. Remaining acceptance belongs to C0/C2/C12, not this
resolved no-release issue.
