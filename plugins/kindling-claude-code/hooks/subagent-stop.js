#!/usr/bin/env node
/**
 * SubagentStop hook handler
 *
 * Captures subagent (Task tool) completions as observations.
 * Exit 0 = success (never blocks subagent completion).
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

    handlers.onSubagentStop({
      sessionId,
      cwd,
      agentType: context.agent_type || 'unknown',
      task: context.task,
      output: context.output,
    });

    const agentTypeForLog =
      context.agent_type || `unknown (available: ${Object.keys(context || {}).join(', ')})`;
    console.error(`[kindling] Captured subagent: ${agentTypeForLog}`);
  } finally {
    cleanup(db);
  }
}

main()
  .catch((err) => {
    console.error(`[kindling] SubagentStop error: ${err.message}`);
  })
  .finally(() => {
    process.exit(0);
  });
