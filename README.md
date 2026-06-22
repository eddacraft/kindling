# kindling

**Local memory for AI-assisted development**

You switch sessions. Your AI starts from zero. kindling remembers what happened: tool calls, file edits, commands, errors, decisions, and pinned findings. Everything stays on your machine in SQLite. Search it later with deterministic, explainable results.

[![crates.io](https://img.shields.io/crates/v/eddacraft-kindling.svg)](https://crates.io/crates/eddacraft-kindling)
[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling.svg)](https://www.npmjs.com/package/@eddacraft/kindling)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

**Full guide:** [docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)

## Try it in 60 seconds

No Node.js or Rust required:

```bash
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
kindling demo
kindling search "JWT"
kindling browse
```

`kindling demo` loads sample memory (a JWT auth debugging session). `kindling browse` opens a local HTML viewer. No adapter needed to see how it works.

> **Terminal demo:** run `./scripts/record-demo.sh`, or record with `asciinema rec -c "./scripts/record-demo.sh" docs/assets/kindling-demo.cast` and embed the cast in this README.

## Integrations

| Tool                            | Status    | How                                                                                      |
| ------------------------------- | --------- | ---------------------------------------------------------------------------------------- |
| **Plain CLI**                   | Supported | `kindling log`, `search`, `browse`, `demo`                                               |
| **Claude Code**                 | Supported | Built-in hooks; [plugin](./plugins/kindling-claude-code)                                 |
| **VS Code / Cursor / Windsurf** | Supported | [`@eddacraft/kindling-adapter-vscode`](./packages/kindling-adapter-vscode)               |
| **OpenCode**                    | Supported | [`@eddacraft/kindling-adapter-opencode`](./packages/kindling-adapter-opencode)           |
| **PocketFlow**                  | Supported | [`@eddacraft/kindling-adapter-pocketflow`](./packages/kindling-adapter-pocketflow)       |
| **Rust SDK**                    | Supported | [`kindling-client`](https://crates.io/crates/kindling-client)                            |
| **Node client**                 | Supported | [`@eddacraft/kindling`](./packages/kindling)                                             |
| **Custom adapter**              | Supported | [Cookbook](./docs/adapters/cookbook.md) · [minimal example](./examples/adapter-minimal/) |

Full matrix: [docs/integrations.md](./docs/integrations.md). No Claude Code? [Quickstart without Claude Code](./docs/quickstart/without-claude-code.md).

## What you get

- **Local-first:** project memory lives in SQLite on your machine.
- **Deterministic retrieval:** pins, current summaries, and ranked hits return in a predictable order, with provenance.
- **Works with your stack:** CLI, Rust SDK, thin Node client, and adapters for popular editors and agents.

## Quick start: Claude Code

```bash
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
# or: cargo install eddacraft-kindling && kindling init --claude-code
```

kindling captures tool calls, file edits, commands, and errors across sessions. Search with `kindling search` or the plugin's `/memory` commands.

Plugin: [plugins/kindling-claude-code](./plugins/kindling-claude-code)

## Install

| Channel                     | Command                                                                                 | Notes                                                            |
| --------------------------- | --------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| **Installer (recommended)** | `curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh \| sh` | Prebuilt binary; Linux, macOS, Windows                           |
| **Cargo**                   | `cargo install eddacraft-kindling`                                                      | Crate name is `eddacraft-kindling`; the binary is `kindling`     |
| **Homebrew (macOS)**        | `brew install eddacraft/tap/kindling`                                                   | See [packaging/homebrew-tap](./packaging/homebrew-tap/README.md) |
| **npm**                     | `npm install @eddacraft/kindling`                                                       | Thin client; bundles a matching prebuilt binary                  |

```bash
kindling init          # create the local database
kindling demo          # load sample memory (try before you capture)
kindling serve         # start the daemon (auto-spawned by clients)
kindling search "auth" # search captured context
kindling browse        # open local HTML viewer
```

Setup guide: [docs.eddacraft.ai/kindling/quickstart/install](https://docs.eddacraft.ai/kindling/quickstart/install)

### Node.js client

`@eddacraft/kindling` is a thin TypeScript client over the Rust daemon (HTTP/1 over a Unix domain socket). It auto-spawns `kindling serve` on first use.

### Rust SDK

```toml
[dependencies]
kindling-client = "0.2"   # daemon-backed; default for integrations
# kindling-service = "0.2"  # embedded, in-process, zero IPC
```

## How it works

**Observations** are atomic captures: tool calls, commands, file diffs, errors, messages, and workflow events.

**Capsules** group observations into bounded units (a session, a workflow node).

Search returns three tiers: **pins** (always first), **current summary**, then **provider hits** (ranked full-text results with provenance).

## CLI reference

```bash
kindling demo                              # load sample memory
kindling browse                            # open HTML viewer
kindling log "root cause: cache eviction"
kindling search "authentication error"
kindling pin observation obs_abc --note "Root cause"
kindling capsule open --intent "debug leak"
kindling export ./backup.json
kindling serve
```

Full CLI reference: [docs.eddacraft.ai/kindling/reference/cli](https://docs.eddacraft.ai/kindling/reference/cli)

## Programmatic usage

```rust
use kindling_client::{Client, CapsuleType, RetrieveOptions, ScopeIds};

#[tokio::main]
async fn main() -> Result<(), kindling_client::ClientError> {
    let client = Client::new()?;
    let scope = ScopeIds { session_id: Some("session-1".into()), ..Default::default() };
    let capsule = client
        .open_capsule(CapsuleType::Session, "debug auth", scope.clone(), None)
        .await?;
    let results = client
        .retrieve(RetrieveOptions { query: "token".into(), scope_ids: scope, ..Default::default() })
        .await?;
    client.close_capsule(&capsule.id, Default::default()).await?;
    Ok(())
}
```

Adapter guide: [docs/adapters/cookbook.md](./docs/adapters/cookbook.md)

## Architecture

Rust is the engine. Adapters and the CLI reach project memory through the daemon (`kindling serve`), which owns the SQLite store and serialises concurrent access.

```diagram
        Adapters / hooks                CLI
  ┌──────────────────────────┐   ┌──────────────┐
  │ VS Code · Claude Code ·    │   │  kindling    │
  │ OpenCode · PocketFlow      │   │  <command>   │
  └────────────┬─────────────┘   └──────┬───────┘
               │  kindling-client       │
               ▼                        ▼
        kindling-server          kindling-service
        (daemon)                 (embedded)
               └────────┬─────────┘
                        ▼
                 SQLite + FTS5
```

Details: [`docs/architecture.md`](./docs/architecture.md)

## Documentation

- [Without Claude Code](./docs/quickstart/without-claude-code.md)
- [Integrations matrix](./docs/integrations.md)
- [Adapter cookbook](./docs/adapters/cookbook.md)
- [Install](https://docs.eddacraft.ai/kindling/quickstart/install)
- [Core concepts](https://docs.eddacraft.ai/kindling/concepts/capsules)
- [Use cases](./docs/use-cases.md)

## Development

```bash
git clone https://github.com/eddacraft/kindling.git && cd kindling
cargo build && cargo test
pnpm install && pnpm run build && pnpm run test
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) · [SECURITY.md](SECURITY.md)

## Licence

Apache 2.0. See [LICENSE](LICENSE).

---

Built by [eddacraft](https://eddacraft.ai). kindling captures what happened; [anvil](https://eddacraft.ai) adds governed plans and quality gates for teams that want that layer on top.
