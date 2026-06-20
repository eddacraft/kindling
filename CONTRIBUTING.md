# Contributing to kindling

Thank you for your interest in contributing to kindling! We welcome contributions from the community.

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
│   └── kindling-types/     #   shared domain types (+ ts-rs bindings)
├── packages/               # npm: thin @eddacraft/kindling client + adapters
├── schema/                 # cross-language schema contract (schema.sql, version.json)
├── docs/                   # documentation
└── plans/                  # planning documents (APS)
```

> The TypeScript implementation packages (`-core`, `-store-sqlite`,
> `-store-sqljs`, `-provider-local`, `-server`, `-cli`) are **deprecated** and
> will be removed at 1.0.0. New feature work targets the Rust crates.

## Getting Started

### Prerequisites

- Rust (stable) — the toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` installs it automatically.
- Node.js >= 20.0.0 and pnpm >= 8.0.0 — only needed to work on the npm thin client / adapters.

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

1. **Open an issue first** for significant changes to discuss approach
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

## Scope Guardrails

kindling is infrastructure for local memory and continuity. Contributions should align with this scope.

### In Scope

- Observation capture and storage
- Capsule lifecycle management
- Retrieval (FTS, recency, deterministic ranking)
- Export/import and portability
- Adapter integrations (OpenCode, PocketFlow, etc.)
- CLI tooling for inspection and debugging
- Performance and reliability improvements
- Documentation and examples

### Out of Scope

These belong to downstream systems and will not be accepted:

- Governance workflows (review, approval, promotion)
- MemoryObject lifecycle management
- Multi-user access control and permissions
- Cloud / hosted deployment modes (the local daemon is in scope; remote/multi-tenant hosting is not)
- Semantic/embedding-based retrieval (planned for a later phase)
- UI components

If you're unsure whether something is in scope, open an issue to discuss before investing time.

### Feature Requests

For net-new functionality, start with a design conversation. Open an issue describing:

- The problem you're solving
- Your proposed approach (optional)
- Why it belongs in kindling

The maintainers will help decide whether it should move forward. Please wait for approval before opening a feature PR.

For the repository layout, see [Project layout](#project-layout) above.

## Questions?

- **Issues**: [GitHub Issues](https://github.com/eddacraft/kindling/issues)
- **Discussions**: Open an issue for questions about contributing

## License

By contributing, you agree that your contributions will be licensed under the [Apache-2.0 License](LICENSE).
