# @eddacraft/kindling-store-sqlite

> **⚠️ Deprecated** — this package is deprecated and will be removed at v1.0.0.
> kindling is now a Rust daemon. Use [`@eddacraft/kindling`](https://www.npmjs.com/package/@eddacraft/kindling)
> (the thin HTTP-over-UDS client) or the `kindling` binary instead.
> See <https://github.com/eddacraft/kindling>.

SQLite persistence layer for kindling with FTS5 full-text search and WAL mode.

[![npm version](https://img.shields.io/npm/v/@eddacraft/kindling-store-sqlite.svg)](https://www.npmjs.com/package/@eddacraft/kindling-store-sqlite)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../../LICENSE)

## Installation

```bash
npm install @eddacraft/kindling-store-sqlite
```

## Overview

`@eddacraft/kindling-store-sqlite` provides the persistence layer for kindling using embedded SQLite:

- **WAL Mode** - Write-ahead logging for concurrent access
- **FTS5 Indexing** - Full-text search on observations and summaries
- **Automatic Migrations** - Schema versioning with migration support
- **Local-First** - No external services required

## Usage

### Opening a Database

```typescript
import { openDatabase, closeDatabase } from '@eddacraft/kindling-store-sqlite';

// Open with file path
const db = openDatabase({ path: './kindling.db' });

// Or use in-memory for testing
const testDb = openDatabase({ path: ':memory:' });

// Close when done
closeDatabase(db);
```

### Using the Store

```typescript
import { openDatabase, SqliteKindlingStore } from '@eddacraft/kindling-store-sqlite';

const db = openDatabase({ path: './kindling.db' });
const store = new SqliteKindlingStore(db);

// Insert an observation
store.insertObservation({
  id: 'obs-1',
  kind: 'tool_call',
  content: 'Read file src/auth.ts',
  provenance: { toolName: 'read_file', path: 'src/auth.ts' },
  ts: Date.now(),
  scopeIds: { sessionId: 's1', repoId: '/repo' },
  redacted: false,
});

// Query observations
const observations = store.getObservations({
  scopeIds: { sessionId: 's1' },
  limit: 100,
});

// Insert a capsule
store.insertCapsule({
  id: 'cap-1',
  type: 'session',
  intent: 'Fix authentication bug',
  status: 'open',
  openedAt: Date.now(),
  scopeIds: { sessionId: 's1', repoId: '/repo' },
  observationIds: [],
});

// Attach observation to capsule
store.attachObservation('cap-1', 'obs-1');

// Full-text search
const results = store.searchObservations({
  query: 'authentication',
  scopeIds: { repoId: '/repo' },
  limit: 50,
});
```

### Migrations

Migrations run automatically when opening the database:

```typescript
import { openDatabase, getMigrationStatus } from '@eddacraft/kindling-store-sqlite';

const db = openDatabase({ dbPath: './kindling.db' });

// Check migration status
const status = getMigrationStatus(db);
console.log('Current version:', status.currentVersion);
console.log('Pending migrations:', status.pending);
```

### Export/Import

```typescript
import { exportDatabase, importDatabase } from '@eddacraft/kindling-store-sqlite';

// Export all data
const bundle = exportDatabase(db, {
  scope: { repoId: '/repo' },
});

// Import into another database
importDatabase(targetDb, bundle);
```

## Database Schema

The store manages these tables:

| Table                  | Purpose                            |
| ---------------------- | ---------------------------------- |
| `observations`         | Atomic event records               |
| `observations_fts`     | FTS5 index for observation content |
| `capsules`             | Bounded units of meaning           |
| `capsule_observations` | Join table with ordering           |
| `summaries`            | Capsule summaries                  |
| `summaries_fts`        | FTS5 index for summary content     |
| `pins`                 | User-marked important items        |
| `schema_migrations`    | Migration version tracking         |

## Configuration

```typescript
interface DatabaseOptions {
  path?: string; // File path (defaults to ~/.kindling/kindling.db)
  verbose?: boolean; // Enable verbose logging (default: false)
  readonly?: boolean; // Read-only mode, skips migrations (default: false)
}
```

## Requirements

- Node.js >= 20.0.0
- SQLite support via `better-sqlite3`

## Related Packages

- [`@eddacraft/kindling-core`](../kindling-core) - Domain types and interfaces
- [`@eddacraft/kindling-provider-local`](../kindling-provider-local) - FTS retrieval using this store

## License

Apache-2.0
