# kindling docs

The published user docs are at
[docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview). This
directory holds the in-repo developer and reference docs.

## Start here

| If you want to…                         | Read                                                                         |
| --------------------------------------- | ---------------------------------------------------------------------------- |
| Learn the vocabulary                    | [CONTEXT.md](../CONTEXT.md)                                                  |
| Understand how the engine fits together | [architecture.md](architecture.md)                                           |
| Make a change to the engine             | [development.md](development.md), then [CONTRIBUTING.md](../CONTRIBUTING.md) |
| Know where the project is headed        | [VISION.md](../VISION.md), [plans/index.aps.md](../plans/index.aps.md)       |

## Reference

- [architecture.md](architecture.md): crates, the daemon model, the v1 API, data
  flow, configuration, and non-goals.
- [data-model.md](data-model.md): observations, capsules, summaries, pins, and
  scope, with field-level detail.
- [retrieval-contract.md](retrieval-contract.md): retrieval tiers, scoring, and
  determinism guarantees.
- [../schema/README.md](../schema/README.md): the SQLite schema contract and how
  to migrate it.
- Crate READMEs: `crates/*/README.md`.

## Guides

- [development.md](development.md): repo map, drift gates, tests, invariants,
  and change recipes.
- [integrations.md](integrations.md): which integration to use.
- [adapters/cookbook.md](adapters/cookbook.md): building a custom adapter.
- [quickstart/without-claude-code.md](quickstart/without-claude-code.md) and
  [quickstart/cursor-windsurf.md](quickstart/cursor-windsurf.md).
- [use-cases.md](use-cases.md) and [pocketflow-capabilities.md](pocketflow-capabilities.md).
- Process: [branching-strategy](guides/branching-strategy.md),
  [worktree-policy](guides/worktree-policy.md), and
  [release-runbook](guides/release-runbook.md).

## Decisions

- [adr/](adr/): architecture decision records, such as
  [0001 keep SQLite](adr/0001-keep-sqlite-over-duckdb.md).
- `D-0xx` decisions are recorded in [plans/index.aps.md](../plans/index.aps.md#decisions).
  Design specs are in [plans/specs/](../plans/specs/).

Write an ADR here for a lasting technical choice: a storage engine, a transport,
a wire format. Record roadmap and scope decisions as `D-0xx` entries in the APS
index.

## Historical and background

These files are kept for context. They may describe the retired TypeScript
engine.

- [plans/](plans/): Node-era implementation plans (Feb 2026).
- [research/](research/): background research.
- [superpowers/specs/](superpowers/specs/): early design specs, such as the
  recall skill.
- [releases/](releases/): per-release notes. [CHANGELOG.md](../CHANGELOG.md) is
  the canonical record.
- [third-party/](third-party/): notes on vendored code.
