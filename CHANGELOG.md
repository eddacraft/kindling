# Changelog

Customer- and integrator-relevant changes to kindling are documented here. For
implementation detail, maintenance, architecture and delivery evidence, see
[Engineering History](./ENGINEERING-HISTORY.md).

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

**Distribution:** kindling is Rust-canonical and ships via Cargo, Homebrew,
the installer and GitHub Release assets (including the editor VSIX). npm
packages from the retired TypeScript engine are deprecated; npm distribution
ended with v0.4.0.

## [Unreleased]

### Added

- `kindling tui` browses local memory with editable search, pins first, and a wrapped, scrollable preview. Panels adapt to terminal width; press Esc or Ctrl-C to quit.

### Fixed

- Project-root overrides now reject sibling directories with similar path prefixes, preventing incorrect repository selection.
- Invalid project-root requests are rejected rather than queued for retry.
- Installer fallback respects a pinned Kindling version and non-default Cargo installation locations.

## [0.4.1] - 2026-11-10

### Security

- Updated bundled SQLite to address RUSTSEC-2026-0459.
- Improved secret redaction in Claude Code hooks and observation metadata, including padded authentication credentials.

### Fixed

- Failed capsule closures, observation attachments and imports now roll back incomplete changes.
- Daemon socket cleanup no longer removes unrelated regular files.
- Database filenames containing harmless adjacent dots are accepted.

### Changed

- `kindling search` now defaults to the current Git repository. Use `--all-repos` to search across repositories, or `--repo` to select one explicitly.
- Public Rust SQLite error variants now use the updated rusqlite dependency.

## [0.4.0] - 2026-10-19

This integration-hardening release includes breaking client, daemon and storage changes. Upgrade connected daemon and client installations together.

### Breaking

- Loopback TCP connections now require a bearer token; older TCP clients cannot connect to the updated daemon.
- The embedded `kindling_server::build_router` API now requires a TCP-secret argument.
- The SQLite schema advances from v5 to v6. Read-write opens migrate automatically; a v6 database cannot be opened by a 0.3 engine.

### Added

- Append responses now report redaction counts and classes without exposing redacted values.
- Versioned hook-payload fixtures are available for adapter authors.

### Changed

- Kindling is distributed through GitHub Releases, Cargo and Homebrew; publishing to npm has ended. Existing 0.1.x npm packages are retired/deprecated.
- Offline observation spooling has improved recovery behaviour.

### Fixed

- Capsule closure handles missing summaries safely.
- Combining `--via-daemon` with `--db` now reports an error instead of silently selecting the wrong database.

## [0.3.0] - 2026-06-26

The downstream-integration release expands the daemon and client APIs used by integrations such as Anvil.

### Breaking

- Rust SDK users must construct `SpoolConfig` through its constructor and builders rather than a struct literal; selected public runtime types are now non-exhaustive.

### Added

- Paginated observation listing with kind, scope and time-range filters.
- Optional age and size limits for durable outage spooling.
- The `kindling-runtime` facade for attach-or-start daemon integration.
- `kindling demo`, `kindling browse`, editor integration and new onboarding materials.

### Fixed

- Claude Code plugin tests run correctly on newer Node versions.
- The local HTML viewer safely embeds observation content.

## [0.2.0] - 2026-06-22

Kindling's engine moved to Rust, with an installable CLI, local daemon and Rust integration crates.

### Added

- Durable observation spooling for temporary daemon outages.
- Rust CLI, daemon, service and client crates, with project-scoped local storage.
- The `kindling forget` command to redact stored observations.
- Install and packaging support for the Rust-based engine.

### Changed

- TypeScript implementation packages were retired in favour of the Rust engine and thin clients.
- Development dependency vulnerabilities were addressed.

## [0.1.3] - 2026-05-08

### Added

- `kindling log` and `kindling capsule` write commands.
- A Claude Code recall plugin and explicit memory command.
- A shared SQLite schema contract and expanded installation documentation.

### Changed

- Updated server dependencies and normalised package layout.

## [0.1.2] - 2026-02-16

### Changed

- **Performance**: Denormalized scope ID columns replace `json_extract()` in queries (migration 004)
- **Performance**: FTS scoring moved to SQL with CTE-based queries; BM25 normalization done cross-table in JS
- **Performance**: Cached project root via `KINDLING_REPO_ROOT` env var in Claude Code hooks

### Fixed

- Shell argument injection in Claude Code command wrappers (`$ARGUMENTS` now quoted)
- Readonly export/import handles pre-migration-004 databases gracefully

## [0.1.1] - 2025-02-10

### Changed

- Version bump for monorepo release consistency (no functional changes)

## [0.1.0] - 2025-02-09

### Added

- Initial public release
- Core domain model with observations, capsules, summaries, and pins
- SQLite persistence with FTS5 full-text search
- sql.js WASM store for browser compatibility
- Local retrieval provider with deterministic ranking
- OpenCode session adapter
- PocketFlow workflow adapter with intent inference and confidence tracking
- CLI tools for status, search, list, pin, export, import, and serve commands
- API server for multi-agent concurrency
- GitHub sync commands for Claude Code Web integration
- Automatic secret detection and redaction
- Export/import functionality for data portability
- Test coverage across all packages

### Security

- Automatic redaction of secrets in captured content
- Configurable excluded file patterns for sensitive paths
- Bounded output capture to prevent excessive storage
