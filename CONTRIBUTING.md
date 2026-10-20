# Contributing to kindling

Thank you for your interest in contributing to kindling. Before you start on
anything substantial, read [VISION.md](VISION.md) and
[What we accept](#what-we-accept) so your work fits the direction.

## Project layout

kindling is **Rust-canonical**. The engine lives in the Rust workspace under
[`crates/`](crates); the npm packages under [`packages/`](packages) are a thin
client over the Rust binary plus the adapters. Most contributions land in the
crates.

```
kindling/
├── crates/                 # Rust workspace (the canonical engine)
│   ├── kindling/           #   CLI binary + Claude Code hooks (kindling, kindling-hook)
│   ├── kindling-client/    #   daemon-backed Rust SDK (default for integrations)
│   ├── kindling-service/   #   in-process orchestration (embedded, zero-IPC)
│   ├── kindling-server/    #   daemon runtime (HTTP/1 over UDS)
│   ├── kindling-store/     #   SQLite persistence (FTS5 + WAL)
│   ├── kindling-provider/  #   deterministic local retrieval (FTS5 BM25 + recency)
│   ├── kindling-bench/     #   latency, throughput, and resource benchmark harness
│   └── kindling-types/     #   shared domain types (+ ts-rs bindings)
├── packages/               # npm: thin @eddacraft/kindling client + adapters
├── schema/                 # cross-language schema contract (schema.sql, version.json)
├── docs/                   # documentation
└── plans/                  # planning documents (APS)
```

> The old TypeScript implementation packages (`-core`, `-store-sqlite`,
> `-store-sqljs`, `-provider-local`, `-server`, `-cli`) have been removed from
> this repository and survive only as deprecated 0.1.x versions on npm. New
> feature work targets the Rust crates.

## Getting Started

### Prerequisites

- Rust (stable) — the toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` installs it automatically.
- Node.js >= 24 and pnpm >= 8.0.0 — only needed to work on the npm thin client / adapters.

### Development Setup

```bash
git clone https://github.com/eddacraft/kindling.git
cd kindling

# Rust workspace (canonical engine)
cargo build

# npm packages (thin client + adapters), optional
pnpm install
pnpm build
```

### Running Tests

```bash
# Rust: all crates
cargo test

# Rust: a single crate
cargo test -p kindling-provider

# Performance and resource profiles
cargo bench -p kindling-bench --bench workloads
cargo run -p kindling-bench --release -- --profile smoke --pretty

# Domain-type bindings (regenerates + checks the ts-rs projection)
cargo test -p kindling-types --features ts-rs

# npm packages
pnpm test
```

### Checks before pushing

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test

# If you touched the npm packages:
pnpm build && pnpm test
```

If you changed the schema, run `scripts/sync-vendored-schema.sh` — CI fails on
schema drift between `schema/` and the vendored copies. If you changed a type
that has a ts-rs binding, regenerate it (CI fails on binding drift).

## Code Style

- **Rust** is the primary language: `rustfmt` formatting, `clippy`-clean (warnings denied).
- **TypeScript** (thin client + adapters): explicit types for public APIs, ESM only — use `.js` extensions in imports.
- **Descriptive names** — clarity over brevity.
- **Small, focused functions** — single responsibility.
- **Tests alongside implementation** — high coverage for the engine crates.

## Branching Model

kindling uses a single permanent branch model with short-lived work branches:

- `main` is the default branch, integration branch, and stable release branch.
  Always releasable to crates.io and npm.
- normal feat, fix, docs, and chore branches are created from `main`.
- hotfix branches are created from `main` or the active `release/*` branch.

Keep `main` as the only permanent worktree. Treat all other worktrees as
disposable and remove them once the branch is merged, replaced, or paused.

Release guidance:

- small releases may tag directly from `main` after release prep lands
- larger releases should use a short-lived `release/*` branch cut from `main`

See the detailed guides for the full policy:

- [`docs/guides/branching-strategy.md`](docs/guides/branching-strategy.md)
- [`docs/guides/worktree-policy.md`](docs/guides/worktree-policy.md)
- [`docs/guides/release-runbook.md`](docs/guides/release-runbook.md)

## Pull Request Process

1. **Check the lane** in [What we accept](#what-we-accept); open an issue first for discuss-first work
2. **Create a branch from `main`** for normal work and production hotfixes
3. **Write tests** for new functionality
4. **Update documentation** if behavior changes
5. **Keep PRs focused** — one logical change per PR
6. **Run the checks**: `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test` (plus `pnpm build && pnpm test` if you touched the npm packages)
7. **Ensure CI passes** before requesting review
8. **Target `main`** — all PRs target `main` unless they are scoped to an
   active `release/*` branch.

### Commit Messages

Use clear, descriptive commit messages following conventional commits:

```
feat: add capsule auto-close on session timeout

Capsules now auto-close when the source provides a natural end signal
or after a configurable inactivity timeout. This prevents orphaned
capsules from accumulating.

Closes #42
```

Prefixes:

- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation only
- `refactor:` - Code change that neither fixes a bug nor adds a feature
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks

## What we accept

kindling's direction is set out in [VISION.md](VISION.md). Every proposal is
judged against it. To save you wasted effort, work falls into one of three
lanes. If you are unsure which lane applies, treat it as discuss first.

### Welcome

Open a pull request directly. An issue first is helpful but not required.

- Fixes for confirmed bugs, with a test that fails before the fix
- New or better tests, including regression and property tests
- Documentation that corrects the docs to match shipped behaviour
- Portability: platforms, shells, filesystems, install paths
- Benchmarks and measured performance improvements (use
  [`kindling-bench`](crates/kindling-bench/README.md) and include before and
  after numbers)
- Diagnostics: clearer errors, better `kindling status` output, logging that
  helps users debug their own setup
- Bringing an existing adapter into line with the documented adapter contract
  ([cookbook](docs/adapters/cookbook.md))

### Discuss first

Open an issue and wait for a maintainer to agree the approach before you write
the implementation. These changes affect contracts other people depend on.

- Schema changes or anything that alters persisted data
- Public API changes in the crates, the daemon HTTP API, or the npm client
- New observation kinds, capsule types, or scope fields
- Retrieval ranking or result ordering, including optional local semantic or
  embedding-based providers
- Retention, redaction, masking, or forget behaviour
- New adapters or integrations maintained in this repository
- New CLI commands or substantial changes to inspection UX such as
  `kindling browse`
- New dependencies with native code, network access, or a large footprint

### Not the direction

These conflict with the [non-goals](VISION.md#non-goals) and will be declined,
however well they are built:

- Hosted, cloud, or multi-tenant memory, or remote access to the daemon
- Governance workflows: review, approval, promotion, or authority over memory
- Multi-user access control and permissions
- Opaque or model-only ranking that replaces deterministic retrieval
- A parallel engine outside the Rust crates (clients and adapters stay thin)
- General agent frameworks, orchestrators, or a general-purpose dashboard
- Changes whose only purpose is to serve one downstream product's policy

A third-party tool that builds on kindling's public APIs is the right home for
most of these.

### Maintainer-led areas

Some work stays maintainer-led even when help is welcome. You can report,
discuss, and review, and a maintainer may invite you to implement part of it,
but a maintainer owns the design and the final change:

- **Security boundaries:** secret masking, socket permissions, path handling,
  and anything in [SECURITY.md](SECURITY.md) scope. Report vulnerabilities
  privately as SECURITY.md describes, never in a public issue or PR.
- **Persistent-data migrations:** changes to `schema/` and the stored format,
  because mistakes there damage user data that cannot be recovered.
- **Public contracts:** the schema contract, daemon API, published type
  bindings, and the [retrieval contract](docs/retrieval-contract.md).
- **Cross-repository changes:** anything that needs coordinated changes in
  another repository or a downstream product.
- **Releases and publishing** to crates.io, npm, and GitHub Releases.

### Examples

Each example maps to exactly one lane. Maintainer-led areas are noted where
they apply on top of the lane.

| Proposal                                                  | Lane              | Note                                                     |
| --------------------------------------------------------- | ----------------- | -------------------------------------------------------- |
| `kindling search` crashes on an empty store               | Welcome           |                                                          |
| Fix a README command that no longer works                 | Welcome           |                                                          |
| Make the installer work on a new Linux distribution       | Welcome           |                                                          |
| Add a benchmark profile for large stores                  | Welcome           |                                                          |
| Add a new output format to `kindling export`              | Discuss first     | Changes a user-facing contract                           |
| Add a new `test_result` observation kind                  | Discuss first     | Touches types, schema, and bindings                      |
| Change how recency is weighted in ranking                 | Discuss first     | Changes result ordering                                  |
| Add an optional local embedding provider                  | Discuss first     | Must keep deterministic retrieval available              |
| Add an adapter for a new editor or agent                  | Discuss first     | Could live in its own repository                         |
| Add a column to the observations table                    | Discuss first     | Maintainer-led migration                                 |
| Tighten secret masking for a new token format             | Discuss first     | Maintainer-led; report privately if secrets already leak |
| Sync memory to a hosted service                           | Not the direction |                                                          |
| Add an approval step before an observation is retrievable | Not the direction | Governance belongs downstream                            |
| Rank results with a remote LLM call                       | Not the direction | Egress and opaque ranking                                |
| Port the store to TypeScript for a Node-only install      | Not the direction | Parallel engine                                          |

### Writing a proposal

For discuss-first work, open an issue with:

- **Problem:** what goes wrong today, for whom, with a concrete example
- **Principle:** which part of [VISION.md](VISION.md) the change serves
- **Proposal:** the behaviour you want, including any change to a public
  contract, schema, or stored data
- **Compatibility:** what happens to existing stores, clients, and adapters,
  and whether a migration is needed
- **Evidence:** reproduction steps, benchmark numbers, or a prototype branch
  where relevant
- **Alternatives:** what else you considered, including doing it outside
  kindling

A maintainer will agree the approach, ask for changes, or decline with a
reason. Please wait for that before opening a feature PR. kindling is
maintained by a small team, so reviews happen as time allows and there is no
guaranteed response time.

For the repository layout, see [Project layout](#project-layout) above.

## Questions?

- **Issues**: [GitHub Issues](https://github.com/eddacraft/kindling/issues)
- **Discussions**: Open an issue for questions about contributing

## License

By contributing, you agree that your contributions will be licensed under the [Apache-2.0 License](LICENSE).
