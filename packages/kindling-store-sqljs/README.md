# @eddacraft/kindling-store-sqljs

> **⚠️ Deprecated** — this package is deprecated and will be removed at v1.0.0.
> Kindling is now a Rust daemon. Use [`@eddacraft/kindling`](https://www.npmjs.com/package/@eddacraft/kindling)
> (the thin HTTP-over-UDS client) or the `kindling` binary instead.
> See <https://github.com/eddacraft/kindling>.

WASM-based SQLite persistence layer for Kindling using [sql.js](https://sql.js.org/). This package provides browser and cross-platform compatibility as a drop-in replacement for `@eddacraft/kindling-store-sqlite`.

## When to Use

Use this package when:

- Running in a **browser** environment
- Running in environments without native compilation support (e.g., some serverless platforms)
- Need a **portable** solution that works everywhere JavaScript runs

Use `@eddacraft/kindling-store-sqlite` instead when:

- Running in **Node.js** with native compilation available
- **Performance** is critical (native is 2-10x faster)
- Memory usage is a concern (sql.js loads entire DB into memory)

## Installation

```bash
pnpm add @eddacraft/kindling-store-sqljs
```

## Usage

### Basic Usage

```typescript
import { openDatabase, SqljsKindlingStore } from '@eddacraft/kindling-store-sqljs';

// Open database (async - needs to load WASM)
const db = await openDatabase();
const store = new SqljsKindlingStore(db);

// Use like any KindlingStore
store.insertObservation({
  id: 'obs_1',
  kind: 'tool_call',
  content: 'User ran npm install',
  provenance: { source: 'cli' },
  ts: Date.now(),
  scopeIds: { sessionId: 'session_1' },
  redacted: false,
});
```

### With Browser Persistence (IndexedDB)

```typescript
import {
  openDatabase,
  SqljsKindlingStore,
  IndexedDBPersistence,
} from '@eddacraft/kindling-store-sqljs';

const persistence = new IndexedDBPersistence({
  dbName: 'my-app', // IndexedDB database name
  storeName: 'databases', // Object store name
  key: 'kindling', // Key for this database
});

// Load existing data or start fresh
const existingData = await persistence.load();
const db = await openDatabase({ data: existingData });
const store = new SqljsKindlingStore(db);

// ... use store ...

// Save changes (call periodically or on important operations)
await persistence.save(db.export());
```

### Custom WASM Location

By default, sql.js loads WASM from its CDN. For production, you should host the WASM files yourself:

```typescript
const db = await openDatabase({
  locateFile: (file) => `/wasm/${file}`, // Your hosted path
});
```

Or bundle with your application:

```typescript
const db = await openDatabase({
  locateFile: (file) => new URL(`./sql-wasm/${file}`, import.meta.url).href,
});
```

### With KindlingService

```typescript
import { KindlingService } from '@eddacraft/kindling-core';
import { LocalFtsProvider } from '@eddacraft/kindling-provider-local';
import { openDatabase, SqljsKindlingStore } from '@eddacraft/kindling-store-sqljs';

const db = await openDatabase();
const store = new SqljsKindlingStore(db);
const provider = new LocalFtsProvider(db);

const kindling = new KindlingService({
  store,
  provider,
});

// Now use kindling.openCapsule(), kindling.appendObservation(), etc.
```

## API

### Database Functions

#### `openDatabase(options?)`

Opens and initializes a Kindling database.

```typescript
interface DatabaseOptions {
  data?: Uint8Array; // Initial database data
  locateFile?: (file: string) => string; // WASM file locator
  skipMigrations?: boolean; // Skip running migrations
  verbose?: boolean; // Enable logging
}
```

#### `closeDatabase(db)`

Closes a database connection.

#### `exportDatabaseToBytes(db)`

Exports the database to a `Uint8Array` for persistence.

### Store Class

`SqljsKindlingStore` implements the `KindlingStore` interface from `@eddacraft/kindling-core`:

- `insertObservation(observation)` - Insert an observation
- `createCapsule(capsule)` - Create a capsule
- `closeCapsule(capsuleId, closedAt?, summaryId?)` - Close a capsule
- `attachObservationToCapsule(capsuleId, observationId)` - Link observation to capsule
- `insertSummary(summary)` - Insert a summary
- `insertPin(pin)` / `deletePin(pinId)` - Manage pins
- `getObservationById(id)` - Get observation by ID
- `getCapsule(id)` - Get capsule by ID
- `getOpenCapsuleForSession(sessionId)` - Find open capsule
- `queryObservations(scopeIds?, fromTs?, toTs?, limit?)` - Query observations
- `listActivePins(scopeIds?, now?)` - List non-expired pins
- `transaction(fn)` - Execute in transaction
- `exportDatabase(options?)` - Export all data
- `importDatabase(dataset)` - Import data

### Persistence Adapters

#### `IndexedDBPersistence`

Browser persistence using IndexedDB.

```typescript
const persistence = new IndexedDBPersistence({
  dbName: 'kindling',
  storeName: 'databases',
  key: 'main',
});

await persistence.save(data); // Save Uint8Array
await persistence.load(); // Load Uint8Array | undefined
await persistence.exists(); // Check if exists
await persistence.delete(); // Delete stored data
```

#### `MemoryPersistence`

In-memory persistence for testing.

```typescript
const persistence = new MemoryPersistence();
```

## Differences from store-sqlite

| Feature        | store-sqlite     | store-sqljs          |
| -------------- | ---------------- | -------------------- |
| Environment    | Node.js only     | Browser + Node.js    |
| Performance    | Native speed     | 2-10x slower         |
| Memory         | Memory-mapped    | Entire DB in memory  |
| Persistence    | Automatic (file) | Manual (export/save) |
| WAL mode       | Supported        | Not supported        |
| Initialization | Synchronous      | Asynchronous         |
| Bundle size    | Native binary    | ~1.5MB WASM          |

## FTS5 Support

FTS5 support depends on your sql.js build:

- **Standard sql.js**: FTS5 is **not included**. The package auto-detects this and skips FTS migrations.
- **Custom sql.js build**: You can compile sql.js with FTS5 enabled for full-text search support.

To explicitly control FTS behavior:

```typescript
// Force FTS5 (will error if not available)
const db = await openDatabase({ enableFts: true });

// Explicitly disable FTS5
const db = await openDatabase({ enableFts: false });

// Auto-detect (default)
const db = await openDatabase();
```

When FTS5 is not available, you can still use the store - you just won't have full-text search capabilities. The `@eddacraft/kindling-provider-local` FTS provider won't work, but you can implement alternative retrieval strategies.

## License

Apache-2.0
