#!/usr/bin/env node
/**
 * PreCompact hook handler
 *
 * Injects pinned items and key summaries before context compaction,
 * so important memories survive the compression.
 * Exit 0 = success (never blocks compaction).
 */

const { init, cleanup, readStdin, getProjectRoot } = require('./lib/init.js');

async function main() {
  const context = await readStdin();

  const cwd = context.cwd || process.cwd();

  const { db, store } = init(cwd);

  try {
    const repoRoot = getProjectRoot(cwd);
    const items = [];

    // Include pins — these are explicitly marked as important
    const pins = store.listActivePins({ repoId: repoRoot }, Date.now());
    if (pins && pins.length > 0) {
      items.push('## Pinned Items (preserve across compaction)');
      for (const pin of pins) {
        const preview = pin.content ? pin.content.substring(0, 300) : '(no content)';
        items.push(`- **${pin.note || 'Pin'}**: ${preview}`);
      }
    }

    // Include latest capsule summary if available
    const latestSummary = db
      .prepare(
        `SELECT s.content, s.confidence FROM summaries s
       JOIN capsules c ON s.capsule_id = c.id
       WHERE c.repo_id = ?
       ORDER BY s.created_at DESC LIMIT 1`,
      )
      .get(repoRoot);

    if (latestSummary && latestSummary.content) {
      items.push('## Session Summary');
      items.push(latestSummary.content.substring(0, 500));
    }

    if (items.length > 0) {
      const output = JSON.stringify({
        continue: true,
        hookSpecificOutput: {
          hookEventName: 'PreCompact',
          additionalContext: items.join('\n'),
        },
      });
      process.stdout.write(output);
      const injectedCount = (pins ? pins.length : 0) + (latestSummary ? 1 : 0);
      console.error(`[kindling] Injected ${injectedCount} context items before compaction`);
    }
  } catch (err) {
    // Best-effort; don't block compaction
    console.error(`[kindling] PreCompact error: ${err.message}`);
  } finally {
    cleanup(db);
  }
}

main()
  .catch((err) => {
    console.error(`[kindling] PreCompact error: ${err.message}`);
  })
  .finally(() => {
    process.exit(0);
  });
