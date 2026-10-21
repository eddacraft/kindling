# Issues & Questions Tracker

> Development-time discoveries that emerge while building. Not a bug tracker replacement — a lightweight log for planning-level concerns that need visibility.

---

## Issues

<!--
Issues are problems discovered during development.
ID format: ISS-NNN (e.g., ISS-001)
Status: Open | Resolved | Deferred | Won't Fix
Severity: Critical | High | Medium | Low

Example:
### ISS-001: API rate limits lower than expected

| Field | Value |
|-------|-------|
| Status | Open |
| Severity | Medium |
| Discovered | AUTH-002 |
| Module | AUTH |

**Context:** During load testing, discovered the API rate-limits at 100 req/min, not 1000 as documented.

**Impact:** Will need retry logic or batching for bulk operations.
-->

### ISS-001: Plugin test glob required on Node 26

| Field      | Value                  |
| ---------- | ---------------------- |
| Status     | Resolved               |
| Severity   | Low                    |
| Discovered | release-prep preflight |
| Module     | 08-conversion-surface  |

**Context:** `node --test test/` fails on Node 26 (`MODULE_NOT_FOUND`); the glob `test/*.test.js` works and preserves skip-when-no-binary behaviour.

### ISS-002: Workspace project count exceeds pre-release estimate

| Field      | Value                  |
| ---------- | ---------------------- |
| Status     | Open                   |
| Severity   | Low                    |
| Discovered | release-prep preflight |
| Module     | 08-conversion-surface  |

**Context:** After merging conversion surface, `pnpm install` reports 7 workspace projects (4 packages + vscode adapter + plugin + `examples/adapter-minimal`), not the 5 estimated in the release plan. All install cleanly with `--frozen-lockfile`.

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

---

## Questions

<!--
Questions are unknowns that emerged during development.
ID format: Q-NNN (e.g., Q-001)
Status: Open | Answered | Deferred
Priority: High | Medium | Low

Example:
### Q-001: Should retry logic live in the client or transport layer?

| Field | Value |
|-------|-------|
| Status | Open |
| Priority | Medium |
| Discovered | AUTH-002 |
| Assigned | @username |

**Context:** Found we need retry logic for rate limits. Unclear where this belongs architecturally.

**Options considered:**
1. Client layer — simpler, but each client reimplements
2. Transport layer — centralized, but may hide failures
-->

_(No questions yet)_

---

## Resolved

<!--
Move resolved issues and answered questions here.
Keep for 1-2 sprints as reference, then archive or delete.
-->

_(Nothing resolved yet)_

---

## Quick Reference

| ID Type  | Format  | Example |
| -------- | ------- | ------- |
| Issue    | ISS-NNN | ISS-001 |
| Question | Q-NNN   | Q-001   |

**Severities:** Critical > High > Medium > Low

**Reference from other docs:** `See ISS-001` or `Related: ISS-001, Q-002`
