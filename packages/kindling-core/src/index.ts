/**
 * Kindling Core
 *
 * Domain model, capsule lifecycle, and retrieval orchestration.
 */

// --- Deprecation notice (once per process) ---
const KEY = '__kindling_deprecation_warned_core';
if (!(globalThis as Record<string, unknown>)[KEY]) {
  (globalThis as Record<string, unknown>)[KEY] = true;
  console.warn(
    '[DEPRECATED] @eddacraft/kindling-core is deprecated and will be removed at v1.0.0. ' +
      'Kindling is now a Rust daemon; use @eddacraft/kindling (thin client) or the `kindling` binary. ' +
      'See https://github.com/eddacraft/kindling.',
  );
}

// Re-export all types
export * from './types/index.js';

// Re-export all validation functions
export * from './validation/index.js';

// Re-export capsule lifecycle
export * from './capsule/index.js';

// Re-export retrieval orchestration
export * from './retrieval/index.js';

// Re-export export/import coordination
export * from './export/index.js';

// Re-export service orchestration
export * from './service/index.js';

// Re-export intent capture (append-only store + high-signal emitters)
export * from './intent/index.js';
