#!/usr/bin/env node
/**
 * SessionStart hook handler
 *
 * Opens a new capsule when a Claude Code session begins.
 * Optionally injects prior context for the current project.
 * Exit 0 = success (never blocks session startup).
 */

const { init, cleanup, readStdin, getProjectRoot } = require('./lib/init.js');

async function main() {
  const context = await readStdin();

  const sessionId = context.session_id || `session-${Date.now()}`;
  const cwd = context.cwd || process.cwd();

  const { db, store, handlers, service, dbPath } = init(cwd);

  try {
    // Open capsule for this session
    handlers.onSessionStart({ sessionId, cwd });

    // Context injection: query prior observations for this project
    const injectContext = process.env.KINDLING_INJECT_CONTEXT !== 'false';
    if (injectContext) {
      try {
        const maxResults = parseInt(process.env.KINDLING_MAX_CONTEXT || '10', 10);
        const repoRoot = getProjectRoot(cwd);

        const items = [];

        // Include pins (via store API)
        const pins = store.listActivePins({ repoId: repoRoot }, Date.now());
        if (pins && pins.length > 0) {
          items.push('## Pinned Items');
          for (const pin of pins) {
            const preview = pin.content ? pin.content.substring(0, 200) : '(no content)';
            items.push(`- **${pin.note || 'Pin'}**: ${preview}`);
          }
        }

        // Include recent observations (recency-based, no FTS query needed)
        const recentObs = db
          .prepare(
            'SELECT id, kind, content, ts FROM observations WHERE repo_id = ? AND redacted = 0 ORDER BY ts DESC LIMIT ?',
          )
          .all(repoRoot, maxResults);

        if (recentObs.length > 0) {
          items.push('## Recent Activity');
          for (const obs of recentObs) {
            const ts = obs.ts ? new Date(obs.ts).toLocaleString() : '';
            const preview = (obs.content || '').substring(0, 300).replace(/\n/g, ' ');
            items.push(`- [${ts}] ${obs.kind}: ${preview}`);
          }
        }

        if (items.length > 0) {
          // Claude Code hook protocol: stdout JSON with hookSpecificOutput causes
          // the additionalContext string to be injected into the system prompt.
          // See: https://docs.anthropic.com/en/docs/claude-code/hooks
          const header = `# Prior Context (from Kindling)\n\nThe following is prior session context for this project:\n`;
          const output = JSON.stringify({
            continue: true,
            hookSpecificOutput: {
              hookEventName: 'SessionStart',
              additionalContext: header + items.join('\n'),
            },
          });
          process.stdout.write(output);
        }
      } catch (err) {
        // Context injection is best-effort; don't fail the session
        console.error(`[kindling] Context injection error: ${err.message}`);
      }
    }

    console.error(`[kindling] Session started (db: ${dbPath})`);
  } finally {
    cleanup(db);
  }
}

main()
  .catch((err) => {
    console.error(`[kindling] SessionStart error: ${err.message}`);
  })
  .finally(() => {
    process.exit(0);
  });
