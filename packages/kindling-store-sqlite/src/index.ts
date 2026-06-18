/**
 * Kindling SQLite Store
 *
 * SQLite-based system of record for Kindling.
 */

// --- Deprecation notice (once per process) ---
const KEY = '__kindling_deprecation_warned_store_sqlite';
if (!(globalThis as Record<string, unknown>)[KEY]) {
  (globalThis as Record<string, unknown>)[KEY] = true;
  console.warn(
    '[DEPRECATED] @eddacraft/kindling-store-sqlite is deprecated and will be removed at v1.0.0. ' +
      'Kindling is now a Rust daemon; use @eddacraft/kindling (thin client) or the `kindling` binary. ' +
      'See https://github.com/eddacraft/kindling.',
  );
}

// Re-export database infrastructure
export * from './db/index.js';

// Re-export store implementation
export * from './store/index.js';
