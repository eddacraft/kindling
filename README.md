# kindling

**Local memory for AI-assisted development**

You switch sessions. Your AI starts from zero. kindling remembers what happened: tool calls, file edits, commands, errors, decisions, and pinned findings. Everything stays on your machine in SQLite. Search it later with deterministic, explainable results.

[![crates.io](https://img.shields.io/crates/v/eddacraft-kindling.svg)](https://crates.io/crates/eddacraft-kindling)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

**Full guide:** [docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)

## Try it in 60 seconds

No Node.js or Rust required:

```bash
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
kindling demo
kindling search "JWT" --all-repos --db ~/.kindling/demo/kindling.db
kindling browse --db ~/.kindling/demo/kindling.db
```

`kindling demo` loads sample memory (a JWT auth debugging session) into its own database at `~/.kindling/demo/kindling.db`, so it never mixes with your real project memory. `kindling browse` opens a local HTML viewer. No adapter needed to see how it works.

## Integrations

| Tool                            | Status    | How                                                                                      |
| ------------------------------- | --------- | ---------------------------------------------------------------------------------------- |
| **Plain CLI**                   | Supported | `kindling log`, `search`, `browse`, `demo`                                               |
| **Claude Code**                 | Supported | Built-in hooks; [plugin](./plugins/kindling-claude-code)                                 |
| **VS Code / Cursor / Windsurf** | Supported | VSIX: [Cursor and Windsurf quickstart](./docs/quickstart/cursor-windsurf.md)             |
| **OpenCode**                    | Supported | [`kindling-adapter-opencode`](./packages/kindling-adapter-opencode) (from source)        |
| **PocketFlow**                  | Supported | [`kindling-adapter-pocketflow`](./packages/kindling-adapter-pocketflow) (from source)    |
| **Rust SDK**                    | Supported | [`kindling-client`](https://crates.io/crates/kindling-client)                            |
| **Embedded Rust runtime**       | Supported | [`kindling-runtime`](./crates/kindling-runtime) (daemon in-process, no CLI install)      |
| **Node client**                 | Supported | [`packages/kindling`](./packages/kindling) (from source)                                 |
| **Custom adapter**              | Supported | [Cookbook](./docs/adapters/cookbook.md) · [minimal example](./examples/adapter-minimal/) |

Full matrix: [docs/integrations.md](./docs/integrations.md). No Claude Code? [Quickstart without Claude Code](./docs/quickstart/without-claude-code.md).

## What you get

- **Local-first:** project memory lives in SQLite on your machine.
- **Deterministic retrieval:** pins, current summaries, and ranked hits return in a predictable order, with provenance.
- **Works with your stack:** CLI, Rust SDK, thin Node client, and adapters for popular editors and agents.

## Quick start: Claude Code

Install the binary, which the plugin's hooks call:

```bash
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
# or: cargo install eddacraft-kindling
```

Then, inside Claude Code, add this repository's marketplace and install the plugin:

```text
/plugin marketplace add eddacraft/kindling
/plugin install kindling@kindling-plugins
```

`kindling init --claude-code` checks this setup and prints the same commands. kindling captures tool calls, file edits, commands, and errors across sessions. Search with `kindling search` or the plugin's `/memory` commands.

Plugin: [plugins/kindling-claude-code](./plugins/kindling-claude-code)

## Install

| Channel                     | Command                                                                                 | Notes                                                                                  |
| --------------------------- | --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| **Installer (recommended)** | `curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh \| sh` | Prebuilt binary; Linux, macOS, Windows (Git Bash/MSYS)                                 |
| **Cargo**                   | `cargo install eddacraft-kindling`                                                      | Crate name is `eddacraft-kindling`; the binary is `kindling`                           |
| **Homebrew (macOS/Linux)**  | `brew install eddacraft/tap/kindling`                                                   | Same tap as anvil: [eddacraft/homebrew-tap](https://github.com/eddacraft/homebrew-tap) |
| **GitHub Release**          | [Releases](https://github.com/eddacraft/kindling/releases)                              | Prebuilt archives with `.sha256` sidecars, plus the editor extension VSIX              |

```bash
kindling init          # create the local database
kindling demo          # load sample memory into ~/.kindling/demo/ (try before you capture)
kindling serve         # start the daemon (auto-spawned by clients)
kindling search "auth" # search captured context
kindling browse        # open local HTML viewer
```

Setup guide: [docs.eddacraft.ai/kindling/quickstart/install](https://docs.eddacraft.ai/kindling/quickstart/install)

kindling is not published to npm. The `@eddacraft/kindling` packages on npm are the retired 0.1.x TypeScript engine and are deprecated; install the binary with one of the channels above instead.

### Node.js client

[`packages/kindling`](./packages/kindling) is a thin TypeScript client over the Rust daemon (HTTP/1 over a Unix domain socket). It auto-spawns `kindling serve` on first use. It is not published to npm: it ships bundled inside the editor extension, and you can use it from a clone of this repository (see [Development](#development)).

### Rust SDK

```toml
[dependencies]
kindling-client = "0.4"     # daemon-backed; default for integrations
# kindling-runtime = "0.4"  # bundles the daemon in your binary; no `kindling` on PATH needed
# kindling-service = "0.4"  # embedded, in-process, zero IPC
```

## How it works

**Observations** are atomic captures: tool calls, commands, file diffs, errors, messages, and workflow events.

**Capsules** group observations into bounded units (a session, a workflow node).

Search returns three tiers: **pins** (always first), **current summary**, then **provider hits** (ranked full-text results with provenance).

## CLI reference

```bash
kindling init                              # create the database (--claude-code to check hooks)
kindling demo                              # load sample memory (own DB; pass --db to search it)
kindling browse                            # open HTML viewer
kindling log "root cause: cache eviction"
kindling search "authentication error"     # scoped to the current git repo
kindling search "authentication error" --all-repos
kindling list capsules                     # capsules | pins | observations
kindling status                            # database path and health
kindling stats                             # counts by kind, capsules, pins, activity
kindling pin observation obs_abc --note "Root cause"
kindling forget obs_abc                    # redact an observation
kindling capsule open --intent "debug leak"
kindling export ./backup.json              # and: kindling import ./backup.json
kindling serve                             # daemon (clients auto-spawn it)
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

Rust is the engine. Adapters and the CLI reach project memory through the daemon (`kindling serve`), which owns the SQLite store and serialises concurrent access. The daemon listens on a Unix domain socket (`~/.kindling/kindling.sock`, mode `0600`; loopback TCP with a per-daemon secret on Windows) and keeps one database per project under `~/.kindling/projects/`.

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
- [Cursor and Windsurf](./docs/quickstart/cursor-windsurf.md)
- [Integrations matrix](./docs/integrations.md)
- [Adapter cookbook](./docs/adapters/cookbook.md)
- [Install](https://docs.eddacraft.ai/kindling/quickstart/install)
- [Core concepts](https://docs.eddacraft.ai/kindling/concepts/capsules)
- [Use cases](./docs/use-cases.md)
- [Vision and direction](./VISION.md)

## Development

This section is for working on kindling itself. Read [VISION.md](VISION.md) and
[CONTRIBUTING.md](CONTRIBUTING.md#what-we-accept) before starting anything
substantial.

### Prerequisites

| Tool                  | Version                                                                     | Needed for                                                                       |
| --------------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| **Git**               | any recent                                                                  | everything                                                                       |
| **Rust via `rustup`** | pinned to 1.95.0 in [`rust-toolchain.toml`](rust-toolchain.toml); MSRV 1.89 | the engine crates and the `kindling` binary (most work)                          |
| **C compiler**        | `cc`/`clang`/`gcc` (Xcode CLT on macOS, MSVC/MinGW on Windows)              | building the bundled SQLite (`rusqlite` with `bundled`); no system SQLite needed |
| **Node.js**           | 24 (see [`.nvmrc`](.nvmrc))                                                 | the thin TS client, adapters, Claude Code plugin tests, and git hooks            |
| **pnpm**              | 10 (lockfile v9; `corepack enable` works)                                   | same as Node.js                                                                  |
| direnv _(optional)_   | any                                                                         | puts the repo's planning CLI (`.aps/bin`) on `PATH` via [`.envrc`](.envrc)       |

Install `rustup` from [rustup.rs](https://rustup.rs) rather than a distro Rust
package: the first `cargo` command in the repo then installs the pinned
toolchain with `rustfmt` and `clippy`. If you only touch Rust, you can skip
Node.js and pnpm, but you won't get the pre-commit hooks.

Platforms: Linux and macOS are the primary development targets. Windows builds
and runs the daemon over loopback TCP; some Unix-socket tests are `cfg(unix)`
and are skipped there.

### First build

```bash
git clone https://github.com/eddacraft/kindling.git
cd kindling

# Rust engine + CLI
cargo build
cargo test

# TypeScript client, adapters, plugin, example (also installs git hooks)
pnpm install
pnpm run build
pnpm run test
```

The TS integration tests (thin client, adapters, the `/memory` plugin commands,
the minimal adapter example) run against a real daemon only when
`target/debug/kindling` exists, otherwise they skip. Run `cargo build` before
`pnpm run test` to exercise them; for the plugin and example suites also set
`KINDLING_BIN=$PWD/target/debug/kindling`.

### Running your local build

```bash
cargo run -p eddacraft-kindling -- --help

# Keep experiments out of your real memory store:
export KINDLING_DB_PATH=/tmp/kindling-dev.db
cargo run -p eddacraft-kindling -- log "trying out a local build"
cargo run -p eddacraft-kindling -- search "local build"

# Sample data (demo always uses its own DB unless you pass --db)
cargo run -p eddacraft-kindling -- demo --db /tmp/kindling-demo.db

# Daemon on a scratch socket
cargo run -p eddacraft-kindling -- serve --socket /tmp/kindling-dev.sock
```

By default kindling stores data under `~/.kindling` (one database per project,
plus the daemon socket). `--db` and `KINDLING_DB_PATH` override the database
path; `serve --socket` / `--kindling-home` override the daemon location.

### Repository layout

```
crates/
  kindling/           the `kindling` CLI + Claude Code hooks (crate: eddacraft-kindling)
  kindling-client/    daemon-backed Rust SDK (HTTP/1 over UDS); must stay dependency-light
  kindling-runtime/   embeddable facade: in-process daemon + client + durable spool
  kindling-service/   in-process orchestration (embedded, zero IPC)
  kindling-server/    daemon runtime (`kindling serve`), per-project routing
  kindling-store/     SQLite persistence (FTS5 + WAL)
  kindling-provider/  deterministic retrieval (FTS5 BM25 + recency)
  kindling-types/     shared domain types (+ optional ts-rs bindings)
  kindling-bench/     latency, throughput, and resource benchmarks
packages/             private TS packages: thin client + OpenCode, PocketFlow, VS Code adapters
plugins/              Claude Code plugin (hooks + /memory commands)
examples/             minimal custom adapter
schema/               cross-language schema contract (schema.sql, version.json)
docs/                 architecture, data model, retrieval contract, guides, ADRs
scripts/              release, publishing, schema sync, demo recording
```

Start with [`docs/architecture.md`](docs/architecture.md),
[`docs/data-model.md`](docs/data-model.md), and
[`docs/retrieval-contract.md`](docs/retrieval-contract.md).

### Checks before you push

CI runs these on every PR to `main`; run them locally first:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test

# If you touched packages/, plugins/ or examples/
pnpm run build && pnpm run type-check && pnpm run lint && pnpm run test
pnpm run format:check
```

Generated files are checked for drift. If you change one of these, regenerate
and commit the result:

| You changed                       | Run                                                                                              |
| --------------------------------- | ------------------------------------------------------------------------------------------------ |
| a type in `crates/kindling-types` | `cargo test -p kindling-types --features ts-rs && node packages/kindling/scripts/sync-types.mjs` |
| `schema/`                         | `./scripts/sync-vendored-schema.sh` (schema changes are discuss-first and maintainer-led)        |

`pnpm install` sets up [husky](https://typicode.github.io/husky/) hooks:
pre-commit runs `type-check` and `lint-staged` (oxlint + oxfmt), and
commit-msg rejects AI-tool attribution trailers. Use
[conventional commit](https://www.conventionalcommits.org/) prefixes
(`feat:`, `fix:`, `docs:` …) and open PRs against `main`.

Performance work uses the Criterion and resource-profiling harness in
[`crates/kindling-bench`](./crates/kindling-bench/README.md); include before
and after numbers.

## Contributing

Read [VISION.md](VISION.md) for where kindling is heading, then
[CONTRIBUTING.md](CONTRIBUTING.md#what-we-accept) for which changes are welcome,
which need discussion first, and which are not the direction. Security issues:
[SECURITY.md](SECURITY.md).

## Licence

Apache 2.0. See [LICENSE](LICENSE).

---

Built by [eddacraft](https://eddacraft.ai). kindling captures what happened; [anvil](https://eddacraft.ai) adds governed plans and quality gates for teams that want that layer on top.
