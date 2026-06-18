# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-02-16

### Changed

- Version bump for monorepo release consistency

## [0.1.0] - 2025-02-09

## [0.1.0] - 2025-02-09

### Added

- Initial release of `@eddacraft/kindling-adapter-claude-code`
- Hook handlers for Claude Code integration:
  - `onSessionStart` - Opens session capsules
  - `onPostToolUse` - Captures tool calls as observations
  - `onStop` - Closes session capsules with optional summary
  - `onUserPromptSubmit` - Captures user messages
  - `onSubagentStop` - Captures subagent completions
- Event mapping from Claude Code hooks to kindling observations:
  - Write/Edit tools → `file_diff` observations
  - Bash tool → `command` observations
  - Other tools → `tool_call` observations
  - User prompts → `message` observations
  - Subagent stops → `node_end` observations
- Content filtering and safety features:
  - Secret detection and masking (API keys, tokens, passwords)
  - Content truncation for large outputs
  - Path exclusion patterns
- Session management with active session tracking
- Provenance extraction for all tool types
- Configuration options for selective capture
