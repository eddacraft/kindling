#!/usr/bin/env node

/**
 * kindling CLI
 *
 * Command-line interface for inspecting, managing, and debugging local memory.
 */

import { Command } from 'commander';
import { initCommand } from './commands/init.js';
import { statusCommand } from './commands/status.js';
import { searchCommand } from './commands/search.js';
import { listCommand } from './commands/list.js';
import { logCommand } from './commands/log.js';
import { capsuleOpenCommand, capsuleCloseCommand } from './commands/capsule.js';
import { pinCommand, unpinCommand } from './commands/pin.js';
import { exportCommand, importCommand } from './commands/export.js';
import { serveCommand } from './commands/serve.js';
import { syncInitCommand, syncAddSubmoduleCommand, syncPushCommand } from './commands/sync.js';

// --- Deprecation notice (once per process, to stderr) ---
const DEPRECATION_KEY = '__kindling_deprecation_warned_cli';
if (!(globalThis as Record<string, unknown>)[DEPRECATION_KEY]) {
  (globalThis as Record<string, unknown>)[DEPRECATION_KEY] = true;
  process.stderr.write(
    '[DEPRECATED] @eddacraft/kindling-cli is deprecated and will be removed at v1.0.0. ' +
      'kindling is now a Rust daemon; use @eddacraft/kindling (thin client) or the `kindling` binary. ' +
      'See https://github.com/eddacraft/kindling.\n',
  );
}

const program = new Command();

program
  .name('kindling')
  .description('Local memory and continuity engine for AI-assisted development')
  .version('0.1.0');

// Init command
program
  .command('init')
  .description('Initialize kindling (create database and configure hooks)')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--claude-code', 'Also configure Claude Code integration')
  .option('--skip-db', 'Skip database creation (only configure hooks)')
  .option('--json', 'Output as JSON')
  .action(initCommand);

// Log command
program
  .command('log <content>')
  .description('Log an observation to memory')
  .option('--kind <kind>', 'Observation kind (default: message)')
  .option('--session <id>', 'Session scope ID')
  .option('--repo <id>', 'Repository scope ID')
  .option('--capsule <id>', 'Attach to existing capsule')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--json', 'Output as JSON')
  .action(logCommand);

// Capsule commands
const capsuleCommand = program.command('capsule').description('Manage capsules (open/close)');

capsuleCommand
  .command('open')
  .description('Open a new capsule')
  .requiredOption('--intent <text>', 'Purpose of the capsule')
  .option('--type <type>', 'Capsule type (default: session)')
  .option('--session <id>', 'Session scope ID')
  .option('--repo <id>', 'Repository scope ID')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--json', 'Output as JSON')
  .action(capsuleOpenCommand);

capsuleCommand
  .command('close <id>')
  .description('Close a capsule')
  .option('--summary <text>', 'Summary text for the capsule')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--json', 'Output as JSON')
  .action(capsuleCloseCommand);

// Status command
program
  .command('status')
  .description('Show database status and statistics')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--json', 'Output as JSON')
  .action(statusCommand);

// Search command
program
  .command('search <query>')
  .description('Search for relevant context in memory')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--session <id>', 'Filter by session ID')
  .option('--repo <id>', 'Filter by repository ID')
  .option('--max <n>', 'Maximum results to return', '10')
  .option('--json', 'Output as JSON')
  .action(searchCommand);

// List command
program
  .command('list <entity>')
  .description('List entities (capsules, pins, observations)')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--session <id>', 'Filter by session ID')
  .option('--repo <id>', 'Filter by repository ID')
  .option('--limit <n>', 'Maximum results to return', '20')
  .option('--json', 'Output as JSON')
  .action(listCommand);

// Pin command
program
  .command('pin <type> <id>')
  .description('Pin an observation or summary (type: observation|summary)')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--note <text>', 'Note describing why this is pinned')
  .option('--ttl <ms>', 'Time-to-live in milliseconds')
  .option('--json', 'Output as JSON')
  .action(pinCommand);

// Unpin command
program
  .command('unpin <id>')
  .description('Remove a pin by ID')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--json', 'Output as JSON')
  .action(unpinCommand);

// Export command
program
  .command('export [output]')
  .description('Export memory to file (default: kindling-export-<timestamp>.json)')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--session <id>', 'Export only specific session')
  .option('--repo <id>', 'Export only specific repository')
  .option('--pretty', 'Pretty-print JSON output')
  .option('--json', 'Output metadata as JSON')
  .action(exportCommand);

// Import command
program
  .command('import <file>')
  .description('Import memory from export file')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--dry-run', 'Validate without importing')
  .option('--json', 'Output as JSON')
  .action(importCommand);

// Serve command
program
  .command('serve')
  .description('Start API server for multi-agent access')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--port <port>', 'Port to listen on', '8080')
  .option('--host <host>', 'Host to bind to', '127.0.0.1')
  .option('--no-cors', 'Disable CORS')
  .action(serveCommand);

// Sync commands
const syncCommand = program
  .command('sync')
  .description('GitHub sync commands for Claude Code Web integration');

syncCommand
  .command('init')
  .description('Initialize kindling GitHub sync (ONCE globally)')
  .requiredOption('--repo <name>', 'GitHub repo (username/kindling-memory)')
  .option('--private', 'Create as private repo (recommended)')
  .action(syncInitCommand);

syncCommand
  .command('add-submodule')
  .description('Add kindling memory as submodule to current project')
  .action(syncAddSubmoduleCommand);

syncCommand
  .command('push')
  .description('Push current kindling memory to GitHub')
  .option('--db <path>', 'Database path (default: ~/.kindling/kindling.db)')
  .option('--branch <name>', 'Branch to push to (default: main)')
  .option('--scope <value>', 'Scope filter: all|7d|30d (default: all)')
  .action(syncPushCommand);

// Parse args and execute
program.parse();
