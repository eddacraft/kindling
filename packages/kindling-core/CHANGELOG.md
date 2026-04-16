# Changelog

All notable changes to @eddacraft/kindling-core will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-02-16

### Changed

- Version bump for monorepo release consistency

## [0.1.0] - 2025-02-09

## [0.1.0] - 2025-02-09

### Added

- Initial release
- Domain types: Observation, Capsule, Summary, Pin, ScopeIds
- ObservationKind types: tool_call, command, file_diff, error, message, node_start, node_end, node_output, node_error
- CapsuleType types: session, pocketflow_node
- Result type pattern for validation (ok/err)
- Type guards for runtime validation
- KindlingService for orchestration
- Capsule lifecycle management (open, close, attach observations)
- Retrieval orchestration with tiered results (pins, current summary, provider hits)
- Pin management with TTL support
- Export/import coordination
