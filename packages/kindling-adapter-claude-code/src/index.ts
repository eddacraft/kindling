/**
 * kindling Claude Code Adapter
 *
 * Captures observations from Claude Code sessions via hooks.
 *
 * @example
 * ```typescript
 * import { createHookHandlers } from '@eddacraft/kindling-adapter-claude-code';
 * import { SqliteKindlingStore, openDatabase } from '@eddacraft/kindling-store-sqlite';
 *
 * const db = openDatabase({ dbPath: '~/.kindling/kindling.db' });
 * const store = new SqliteKindlingStore(db);
 * const handlers = createHookHandlers(store);
 *
 * // Register handlers with Claude Code hooks
 * // SessionStart -> handlers.onSessionStart
 * // PostToolUse -> handlers.onPostToolUse
 * // Stop -> handlers.onStop
 * ```
 */

// --- Deprecation notice (once per process) ---
const KEY = '__kindling_deprecation_warned_adapter_claude_code';
if (!(globalThis as Record<string, unknown>)[KEY]) {
  (globalThis as Record<string, unknown>)[KEY] = true;
  console.warn(
    '[DEPRECATED] @eddacraft/kindling-adapter-claude-code is deprecated and will be removed at v1.0.0. ' +
      'kindling is now a Rust daemon; use @eddacraft/kindling (thin client) or the `kindling` binary. ' +
      'See https://github.com/eddacraft/kindling.',
  );
}

export * from './claude-code/index.js';
