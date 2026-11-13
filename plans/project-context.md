# Project Context

User-owned APS context, reconciled 2026-11-13.

## Overview

kindling is a local memory and continuity engine. Rust is canonical; eight
publishable workspace crates implement types, SQLite storage, retrieval, service,
daemon, client, runtime facade and the eddacraft-kindling binary. Thin TypeScript
source packages support adapters; npm distribution is retired by D-010.

## Team and authority

Single maintainer, @aneki. APS is the planning authority; GitHub carries bugs and
public discussion. Draft and Blocked work does not authorise implementation.
Follow AGENTS.md and the repository branching/worktree guides for execution.

## Planning and archive conventions

- [index.aps.md](./index.aps.md) lists active modules and next decisions.
- [completed.aps.md](./completed.aps.md) indexes completed task detail under
  `plans/archive/modules/`, following the sibling anvil-001 layout.
- A merge is not completion: require release inclusion or verified live internal
  use plus acceptance evidence. Preserve unmet acceptance in the active module.
- Split delivered subsets from mixed modules; preserve IDs and original receipts.
  Record satisfied prerequisites under Delivered dependencies with archive links;
  keep Dependencies for unmet work. The native selector does not load archive
  items. Do not recreate completed prerequisites as new executable tasks.
- Superseded items are historical, excluded from completion totals. Archived
  outcomes and commands describe their delivery-time layout, not current setup.
- Keep later defects open separately; do not silently narrow scope to archive an
  incomplete item. The intent-event contract is blocked on GH-215.
- Specs, execution plans and post-merge reviews are retained technical rationale;
  their dated checklists do not override the current module status.
- Feature branches do not edit APS statuses or plan files; they list completed
  and discovered work in the PR description. A separate **bookkeeping pass**,
  in its own PR, updates statuses and plan files after work merges.

## Current delivery and validation

Latest published release: v0.4.1 (2026-11-10). See the
[reconciliation report](./reconciliation/2026-11-13-aps-cleanup.md) for ancestry,
registry and archive evidence. The deferred 1.0.0 declaration is still a decision.

For plan-only changes use `aps lint`, `pnpm exec oxfmt --check plans`, local link
checks and `git diff --check`. There is no repository docs/date-normalisation
script. The project APS pin (0.3.0) differs from the installed CLI (0.9.0);
`aps doctor` reports migration warnings. Do not call non-strict lint a pinned
CLI verification or execute removed TypeScript validation commands from archives.
