#!/usr/bin/env node
/**
 * PostToolUse hook handler
 *
 * Captures tool calls as observations in the SQLite store.
 * Exit 0 = success (never blocks tool use).
 */

const { init, cleanup, readStdin } = require('./lib/init.js');

async function main() {
  const context = await readStdin();

  const sessionId = context.session_id || 'unknown';
  const cwd = context.cwd || process.cwd();
  const toolName = context.tool_name || 'unknown';

  const { db, handlers } = init(cwd);

  try {
    // Re-hydrate session from DB (each hook invocation is a separate process)
    handlers.onSessionStart({ sessionId, cwd });

    handlers.onPostToolUse({
      sessionId,
      cwd,
      toolName,
      toolInput: context.tool_input || {},
      toolResult: context.tool_result,
      toolError: context.tool_error,
    });

    console.error(`[kindling] Captured ${toolName}`);
  } finally {
    cleanup(db);
  }
}

main()
  .catch((err) => {
    console.error(`[kindling] PostToolUse error: ${err.message}`);
  })
  .finally(() => {
    process.exit(0);
  });
