# @eddacraft/kindling-provider-local

> **⚠️ Deprecated** — this package is deprecated and will be removed at v1.0.0.
> kindling is now a Rust daemon. Use [`@eddacraft/kindling`](https://www.npmjs.com/package/@eddacraft/kindling)
> (the thin HTTP-over-UDS client) or the `kindling` binary instead.
> See <https://github.com/eddacraft/kindling>.

Local FTS-based retrieval provider for kindling with deterministic, explainable ranking.

[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling-provider-local.svg)](https://www.npmjs.com/package/@eddacraft/kindling-provider-local)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../../LICENSE)

## Installation

```bash
npm install @eddacraft/kindling-provider-local
```

## Overview

`@eddacraft/kindling-provider-local` implements the `RetrievalProvider` interface using SQLite FTS5:

- **FTS5 Search** - Full-text search with BM25 ranking
- **Recency Weighting** - Recent observations score higher
- **Deterministic Results** - Same query always produces same results
- **Explainable Scoring** - Results include match context and scores

## Usage

```typescript
import { LocalFtsProvider } from '@eddacraft/kindling-provider-local';
import { openDatabase } from '@eddacraft/kindling-store-sqlite';

// Initialize
const db = openDatabase({ path: './kindling.db' });
const provider = new LocalFtsProvider(db);

// Search
const results = await provider.search({
  query: 'authentication error',
  scopeIds: { repoId: '/repo' },
  maxResults: 50,
});

// Results include scores and context
for (const result of results) {
  console.log(`${result.entity.id}: ${result.score}`);
  console.log(`  Match: ${result.matchContext}`);
}
```

## Scoring

The provider combines FTS relevance with recency:

```
score = (fts_relevance * 0.7) + (recency_score * 0.3)

where:
  fts_relevance = BM25 rank normalized to [0,1] across all results
                  (observations + summaries combined)
  recency_score = MAX(0, 1.0 - age_ms / max_age_ms)
```

BM25 normalization is done cross-table in JavaScript so that observation and summary scores are directly comparable. Singleton results receive a relevance of 0.5 (unknown relative relevance).

## Provider Interface

```typescript
interface RetrievalProvider {
  name: string;
  search(options: ProviderSearchOptions): Promise<ProviderSearchResult[]>;
}

interface ProviderSearchOptions {
  query: string;
  scopeIds: ScopeIds;
  maxResults?: number;
  excludeIds?: string[];
  includeRedacted?: boolean;
}

interface ProviderSearchResult {
  entity: Observation | Summary;
  score: number;
  matchContext?: string;
}
```

## Characteristics

| Property          | Value                      |
| ----------------- | -------------------------- |
| **Name**          | `local-fts`                |
| **Deterministic** | Yes                        |
| **Latency**       | < 50ms typical             |
| **Max Results**   | Configurable (default: 50) |

## Integration with Core

```typescript
import { CapsuleManager } from '@eddacraft/kindling-core';
import { SqliteKindlingStore } from '@eddacraft/kindling-store-sqlite';
import { LocalFtsProvider } from '@eddacraft/kindling-provider-local';

const store = new SqliteKindlingStore(db);
const provider = new LocalFtsProvider(db);

// Provider is used by core for retrieval
const manager = new CapsuleManager(store, { provider });

const results = manager.retrieve({
  query: 'authentication',
  scopeIds: { sessionId: 's1' },
});
```

## Related Packages

- [`@eddacraft/kindling-core`](../kindling-core) - Domain types and `RetrievalProvider` interface
- [`@eddacraft/kindling-store-sqlite`](../kindling-store-sqlite) - SQLite store with FTS indexes

## License

Apache-2.0
