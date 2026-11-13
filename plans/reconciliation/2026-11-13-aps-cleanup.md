# APS reconciliation — 2026-11-13

## Scope and rule

Audited all nine module plans: 87 original identities (72 heading-based items
and 15 C0..C14 table entries). Final disposition: **44 Complete, 14 Superseded,
29 active**. Active items comprise 15 Draft, 6 Blocked, 5 In Progress and 3 Merged.
No implementation, registry mutation, release, issue closure or product decision
was performed. Main and the sibling repository were read; edits are isolated on
`docs/aps-reconciliation` from kindling `5379c5f4f88d3049d648f82b7414fd1f6aba9135`.

Followed anvil-001's `plans/completed.aps.md`, active index and
`plans/archive/modules/` convention. Whole completed or superseded modules move;
mixed modules split at item boundaries. Existing IDs and original intent,
outcomes, validation receipts and implementation notes are preserved in detail.
Historical commands for removed packages are not current executable acceptance.
A published implementation with missing item acceptance stays active. Later
bugs do not erase a delivered API's release history.

## Evidence refreshed

- Local HEAD equals the live GitHub main SHA above at audit time. No change to
  another branch/worktree is included.
- [v0.4.0 release](https://github.com/eddacraft/kindling/releases/tag/v0.4.0):
  published 2026-10-21; annotated tag resolves to
  `a2704b4d0cce1447dd1e1124b6608cc49a331ac5`.
- [v0.4.1 release](https://github.com/eddacraft/kindling/releases/tag/v0.4.1):
  published 2026-11-10; tag resolves to
  `1dc494883a55e6d4a0e883bd7d41469a7e4dbc18`. Each release has seven target
  archives and a VSIX, each with a checksum sidecar (16 assets).
- Release workflow conclusions remain failure for
  [v0.4.0](https://github.com/eddacraft/kindling/actions/runs/39569455429) and
  [v0.4.1](https://github.com/eddacraft/kindling/actions/runs/42786932761).
  Asset presence and workflow success are separate receipts.
- crates.io API version records read for all eight publishable workspace crates
  at 0.4.1: types, store, provider, service, server, client, runtime and
  eddacraft-kindling; all non-yanked. Client/runtime/binary histories also confirm
  0.3.0 and 0.4.0. Client/binary 0.2.0 was published 2026-06-23, correcting the
  older plan's 2026-06-24 verification date. Runtime first appears at 0.3.0.
  [Client history](https://crates.io/crates/kindling-client/versions),
  [runtime history](https://crates.io/crates/kindling-runtime/versions).
- Ten npm 0.1.2 package notices read from registry.npmjs.org: seven retired
  implementation packages plus kindling, OpenCode and PocketFlow. All deprecated;
  the seven earlier notices still recommend the now-retired thin client.
  [npm publication workflow](https://github.com/eddacraft/kindling/actions/runs/22067119341)
  succeeded. No notices were changed.
- [homebrew-tap#1](https://github.com/eddacraft/homebrew-tap/pull/1) merged
  2026-10-22, commit `b0f0ebe02bd396d7d44480e77a7382dc548ee7c6`.
- GitHub PR data was paged through the full kindling history. Named delivery
  commits were tested with `git merge-base --is-ancestor <sha> <tag>`.
  Foundation, schema, port, KINTEG and conversion/public-direction changes are
  included in v0.4.0 as recorded per item. PUBLISH-001's PR #35 postdates npm
  v0.1.2, so its completion uses v0.4.0 ancestry rather than an earlier release.
- PORT-011: live anvil PRs #2897 (`9ab2cfd0`) and #2906 (`a20b2c88`) both
  pass ancestry against published
  [anvil v0.9.0-beta](https://github.com/eddacraft/anvil-001/releases/tag/v0.9.0-beta)
  (2026-07-12, commit `6b0ed1d1d7d7662d1033403ae6291d907afe262d`).
  Sibling KDS archive records Released/Shipped; current source retains the
  daemon sink and parity test. The sibling tree was not modified.

## Source audit and unresolved boundaries

- Cargo.toml, scripts/publish.sh and the registry agree on eight publishable
  crates, plus unpublished kindling-bench. Obsolete 10-crate layouts are
  delivery-time history, not the current architecture.
- CLI commands, runtime API, hook/plugin configuration, generated type pipeline,
  fixture tree, schema contract, private TS package manifests and release
  workflows were compared with module outcomes.
- KINTENT module-level Done contradicted its old Merged items and current
  availability. The old implementation was removed by PR #105. PocketFlow's
  `inferIntent` is naming inference, not the hash-chained intent-event store.
  [GH-215](https://github.com/eddacraft/kindling/issues/215) remains open;
  six items are Blocked, not archived as complete.
- KINTEG-007 promised Claude Code, OpenCode and PocketFlow fixtures; only
  `fixtures/hook-payloads/claude-code.json` exists. Its delivered subset stays
  documented, with the full item In Progress. No acceptance narrowing.
- PORT-010 uses format checks for macOS/Windows; no fresh native execution was
  performed. PORT-014/015 and C0/C2/C12 retain missing install/plugin/editor,
  demo and external-docs receipts. C12's 2026-10-22 Linux installs are historical
  receipts, not a fresh native platform sweep.
- KINTEG-003's concurrent-insert completeness wording is disputed by
  [GH-186](https://github.com/eddacraft/kindling/issues/186); archive delivery
  of deterministic pagination without closing that issue. Benchmark and other
  post-release bugs likewise remain in GitHub, outside this plan cleanup.
- PORT-020's formal 1.0.0 declaration remains deferred by the maintainer.
  Delivered source removal does not override that decision.
- No wrap, TUI, requested search filters, OpenCode bundled plugin, composite
  Action, docs-sync script, ROADMAP.md, contributor issue forms or APS public
  projection implementation was found for the corresponding Draft outcomes.
  C14 trusted publishing is not implemented in release.yml.

## Supporting plan documents

All seven design specs, two execution documents and four post-merge reviews
were inventoried. Preserve them at stable paths as rationale and evidence:
implementation of a design does not make its entire historical cross-repository
proposal a completed work item. Added current-status notes where designs,
handoffs or checklists advertised unpublished runtime/APIs, npm distribution,
pending integration or already-delivered work. The dual-maintain spec was
already Superseded. Hearth remains a historical proposal and cannot override
D-005/D-010. Managed aps-rules.md is unchanged; project-context.md now defines
this project's archive/delivery conventions.

ISS-001/003 stay Resolved and move to the resolved section; ISS-002's stale
workspace estimate is reconciled to the current seven-project inventory and
resolved. ISS-004 remains Open after teardown source review; ISS-005 links the
intent-availability decision to GH-215. No GitHub issue was changed.

## Complete disposition inventory

The Before column records the original item status; parent-level Done did not
supply missing item delivery evidence. Paths identify the final authoritative
record. Every original identity occurs exactly once as an item definition.

| Item        | Before      | After       | Detail                                                                                                 | Evidence or remaining work                                                                                                                                                                                          |
| ----------- | ----------- | ----------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PUBLISH-001 | Ready       | Complete    | [01-npm-publish](../archive/modules/01-npm-publish.aps.md)                                             | PRs #14/#15/#35 are merged and ancestors of v0.4.0; #35 postdates v0.1.2.                                                                                                                                           |
| PUBLISH-002 | Ready       | Complete    | [01-npm-publish](../archive/modules/01-npm-publish.aps.md)                                             | Historical npm 0.1.2 publication and successful Publish to npm release workflow; channel retired by D-010.                                                                                                          |
| PUBLISH-003 | Ready       | Complete    | [01-npm-publish](../archive/modules/01-npm-publish.aps.md)                                             | Historical npm 0.1.2 publication and successful Publish to npm release workflow; channel retired by D-010.                                                                                                          |
| PUBLISH-004 | Ready       | Complete    | [01-npm-publish](../archive/modules/01-npm-publish.aps.md)                                             | Historical npm 0.1.2 publication and successful Publish to npm release workflow; channel retired by D-010.                                                                                                          |
| PUBLISH-005 | Ready       | Complete    | [01-npm-publish](../archive/modules/01-npm-publish.aps.md)                                             | Historical npm 0.1.2 publication and successful Publish to npm release workflow; channel retired by D-010.                                                                                                          |
| PUBLISH-006 | Draft       | Complete    | [01-npm-publish](../archive/modules/01-npm-publish.aps.md)                                             | Historical npm 0.1.2 publication and successful Publish to npm release workflow; channel retired by D-010.                                                                                                          |
| HOOK-001    | Ready       | Superseded  | [02-rust-hook-binary](../archive/modules/02-rust-hook-binary.aps.md)                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| HOOK-002    | Ready       | Superseded  | [02-rust-hook-binary](../archive/modules/02-rust-hook-binary.aps.md)                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| HOOK-003    | Ready       | Superseded  | [02-rust-hook-binary](../archive/modules/02-rust-hook-binary.aps.md)                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| HOOK-004    | Ready       | Superseded  | [02-rust-hook-binary](../archive/modules/02-rust-hook-binary.aps.md)                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| HOOK-005    | Ready       | Superseded  | [02-rust-hook-binary](../archive/modules/02-rust-hook-binary.aps.md)                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| HOOK-006    | Draft       | Superseded  | [02-rust-hook-binary](../archive/modules/02-rust-hook-binary.aps.md)                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| HOOK-007    | Draft       | Superseded  | [02-rust-hook-binary](../archive/modules/02-rust-hook-binary.aps.md)                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| HOOK-008    | Draft       | Superseded  | [02-rust-hook-binary](../archive/modules/02-rust-hook-binary.aps.md)                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| CLI-001     | Draft       | Superseded  | [03-rust-cli](../archive/modules/03-rust-cli.aps.md)                                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| CLI-002     | Draft       | Superseded  | [03-rust-cli](../archive/modules/03-rust-cli.aps.md)                                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| CLI-003     | Draft       | Superseded  | [03-rust-cli](../archive/modules/03-rust-cli.aps.md)                                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| CLI-004     | Draft       | Superseded  | [03-rust-cli](../archive/modules/03-rust-cli.aps.md)                                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| CLI-005     | Draft       | Superseded  | [03-rust-cli](../archive/modules/03-rust-cli.aps.md)                                                   | D-004 absorbs this phase into PORT; do not select the historical item.                                                                                                                                              |
| SCHEMA-001  | Done        | Complete    | [04-schema-contract](../archive/modules/04-schema-contract.aps.md)                                     | Schema extraction commit 2285c96 is an ancestor of v0.4.0; canonical schema and version contract remain in the release tree.                                                                                        |
| SCHEMA-002  | Done        | Complete    | [04-schema-contract](../archive/modules/04-schema-contract.aps.md)                                     | Schema extraction commit 2285c96 is an ancestor of v0.4.0; canonical schema and version contract remain in the release tree.                                                                                        |
| SCHEMA-003  | Done        | Complete    | [04-schema-contract](../archive/modules/04-schema-contract.aps.md)                                     | Schema extraction commit 2285c96 is an ancestor of v0.4.0; canonical schema and version contract remain in the release tree.                                                                                        |
| SCHEMA-004  | Done        | Complete    | [04-schema-contract](../archive/modules/04-schema-contract.aps.md)                                     | Schema extraction commit 2285c96 is an ancestor of v0.4.0; canonical schema and version contract remain in the release tree.                                                                                        |
| SCHEMA-005  | Done        | Complete    | [04-schema-contract](../archive/modules/04-schema-contract.aps.md)                                     | Schema extraction commit 2285c96 is an ancestor of v0.4.0; canonical schema and version contract remain in the release tree.                                                                                        |
| PORT-001    | Done        | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-002    | Done        | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-003    | Done        | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-004    | Done        | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-005    | Done        | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-006    | Merged      | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-007    | Merged      | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-008    | Merged      | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-009    | Merged      | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-010    | Merged      | Merged      | [05-rust-port](../modules/05-rust-port.aps.md)                                                         | Released assets exist; native macOS/Windows execution and a green release matrix not proved.                                                                                                                        |
| PORT-011    | Merged      | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | anvil PRs #2897 (9ab2cfd0) and #2906 (a20b2c88) are ancestors of published anvil v0.9.0-beta (2026-07-12, commit 6b0ed1d1). KDS-001/002/003 release receipts and parity test are present in the sibling repository. |
| PORT-012    | Merged      | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-013    | Merged      | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-014    | Merged      | In Progress | [05-rust-port](../modules/05-rust-port.aps.md)                                                         | Published channels and historical Linux installs exist; requested native install/hook matrix remains owed.                                                                                                          |
| PORT-015    | Merged      | Merged      | [05-rust-port](../modules/05-rust-port.aps.md)                                                         | Plugin cutover is released; clean-machine hook/database receipt remains owed.                                                                                                                                       |
| PORT-016    | Merged      | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-017    | Merged      | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-018    | Superseded  | Superseded  | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | D-010 retires npm binary delivery before platform-package publication.                                                                                                                                              |
| PORT-019    | Merged      | Complete    | [05-rust-port](../archive/modules/05-rust-port.aps.md)                                                 | Implementation included in published GitHub Release v0.4.0 and its crate graph; retained in v0.4.1. Historical validation and known limitations follow.                                                             |
| PORT-020    | In Progress | In Progress | [05-rust-port](../modules/05-rust-port.aps.md)                                                         | Source removal/deprecation delivered; maintainer deferred 1.0.0 declaration; anvil bridge follow-up separate.                                                                                                       |
| KINTEG-001  | Done        | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.                                                                                                            |
| KINTEG-002  | Merged      | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.                                                                                                            |
| KINTEG-003  | Merged      | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.                                                                                                            |
| KINTEG-009  | Merged      | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.                                                                                                            |
| KINTEG-004  | Done        | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.                                                                                                            |
| KINTEG-005  | Complete    | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.                                                                                                            |
| KINTEG-006  | Merged      | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.                                                                                                                 |
| KINTEG-007  | Merged      | In Progress | [06-downstream-integration-surface](../modules/06-downstream-integration-surface.aps.md)               | Only Claude Code fixtures delivered; expected OpenCode/PocketFlow families and round-trips missing.                                                                                                                 |
| KINTEG-008  | Merged      | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Published crates.io 0.3.0 (2026-06-25); implementation also included in published GitHub Release v0.4.0.                                                                                                            |
| KINTEG-010  | Merged      | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.                                                                                                                 |
| KINTEG-011  | Merged      | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.                                                                                                                 |
| KINTEG-012  | Merged      | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.                                                                                                                 |
| KINTEG-013  | Merged      | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.                                                                                                                 |
| KINTEG-014  | Merged      | Complete    | [06-downstream-integration-surface](../archive/modules/06-downstream-integration-surface.aps.md)       | Implementation included in published GitHub Release v0.4.0 and crates.io 0.4.0; retained in v0.4.1.                                                                                                                 |
| KINTEG-015  | Draft       | Draft       | [06-downstream-integration-surface](../modules/06-downstream-integration-surface.aps.md)               | Draft generic-event@1 and retained-key append; current runtime does not expose these contracts.                                                                                                                     |
| KINTENT-001 | Merged      | Blocked     | [07-intent-capture-events](../modules/07-intent-capture-events.aps.md)                                 | Current Rust release lacks the retired TS intent contract; GH-215 owns restore-or-retire.                                                                                                                           |
| KINTENT-002 | Merged      | Blocked     | [07-intent-capture-events](../modules/07-intent-capture-events.aps.md)                                 | Current Rust release lacks the retired TS intent contract; GH-215 owns restore-or-retire.                                                                                                                           |
| KINTENT-003 | Merged      | Blocked     | [07-intent-capture-events](../modules/07-intent-capture-events.aps.md)                                 | Current Rust release lacks the retired TS intent contract; GH-215 owns restore-or-retire.                                                                                                                           |
| KINTENT-004 | Merged      | Blocked     | [07-intent-capture-events](../modules/07-intent-capture-events.aps.md)                                 | Current Rust release lacks the retired TS intent contract; GH-215 owns restore-or-retire.                                                                                                                           |
| KINTENT-005 | Merged      | Blocked     | [07-intent-capture-events](../modules/07-intent-capture-events.aps.md)                                 | Current Rust release lacks the retired TS intent contract; GH-215 owns restore-or-retire.                                                                                                                           |
| KINTENT-006 | Merged      | Blocked     | [07-intent-capture-events](../modules/07-intent-capture-events.aps.md)                                 | Current Rust release lacks the retired TS intent contract; GH-215 owns restore-or-retire.                                                                                                                           |
| PUBDIR-001  | Merged      | Complete    | [09-public-direction-and-contribution](../archive/modules/09-public-direction-and-contribution.aps.md) | PR #151 (70cf1f52) is an ancestor of v0.4.0; VISION.md, README links and CONTRIBUTING.md are live on main.                                                                                                          |
| PUBDIR-002  | Merged      | Complete    | [09-public-direction-and-contribution](../archive/modules/09-public-direction-and-contribution.aps.md) | PR #151 (70cf1f52) is an ancestor of v0.4.0; VISION.md, README links and CONTRIBUTING.md are live on main.                                                                                                          |
| PUBDIR-003  | Draft       | Draft       | [09-public-direction-and-contribution](../modules/09-public-direction-and-contribution.aps.md)         | No delivered implementation of the planned contributor surface; prerequisites do not approve Draft work.                                                                                                            |
| PUBDIR-004  | Draft       | Draft       | [09-public-direction-and-contribution](../modules/09-public-direction-and-contribution.aps.md)         | No delivered implementation of the planned contributor surface; prerequisites do not approve Draft work.                                                                                                            |
| PUBDIR-005  | Draft       | Draft       | [09-public-direction-and-contribution](../modules/09-public-direction-and-contribution.aps.md)         | No delivered implementation of the planned contributor surface; prerequisites do not approve Draft work.                                                                                                            |
| PUBDIR-006  | Draft       | Draft       | [09-public-direction-and-contribution](../modules/09-public-direction-and-contribution.aps.md)         | No delivered implementation of the planned contributor surface; prerequisites do not approve Draft work.                                                                                                            |
| PUBDIR-007  | Draft       | Draft       | [09-public-direction-and-contribution](../modules/09-public-direction-and-contribution.aps.md)         | No delivered implementation of the planned contributor surface; prerequisites do not approve Draft work.                                                                                                            |
| C0          | Merged      | In Progress | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | Merge and release delivered; demo embed and external docs publication receipts still missing.                                                                                                                       |
| C1          | Merged      | Complete    | [08-conversion-surface](../archive/modules/08-conversion-surface.aps.md)                               | Implementation PR is an ancestor of published v0.4.0; retained in v0.4.1.                                                                                                                                           |
| C2          | Merged      | Merged      | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | VSIX attached to releases; Cursor/Windsurf install smoke remains owed.                                                                                                                                              |
| C3          | Merged      | Complete    | [08-conversion-surface](../archive/modules/08-conversion-surface.aps.md)                               | Implementation PR is an ancestor of published v0.4.0; retained in v0.4.1.                                                                                                                                           |
| C4          | Merged      | Complete    | [08-conversion-surface](../archive/modules/08-conversion-surface.aps.md)                               | Implementation PR is an ancestor of published v0.4.0; retained in v0.4.1.                                                                                                                                           |
| C5          | Draft       | Draft       | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | No delivered implementation found for this Draft outcome; no readiness approval inferred.                                                                                                                           |
| C6          | Draft       | Draft       | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | No delivered implementation found for this Draft outcome; no readiness approval inferred.                                                                                                                           |
| C7          | Draft       | Draft       | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | No delivered implementation found for this Draft outcome; no readiness approval inferred.                                                                                                                           |
| C8          | Draft       | Draft       | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | No delivered implementation found for this Draft outcome; no readiness approval inferred.                                                                                                                           |
| C9          | Draft       | Draft       | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | No delivered implementation found for this Draft outcome; no readiness approval inferred.                                                                                                                           |
| C10         | Draft       | Draft       | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | No delivered implementation found for this Draft outcome; no readiness approval inferred.                                                                                                                           |
| C11         | Draft       | Draft       | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | No delivered implementation found for this Draft outcome; no readiness approval inferred.                                                                                                                           |
| C12         | In Progress | In Progress | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | Tap PR merged; native install/editor checks and remaining release ops still owed.                                                                                                                                   |
| C13         | Draft       | Draft       | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | No delivered implementation found for this Draft outcome; no readiness approval inferred.                                                                                                                           |
| C14         | Draft       | Draft       | [08-conversion-surface](../modules/08-conversion-surface.aps.md)                                       | No delivered implementation found for this Draft outcome; no readiness approval inferred.                                                                                                                           |

## Verification and limits

The cleanup validates planning consistency and delivery evidence. Historical
Rust/TS test receipts are preserved; removed-package validation commands are not
replayed. Native install tests and product acceptance are not claimed.

Project tooling: `aps doctor` reports no blocking problems, with pin drift
(project 0.3.0 vs installed 0.9.0), leftover vendored CLI and unmanaged/missing
skill metadata. Tool migration and generated skill changes are outside this
plan cleanup. No repo docs/date-normalisation gate exists; use APS lint,
format checking, local links and inventory checks, then whitespace validation.

Active prerequisite reconciliation moves completed prerequisites to explicit
Delivered dependencies fields with archive links. Dependencies now names unmet
work only; the native selector does not load archive items. No item is promoted
to Ready by this bookkeeping.

The native `aps export --json` succeeds but omits the legacy C-numbered conversion
module and reports Merged item statuses as Unknown. Those are pre-existing CLI
representation limits, not proof that the conversion backlog is empty. The
inventory verifier below covers all 87 identities directly and preserves their
existing IDs. No CLI/toolchain migration was performed.

Validation at closeout:

- `aps lint`: exit 0, all 16 APS files valid (non-strict; pin warning retained).
- `aps next`: exit 0, no Ready work item; not a complete export of the legacy C table.
- `pnpm exec oxfmt --check plans`: exit 0, all 36 Markdown files formatted.
- `git diff --check`: exit 0.
- The inline Python verifier below: exit 0; all 87 identities accounted for,
  exactly 44 completed-index entries, 87 report rows and no broken local links
  in non-template plan documents. Template placeholders are excluded.
- `pnpm run format:check`: exit 1, 466 existing formatting failures outside
  the plan surface. Every working-tree change is under plans/ and the focused
  plans formatter passes; unrelated files were not reformatted.

## Reproducible inventory and link check

Run from the repository root against the recorded original base of this branch:

````sh
python3 - <<'PY'
import collections,re,subprocess
from pathlib import Path
defs={}
for p in Path('plans').rglob('*.aps.md'):
 if p.name in ['index.aps.md','completed.aps.md']: continue
 text=p.read_text()
 if p.name=='08-conversion-surface.aps.md' and 'archive' not in p.parts:
  for line in text.splitlines():
   parts=[s.strip() for s in line.split('|')]
   if len(parts)>4 and re.fullmatch(r'C\d+',parts[1]):defs.setdefault(parts[1],[]).append((str(p),parts[4]))
  continue
 for m in re.finditer(r'^#{3,4} ([A-Z][A-Z0-9]*-\d+|C\d+): (.+)$',text,re.M):
  part=text[m.end():]; nxt=re.search(r'^#{1,4} ',part,re.M); part=part[:nxt.start()] if nxt else part
  status=re.search(r'\*\*Status:\*\* ([^\n]+)',part)
  defs.setdefault(m[1],[]).append((str(p),re.match(r'Complete|Superseded|Draft|Blocked|In Progress|Merged',status[1])[0] if status else 'MISSING'))
old={}
for name in subprocess.check_output(['git','ls-tree','--name-only','5379c5f4f88d3049d648f82b7414fd1f6aba9135:plans/modules'],text=True).splitlines():
 if not name.endswith('.aps.md'): continue
 text=subprocess.check_output(['git','show','5379c5f4f88d3049d648f82b7414fd1f6aba9135:plans/modules/'+name],text=True)
 for m in re.finditer(r'^#{3,4} ([A-Z][A-Z0-9]*-\d+):',text,re.M):old[m[1]]=name
 if name=='08-conversion-surface.aps.md':
  for m in re.finditer(r'^\| (C\d+) +\|',text,re.M):old[m[1]]=name
assert set(defs)==set(old),(set(old)-set(defs),set(defs)-set(old))
assert all(len(v)==1 for v in defs.values()), {k:v for k,v in defs.items() if len(v)!=1}
counts=collections.Counter(v[0][1] for v in defs.values())
assert counts=={'Complete':44,'Superseded':14,'Draft':15,'Blocked':6,'In Progress':5,'Merged':3},counts
for id,entries in defs.items():
 p,status=entries[0]
 assert ('/archive/' in p)==(status in ['Complete','Superseded']),(id,p,status)
completed=set(re.findall(r'^\| (\S+) +\|[^\n]*\| Complete +\|',Path('plans/completed.aps.md').read_text(),re.M))
assert completed=={id for id,v in defs.items() if v[0][1]=='Complete'},completed
report_ids=re.findall(r'^\|\s*([A-Z][A-Z0-9]*-\d+|C\d+) +\|',Path('plans/reconciliation/2026-11-13-aps-cleanup.md').read_text(),re.M)
assert collections.Counter(report_ids)==collections.Counter(old.keys()),len(report_ids)
bad=[];links=0
for p in Path('plans').rglob('*.md'):
 if p.name.startswith('.') or '.template.' in p.name: continue
 text=re.sub(r'```.*?```','',p.read_text(),flags=re.S)
 for label,target in re.findall(r'\[([^\]]+)\]\(([^)]+)\)',text):
  if re.match(r'\w+://|mailto:|#',target):continue
  links+=1
  if not (p.parent/target.split('#')[0]).exists():bad.append((str(p),target))
print('Inventory PASS:',len(defs),'unique identities;',dict(counts))
print('Completed index PASS:',len(completed),'matches; report PASS:',len(report_ids),'dispositions')
print('Local links:',links,'checked;',len(bad),'broken')
for x in bad: print(*x)
assert not bad
PY
````

## Docs closeout and evidence

- Changed files: plans/ only; active modules, archive/index, reconciliation,
  project context, issues, design annotations and handoff/checklist updates.
- Doc type: APS plans and delivery records.
- Reader/job: maintainer and future agents locating executable work and delivery proof.
- Source truth checked: repository source and workflows, full PR history, tag
  ancestry, live GitHub Releases, crates.io versions, npm deprecations and sibling KDS receipts.
- Links/anchors checked: existing local file targets, including relocated module
  links; original work-item headings are preserved in their detail records.
- Commands and exits: recorded above; broad formatting is an inherited failure,
  while scoped plan formatting, APS lint, inventory and links pass.
- Claim: plan reconciliation and evidence-backed archival are supported. Product
  acceptance and release automation success are not claimed by the cleanup.
- Audit base: `5379c5f4f88d3049d648f82b7414fd1f6aba9135`; the plans-only change
  was prepared on `docs/aps-reconciliation` and rebased onto current main for publication.
- Remaining risk: outstanding acceptance and design decisions remain active;
  CLI pin/export limits and unrelated repository formatting remain recorded.
