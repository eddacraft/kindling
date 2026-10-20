# kindling vision

This document says what kindling is trying to become and how to judge whether a
change moves it there. It is a direction, not a delivery plan: it makes no
promises about dates or scope. For what ships today, see the
[README](README.md), [integrations matrix](docs/integrations.md), and
[architecture](docs/architecture.md). For how to propose work, see
[CONTRIBUTING.md](CONTRIBUTING.md).

## North star

> kindling makes useful development context survive the boundaries where AI
> tools forget, locally, explainably, and under the developer's control.

The boundaries are the places where context is lost today: a new session, a
different tool, a context window that filled up, a workflow node that failed,
or a developer returning to a project after a week away.

## Principles

These are the tests a proposal is measured against.

1. **Capture once; recall across tools and sessions.** Context captured by one
   integration (Claude Code hooks, an editor, an agent framework, the CLI)
   should be retrievable from any other on the same project.
2. **Local-first means no account, no cloud dependency, and no hidden egress.**
   Memory lives in SQLite on the developer's machine. kindling does not phone
   home and does not need a network to work.
3. **Deterministic and explainable before clever.** The same query against the
   same store returns the same results in the same order, and every result
   carries provenance. Ranking stays inspectable (FTS5 BM25 plus recency
   today). Any smarter retrieval must keep that property or sit beside it,
   not replace it.
4. **Trust matters more than capture volume.** Users can inspect, redact,
   forget, export, and diagnose what kindling retains. Secrets are masked at
   the service boundary before they are stored. Capturing less, correctly, is
   better than capturing everything.
5. **Rust is the canonical engine; clients and adapters stay thin.** There is
   one engine. The TypeScript client, adapters, and SDKs reach it through the daemon
   or the embedded service and do not reimplement storage or retrieval.
6. **Mechanisms, not policy.** kindling provides memory mechanisms: capture,
   capsules, pins, retrieval, redaction, and export. Deciding what is
   authoritative, approved, or promoted belongs to the tools built on top.
7. **Small inspection experiences support the memory loop.** Tools such as
   `kindling browse` and `kindling search` exist so people can see and trust
   their memory. A hosted application or general-purpose dashboard is not the
   product.

## Intended users

- **Developers using AI coding tools** who lose context between sessions or
  between tools and want it back without sending their code anywhere.
- **Integration authors** building adapters or agents who need a small, stable,
  local memory API (`kindling-client`, `kindling-service`, or
  `@eddacraft/kindling`).
- **Workflow builders** (for example PocketFlow users) who want a durable,
  explainable record of what each node saw and produced.

kindling is designed for individual developers and small teams working on their
own machines. It is not an enterprise observability platform or a shared
multi-tenant memory service. See
[When kindling does not fit](docs/use-cases.md#when-kindling-does-not-fit).

## What success looks like

These are the measures we use to judge progress. They describe outcomes, not
deadlines.

- **Time to first recall:** a new user can install the binary and retrieve
  context from sample memory (`kindling demo`, then `kindling search`) in about
  a minute, with no account and no Node.js or Rust toolchain.
- **Cross-tool recall:** context captured through one supported integration is
  retrievable through another on the same project, with provenance showing
  where it came from.
- **Reproducibility:** retrieval for a fixed store and query returns the same
  results in the same order across runs, and the ordering can be explained from the documented
  [retrieval contract](docs/retrieval-contract.md).
- **No egress:** normal operation makes no network calls. A user can verify
  this.
- **User control:** everything kindling retains can be listed, inspected,
  forgotten, and exported with the shipped CLI.
- **Thin integrations:** a new adapter can be written against the documented
  contract ([cookbook](docs/adapters/cookbook.md)) without engine changes.
- **Predictable cost:** capture and retrieval stay fast enough to sit in an
  agent's hot path, measured by the
  [benchmark harness](crates/kindling-bench/README.md) rather than asserted.

## Non-goals

kindling will not become:

- a hosted, cloud, or multi-tenant memory service;
- a governance, review, approval, or promotion workflow;
- a retrieval system whose ranking is opaque or model-only;
- a second, parallel engine in another language;
- a general agent framework, orchestrator, or IDE;
- a general-purpose dashboard or hosted UI.

These are not judgements on those products. They are outside what kindling is
for, and adding them would weaken the principles above.

## Boundary with downstream products

kindling is a standalone open-source engine. It is useful on its own and does
not require, or favour, any downstream product.

Products built on kindling, such as eddacraft's [anvil](https://eddacraft.ai),
add their own policy layers (governed plans, quality gates) on top of
kindling's public APIs. When a downstream product needs something from
kindling, it lands here only as a general mechanism that any integration could
use, and it follows the same contribution process as any other change.
Downstream policy stays downstream.
