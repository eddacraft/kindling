/**
 * kindling API Server
 *
 * HTTP API server for multi-agent concurrency.
 */

// --- Deprecation notice (once per process) ---
const KEY = '__kindling_deprecation_warned_server';
if (!(globalThis as Record<string, unknown>)[KEY]) {
  (globalThis as Record<string, unknown>)[KEY] = true;
  console.warn(
    '[DEPRECATED] @eddacraft/kindling-server is deprecated and will be removed at v1.0.0. ' +
      'kindling is now a Rust daemon; use @eddacraft/kindling (thin client) or the `kindling` binary. ' +
      'See https://github.com/eddacraft/kindling.',
  );
}

export { createServer, startServer } from './server.js';
export type { ServerConfig } from './server.js';
