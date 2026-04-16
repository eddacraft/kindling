#!/usr/bin/env node
/**
 * PostToolUseFailure hook handler
 *
 * Captures failed tool calls as observations. Errors are particularly
 * valuable for cross-session memory (what broke, what was tried).
 * Exit 0 = success (never blocks tool failure handling).
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
      toolError: context.tool_error || context.error || 'Unknown error',
    });

    console.error(`[kindling] Captured ${toolName} failure`);
  } finally {
    cleanup(db);
  }
}

main()
  .catch((err) => {
    console.error(`[kindling] PostToolUseFailure error: ${err.message}`);
  })
  .finally(() => {
    process.exit(0);
  });
