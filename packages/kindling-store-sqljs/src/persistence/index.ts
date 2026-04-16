/**
 * Persistence adapters for sql.js databases
 */

export { IndexedDBPersistence, type IndexedDBPersistenceOptions } from './indexeddb.js';

export { MemoryPersistence } from './memory.js';

export type { PersistenceAdapter, PersistenceResult } from './types.js';
