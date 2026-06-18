/**
 * Init command - Initialize kindling (create database and optionally configure hooks)
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync, cpSync } from 'fs';
import { homedir } from 'os';
import { join, dirname } from 'path';
import { openDatabase } from '@eddacraft/kindling-store-sqlite';
import { handleError, formatJson, getDefaultDbPath } from '../utils.js';

interface InitOptions {
  db?: string;
  claudeCode?: boolean;
  skipDb?: boolean;
  json?: boolean;
}

interface ClaudeSettings {
  enabledPlugins?: string[];
  [key: string]: unknown;
}

export async function initCommand(options: InitOptions): Promise<void> {
  try {
    const results: {
      directory: { created: boolean; path: string };
      database: { created: boolean; path: string; existed: boolean } | null;
      claudeCode: { configured: boolean; pluginPath: string; message: string } | null;
    } = {
      directory: { created: false, path: '' },
      database: null,
      claudeCode: null,
    };

    const dbPath = options.db || getDefaultDbPath();
    const kindlingDir = dirname(dbPath);

    // Step 1: Create ~/.kindling/ directory
    if (!existsSync(kindlingDir)) {
      mkdirSync(kindlingDir, { recursive: true });
      results.directory = { created: true, path: kindlingDir };
    } else {
      results.directory = { created: false, path: kindlingDir };
    }

    // Step 2: Create database (unless --skip-db)
    if (!options.skipDb) {
      const dbExisted = existsSync(dbPath);

      // openDatabase creates the database and runs migrations
      const db = openDatabase({ path: dbPath });
      db.close();

      results.database = {
        created: !dbExisted,
        path: dbPath,
        existed: dbExisted,
      };
    }

    // Step 3: Configure Claude Code (if --claude-code flag)
    if (options.claudeCode) {
      results.claudeCode = await configureClaudeCode();
    }

    // Output results
    if (options.json) {
      console.log(formatJson(results, true));
    } else {
      printHumanReadable(results, options.claudeCode ?? false);
    }
  } catch (error) {
    handleError(error, options.json);
  }
}

async function configureClaudeCode(): Promise<{
  configured: boolean;
  pluginPath: string;
  message: string;
}> {
  const claudeDir = join(homedir(), '.claude');
  const pluginsDir = join(claudeDir, 'plugins');
  const kindlingPluginDir = join(pluginsDir, 'kindling');
  const settingsPath = join(claudeDir, 'settings.json');

  // Check if Claude Code is installed
  if (!existsSync(claudeDir)) {
    return {
      configured: false,
      pluginPath: '',
      message: 'Claude Code not detected (~/.claude/ does not exist)',
    };
  }

  // Check if plugin already exists
  if (existsSync(kindlingPluginDir)) {
    // Plugin exists, just ensure it's enabled
    enablePluginInSettings(settingsPath);
    return {
      configured: true,
      pluginPath: kindlingPluginDir,
      message: 'Plugin already installed, ensured enabled in settings',
    };
  }

  // Try to find the plugin source in common locations
  const possiblePluginSources = [
    // Relative to this CLI package (in monorepo)
    join(dirname(new URL(import.meta.url).pathname), '../../../../plugins/kindling-claude-code'),
    // Installed globally or locally
    join(dirname(new URL(import.meta.url).pathname), '../../../kindling-plugin-claude-code'),
    // In node_modules
    join(process.cwd(), 'node_modules/@eddacraft/kindling-plugin-claude-code'),
  ];

  let pluginSource: string | null = null;
  for (const source of possiblePluginSources) {
    if (existsSync(join(source, 'plugin.json'))) {
      pluginSource = source;
      break;
    }
  }

  if (!pluginSource) {
    return {
      configured: false,
      pluginPath: '',
      message: `Plugin source not found. Install manually:\n  git clone https://github.com/eddacraft/kindling ${kindlingPluginDir}`,
    };
  }

  // Create plugins directory if needed
  if (!existsSync(pluginsDir)) {
    mkdirSync(pluginsDir, { recursive: true });
  }

  // Copy plugin to Claude Code plugins directory
  cpSync(pluginSource, kindlingPluginDir, { recursive: true });

  // Enable plugin in settings
  enablePluginInSettings(settingsPath);

  return {
    configured: true,
    pluginPath: kindlingPluginDir,
    message: 'Plugin installed and enabled',
  };
}

function enablePluginInSettings(settingsPath: string): void {
  let settings: ClaudeSettings = {};

  // Read existing settings if present
  if (existsSync(settingsPath)) {
    try {
      const content = readFileSync(settingsPath, 'utf-8');
      settings = JSON.parse(content);
    } catch {
      // If settings file is invalid, start fresh
      settings = {};
    }
  }

  // Ensure enabledPlugins array exists and includes kindling
  if (!settings.enabledPlugins) {
    settings.enabledPlugins = [];
  }

  if (!settings.enabledPlugins.includes('kindling')) {
    settings.enabledPlugins.push('kindling');
  }

  // Write settings back
  writeFileSync(settingsPath, JSON.stringify(settings, null, 2) + '\n');
}

function printHumanReadable(
  results: {
    directory: { created: boolean; path: string };
    database: { created: boolean; path: string; existed: boolean } | null;
    claudeCode: { configured: boolean; pluginPath: string; message: string } | null;
  },
  claudeCodeRequested: boolean,
): void {
  console.log('\nKindling Setup');
  console.log('==============\n');

  // Directory status
  if (results.directory.created) {
    console.log(`✓ Created directory ${results.directory.path}`);
  } else {
    console.log(`✓ Directory exists ${results.directory.path}`);
  }

  // Database status
  if (results.database) {
    if (results.database.created) {
      console.log(`✓ Created database ${results.database.path}`);
    } else if (results.database.existed) {
      console.log(`✓ Database exists ${results.database.path}`);
    }
  }

  // Claude Code status
  if (claudeCodeRequested && results.claudeCode) {
    console.log('\nClaude Code Integration');
    console.log('-----------------------');
    if (results.claudeCode.configured) {
      console.log(`✓ ${results.claudeCode.message}`);
      if (results.claudeCode.pluginPath) {
        console.log(`  Plugin: ${results.claudeCode.pluginPath}`);
      }
    } else {
      console.log(`✗ ${results.claudeCode.message}`);
    }
  }

  // Next steps
  console.log('\nKindling is ready!\n');
  console.log('Next steps:');
  console.log('  kindling status     - Check database status');
  console.log('  kindling search     - Search your memory');
  console.log('  kindling serve      - Start API server');

  if (!claudeCodeRequested) {
    console.log('\nFor Claude Code integration:');
    console.log('  kindling init --claude-code');
  } else if (results.claudeCode?.configured) {
    console.log('\nRestart Claude Code to activate the plugin. Then use:');
    console.log('  /memory search <query>   - Search past sessions');
    console.log('  /memory status           - View stats');
    console.log('  /memory pin [note]       - Pin important findings');
  }

  console.log('');
}
