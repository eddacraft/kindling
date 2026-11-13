<!-- This document indexes completed work; module archives own the detail. -->

# Completed Work Archive

| Scope          | Status   | Items | Reconciled |
| -------------- | -------- | ----- | ---------- |
| Delivered work | Complete | 44    | 2026-11-13 |

## Purpose and archive rule

This index follows the sibling anvil-001 pattern: historical task detail lives
under `plans/archive/modules/`; [index.aps.md](./index.aps.md) locates active work.
A merged change is archived as Complete only with release or live internal-use
evidence and no outstanding acceptance for that item. Mixed modules retain their
unfinished items. Later defects remain separately tracked; an archive receipt
records delivery, not a claim that the implementation is defect-free.

[The reconciliation report](./reconciliation/2026-11-13-aps-cleanup.md) records
all 87 dispositions and delivery evidence. There are 44 Complete, 14 Superseded
and 29 active items. Superseded work is listed separately and excluded from
completed counts. No current supported intent-event implementation was found;
KINTENT-001..006 remain blocked under GH-215.

## Release evidence

- [v0.4.0](https://github.com/eddacraft/kindling/releases/tag/v0.4.0), published
  2026-10-21, commit `a2704b4d0cce1447dd1e1124b6608cc49a331ac5`.
- [v0.4.1](https://github.com/eddacraft/kindling/releases/tag/v0.4.1), published
  2026-11-10, commit `1dc494883a55e6d4a0e883bd7d41469a7e4dbc18`;
  eight non-yanked workspace crates on crates.io, including kindling-runtime.
- [npm v0.1.2 publication](https://github.com/eddacraft/kindling/actions/runs/22067119341)
  succeeded; the ten retired npm package notices were read back on 2026-11-13.
  PUBLISH-001's PR #35 postdates that release and is included in v0.4.0.
- [anvil v0.9.0-beta](https://github.com/eddacraft/anvil-001/releases/tag/v0.9.0-beta),
  published 2026-07-12, contains both PORT-011 integration PRs.

v0.3.0 is a tag and crates.io release, not a published GitHub Release.
The v0.4.0/v0.4.1 binary/VSIX assets exist, while both release workflows have
an overall failure conclusion. Native acceptance gaps stay in active modules.

## Historical npm delivery

| Item        | Description         | Status   | Detail                                                    |
| ----------- | ------------------- | -------- | --------------------------------------------------------- |
| PUBLISH-001 | Merge open PRs      | Complete | [01-npm-publish](./archive/modules/01-npm-publish.aps.md) |
| PUBLISH-002 | Claim npm scope     | Complete | [01-npm-publish](./archive/modules/01-npm-publish.aps.md) |
| PUBLISH-003 | Package metadata    | Complete | [01-npm-publish](./archive/modules/01-npm-publish.aps.md) |
| PUBLISH-004 | Per-package READMEs | Complete | [01-npm-publish](./archive/modules/01-npm-publish.aps.md) |
| PUBLISH-005 | Publish pipeline    | Complete | [01-npm-publish](./archive/modules/01-npm-publish.aps.md) |
| PUBLISH-006 | CI release workflow | Complete | [01-npm-publish](./archive/modules/01-npm-publish.aps.md) |

## Schema contract included in v0.4.0

| Item       | Description                           | Status   | Detail                                                            |
| ---------- | ------------------------------------- | -------- | ----------------------------------------------------------------- |
| SCHEMA-001 | Extract schema to schema/schema.sql   | Complete | [04-schema-contract](./archive/modules/04-schema-contract.aps.md) |
| SCHEMA-002 | Set PRAGMA user_version in migrations | Complete | [04-schema-contract](./archive/modules/04-schema-contract.aps.md) |
| SCHEMA-003 | Document FTS5 tokenizer config        | Complete | [04-schema-contract](./archive/modules/04-schema-contract.aps.md) |
| SCHEMA-004 | Add schema/version.json               | Complete | [04-schema-contract](./archive/modules/04-schema-contract.aps.md) |
| SCHEMA-005 | Add schema/README.md                  | Complete | [04-schema-contract](./archive/modules/04-schema-contract.aps.md) |

## Rust implementation included in v0.4.0

| Item     | Description                                       | Status   | Detail                                                |
| -------- | ------------------------------------------------- | -------- | ----------------------------------------------------- |
| PORT-001 | Rust workspace scaffold                           | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-002 | kindling-types crate                              | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-003 | kindling-store crate                              | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-004 | kindling-filter crate                             | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-005 | kindling-provider crate                           | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-006 | kindling-service crate (in-process API)           | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-007 | kindling-server crate (UDS daemon)                | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-008 | kindling-client crate (Rust HTTP-over-UDS client) | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-009 | kindling-hook crate                               | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-012 | kindling-cli crate                                | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-013 | Umbrella `kindling` binary                        | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-016 | ts-rs export pipeline                             | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-017 | `@eddacraft/kindling` thin client SDK             | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |
| PORT-019 | Adapter packages cutover                          | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |

## Downstream contracts published through v0.4.0

| Item       | Description                                                  | Status   | Detail                                                                                          |
| ---------- | ------------------------------------------------------------ | -------- | ----------------------------------------------------------------------------------------------- |
| KINTEG-001 | Publish kindling 0.2.0 (client + spool) to crates.io         | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-002 | Daemon-side observation dedup (exactly-once-ish replay)      | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-003 | Structured read/query API over the daemon                    | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-004 | Capability handshake + machine-readable kind registry        | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-005 | Durable-emit observability + cold-start diagnostics          | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-006 | Redaction evidence in append responses                       | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-008 | `kindling-runtime` — anvil-first integration facade          | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-009 | Spool retention cap (size + age)                             | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-010 | Authenticate the TCP daemon transport (no per-user boundary) | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-011 | Make capsule close atomic when the summary is missing        | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-012 | Honor `--db` routing hint in daemon-backed commands          | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-013 | Keep outage spooling independent of backlog depth            | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |
| KINTEG-014 | Index deterministic scoped reads                             | Complete | [06-downstream-integration-surface](./archive/modules/06-downstream-integration-surface.aps.md) |

## Conversion improvements included in v0.4.0

| Item | Description                    | Status   | Detail                                                                  |
| ---- | ------------------------------ | -------- | ----------------------------------------------------------------------- |
| C1   | install.sh demo prompt         | Complete | [08-conversion-surface](./archive/modules/08-conversion-surface.aps.md) |
| C3   | Workspace auto-detect (search) | Complete | [08-conversion-surface](./archive/modules/08-conversion-surface.aps.md) |
| C4   | kindling stats                 | Complete | [08-conversion-surface](./archive/modules/08-conversion-surface.aps.md) |

## Public direction live on main and included in v0.4.0

| Item       | Description                                          | Status   | Detail                                                                                                |
| ---------- | ---------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------- |
| PUBDIR-001 | Establish the public vision and direction contract   | Complete | [09-public-direction-and-contribution](./archive/modules/09-public-direction-and-contribution.aps.md) |
| PUBDIR-002 | Define contribution acceptance lanes and checkpoints | Complete | [09-public-direction-and-contribution](./archive/modules/09-public-direction-and-contribution.aps.md) |

## anvil integration released in v0.9.0-beta

| Item     | Description             | Status   | Detail                                                |
| -------- | ----------------------- | -------- | ----------------------------------------------------- |
| PORT-011 | anvil integration proof | Complete | [05-rust-port](./archive/modules/05-rust-port.aps.md) |

## Superseded work (excluded from completed totals)

| Items         | Disposition                                                       | Detail                                                         |
| ------------- | ----------------------------------------------------------------- | -------------------------------------------------------------- |
| HOOK-001..008 | Superseded by PORT under D-004; not a second backlog              | [Rust hook plan](./archive/modules/02-rust-hook-binary.aps.md) |
| CLI-001..005  | Superseded by PORT under D-004; not a second backlog              | [Rust CLI plan](./archive/modules/03-rust-cli.aps.md)          |
| PORT-018      | Superseded by D-010 before any npm platform package was published | [Rust port archive](./archive/modules/05-rust-port.aps.md)     |
