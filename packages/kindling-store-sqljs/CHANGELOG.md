# Changelog

All notable changes to @eddacraft/kindling-store-sqljs will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-02-16

### Changed

- Version bump for monorepo release consistency

## [0.1.0] - 2025-02-09

## [0.1.0] - 2025-02-09

### Added

- Initial release
- sql.js WASM-based SQLite store for browser compatibility
- Drop-in replacement for @eddacraft/kindling-store-sqlite
- FTS5 full-text search support (via sql.js)
- Memory persistence adapter for ephemeral storage
- IndexedDB persistence adapter for browser storage
- All core store operations: observations, capsules, summaries, pins
- Transaction support via manual BEGIN/COMMIT/ROLLBACK
- Export/import functionality
