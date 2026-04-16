#!/usr/bin/env node
/**
 * Stop hook handler
 *
 * Closes the session capsule when Claude Code stops.
 * Exit 0 = success (never blocks shutdown).
 */

const { init, cleanup, readStdin } = require('./lib/init.js');

async function main() {
  const context = await readStdin();

  const sessionId = context.session_id || 'unknown';
  const cwd = context.cwd || process.cwd();

  const { db, handlers } = init(cwd);

  try {
    // Re-hydrate session from DB (each hook invocation is a separate process)
    handlers.onSessionStart({ sessionId, cwd });

    handlers.onStop({
      sessionId,
      cwd,
      reason: context.stop_reason || context.reason,
      summary: context.summary,
    });

    console.error(`[kindling] Session closed`);
  } finally {
    cleanup(db);
  }
}

main()
  .catch((err) => {
    console.error(`[kindling] Stop error: ${err.message}`);
  })
  .finally(() => {
    process.exit(0);
  });
