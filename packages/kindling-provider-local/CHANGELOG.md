# Changelog

All notable changes to @eddacraft/kindling-provider-local will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-02-16

### Changed

- FTS matching and recency scoring moved from JS to SQL (CTE-based queries)
- BM25 normalization done cross-table in JS for accurate observation vs summary ranking
- Singleton results receive 0.5 relevance instead of inflated 1.0

## [0.1.0] - 2025-02-09

## [0.1.0] - 2025-02-09

### Added

- Initial release
- LocalFtsProvider for FTS-based retrieval
- Deterministic ranking algorithm combining FTS score and recency
- Scope-based filtering (sessionId, repoId, agentId, userId)
- Explainable results with match explanations
- Configurable result limits
- Evidence snippet extraction
