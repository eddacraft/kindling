/**
 * kindling Local Provider
 *
 * FTS + recency-based retrieval provider.
 */

// --- Deprecation notice (once per process) ---
const KEY = '__kindling_deprecation_warned_provider_local';
if (!(globalThis as Record<string, unknown>)[KEY]) {
  (globalThis as Record<string, unknown>)[KEY] = true;
  console.warn(
    '[DEPRECATED] @eddacraft/kindling-provider-local is deprecated and will be removed at v1.0.0. ' +
      'kindling is now a Rust daemon; use @eddacraft/kindling (thin client) or the `kindling` binary. ' +
      'See https://github.com/eddacraft/kindling.',
  );
}

export { LocalFtsProvider } from './provider/local-fts.js';
