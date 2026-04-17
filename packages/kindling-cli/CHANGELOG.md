# Changelog

All notable changes to @eddacraft/kindling-cli will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-02-16

### Changed

- Version bump for monorepo release consistency

## [0.1.0] - 2025-02-09

## [0.1.0] - 2025-02-09

### Added

- Initial release
- `kindling status` - Show database status and statistics
- `kindling search <query>` - Search for relevant context
- `kindling list <entity>` - List capsules, pins, or observations
- `kindling pin <type> <id>` - Pin an observation or summary
- `kindling unpin <id>` - Remove a pin
- `kindling export [output]` - Export memory to JSON file
- `kindling import <file>` - Import memory from export file
- `kindling serve` - Start API server for multi-agent access
- `kindling sync init` - Initialize GitHub sync
- `kindling sync add-submodule` - Add memory as git submodule
- `kindling sync push` - Push memory to GitHub
- JSON output mode for all commands
- Configurable database path via --db option
