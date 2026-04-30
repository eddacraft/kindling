# Contributing to Kindling

Thank you for your interest in contributing to Kindling! We welcome contributions from the community.

## Getting Started

### Prerequisites

- Node.js >= 20.0.0
- pnpm >= 8.0.0

### Development Setup

```bash
git clone https://github.com/EddaCraft/kindling.git
cd kindling
pnpm install
pnpm build
```

### Running Tests

```bash
# Run all tests
pnpm test

# Run tests for a specific package
cd packages/kindling-core
pnpm test

# Watch mode
pnpm test:watch
```

### Building

```bash
# Build all packages
pnpm build

# Type-check without emitting
pnpm type-check

# Clean build artifacts
pnpm clean
```

## Code Style

- **TypeScript** for all source code
- **Explicit types** for public APIs
- **Descriptive names** - clarity over brevity
- **Small, focused functions** - single responsibility
- **Tests alongside implementation** - high coverage for core functionality
- **ESM only** - use `.js` extensions in imports

## Branching Model

Kindling uses a two-branch model that supports multiple active streams in
parallel worktrees:

- `main` is the stable release branch. Always publishable to npm.
- `dev` is the active integration branch and the default branch.
- normal feat, fix, docs, and chore branches are created from `dev`.
- hotfix branches are created from `main` or the active `release/*` branch.

Keep `main` and `dev` as the only permanent worktrees. Treat all other
worktrees as disposable and remove them once the branch is merged, replaced,
or paused.

Release guidance:

- small releases may promote directly from `dev` to `main`
- larger releases should use a short-lived `release/*` branch
- any fix that lands during release stabilisation must be merged back to `dev`
  immediately after release

See the detailed guides for the full policy:

- [`docs/guides/branching-strategy.md`](docs/guides/branching-strategy.md)
- [`docs/guides/worktree-policy.md`](docs/guides/worktree-policy.md)
- [`docs/guides/release-runbook.md`](docs/guides/release-runbook.md)

## Pull Request Process

1. **Open an issue first** for significant changes to discuss approach
2. **Create a branch from `dev`** for normal work, or from `main` only for
   production hotfixes
3. **Write tests** for new functionality
4. **Update documentation** if behavior changes
5. **Keep PRs focused** — one logical change per PR
6. **Run the full test suite**: `pnpm build && pnpm test && pnpm lint`
7. **Ensure CI passes** before requesting review
8. **Target `dev`** — feature PRs should target `dev`, not `main`. Only
   release-promotion and hotfix PRs target `main`.

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

Kindling is infrastructure for local memory and continuity. Contributions should align with this scope.

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
- Cloud/server deployment modes
- Semantic/embedding-based retrieval (planned for Phase 2)
- UI components

If you're unsure whether something is in scope, open an issue to discuss before investing time.

### Feature Requests

For net-new functionality, start with a design conversation. Open an issue describing:

- The problem you're solving
- Your proposed approach (optional)
- Why it belongs in Kindling

The maintainers will help decide whether it should move forward. Please wait for approval before opening a feature PR.

## Project Structure

```
kindling/
├── packages/
│   ├── kindling/               # Main package (core + sqlite + provider + server)
│   ├── kindling-core/          # Domain types & KindlingService
│   ├── kindling-store-sqljs/   # Browser/WASM store
│   ├── kindling-adapter-opencode/
│   ├── kindling-adapter-pocketflow/
│   ├── kindling-adapter-claude-code/
│   └── kindling-cli/           # CLI tools
├── docs/                       # Documentation
└── plans/                      # Planning documents (APS)
```

## Questions?

- **Issues**: [GitHub Issues](https://github.com/EddaCraft/kindling/issues)
- **Discussions**: Open an issue for questions about contributing

## License

By contributing, you agree that your contributions will be licensed under the [Apache-2.0 License](LICENSE).
