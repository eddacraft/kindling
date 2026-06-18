/**
 * GitHub Sync Command - Sync kindling memory to GitHub repo
 *
 * Enables Claude Code Web integration using git submodules.
 */

import { openDatabase } from '@eddacraft/kindling-store-sqlite';
import { SqliteKindlingStore } from '@eddacraft/kindling-store-sqlite';
import { LocalFtsProvider } from '@eddacraft/kindling-provider-local';
import { KindlingService } from '@eddacraft/kindling-core';
import { getDefaultDbPath } from '../utils.js';
import { execSync } from 'child_process';
import { writeFileSync, mkdirSync, existsSync, readFileSync } from 'fs';
import { join } from 'path';
import { homedir } from 'os';

export interface SyncOptions {
  db?: string;
  repo?: string;
  branch?: string;
  interval?: string;
  scope?: string; // 'all' | '7d' | '30d'
}

/**
 * Get stored sync config
 */
function getSyncConfig(): { repo?: string } {
  const configPath = join(homedir(), '.kindling', 'sync-config.json');
  if (existsSync(configPath)) {
    return JSON.parse(readFileSync(configPath, 'utf8'));
  }
  return {};
}

/**
 * Save sync config
 */
function saveSyncConfig(config: { repo: string }): void {
  const configDir = join(homedir(), '.kindling');
  if (!existsSync(configDir)) {
    mkdirSync(configDir, { recursive: true });
  }
  const configPath = join(configDir, 'sync-config.json');
  writeFileSync(configPath, JSON.stringify(config, null, 2));
}

/**
 * Initialize GitHub sync (creates the kindling-memory repo)
 *
 * This is done ONCE globally, not per-project.
 */
export async function syncInitCommand(options: SyncOptions): Promise<void> {
  console.log('🔄 Initializing kindling GitHub Sync...\n');
  console.log('⚠️  IMPORTANT: This creates a SHARED memory repository.');
  console.log('    Do this ONCE globally, not per project.\n');

  const repoUrl = options.repo;
  if (!repoUrl) {
    console.error('Error: Repository name required');
    console.error('\nUsage:');
    console.error('  kindling sync init --repo username/kindling-memory --private');
    console.error('\nThis creates: https://github.com/username/kindling-memory');
    process.exit(1);
  }

  // Create local sync directory
  const syncDir = join(homedir(), '.kindling-sync');

  if (existsSync(syncDir)) {
    console.log('⚠️  Sync directory already exists:', syncDir);
    console.log('    Reinitializing...\n');
  } else {
    console.log(`📁 Creating sync directory: ${syncDir}`);
    mkdirSync(syncDir, { recursive: true });
  }

  // Initialize git repo
  if (!existsSync(join(syncDir, '.git'))) {
    execSync('git init', { cwd: syncDir, stdio: 'pipe' });
  }

  // Set remote
  try {
    execSync(`git remote remove origin`, { cwd: syncDir, stdio: 'pipe' });
  } catch {
    // Remote doesn't exist, that's fine
  }
  execSync(`git remote add origin https://github.com/${repoUrl}.git`, { cwd: syncDir });

  // Create .kindling directory structure
  mkdirSync(join(syncDir, '.kindling', 'capsules'), { recursive: true });
  mkdirSync(join(syncDir, '.kindling', 'pins'), { recursive: true });
  mkdirSync(join(syncDir, '.kindling', 'observations'), { recursive: true });

  // Create README
  writeFileSync(
    join(syncDir, 'README.md'),
    `# kindling Memory Sync

This repository contains synced kindling memory for integration with Claude Code Web.

## ⚠️ IMPORTANT: This should be PRIVATE

This repo contains your development history. Make it private on GitHub.

## Usage as Git Submodule

Add this repo as a submodule in your projects:

\`\`\`bash
cd ~/projects/my-app
git submodule add https://github.com/${repoUrl}.git .kindling
git commit -m "Add kindling memory submodule"
\`\`\`

Or use the CLI:

\`\`\`bash
cd ~/projects/my-app
kindling sync add-submodule
\`\`\`

## Structure

- \`.kindling/index.json\` - Memory index with pins and summaries
- \`.kindling/capsules/\` - Recent capsule data
- \`.kindling/pins/\` - Active pins
- \`.kindling/observations/\` - Recent observations

## Claude Code Web Usage

When Claude Code Web connects to your project repo (with this as a submodule), it can:

\`\`\`javascript
// Read memory index (automatically available via submodule)
const memory = JSON.parse(fs.readFileSync('.kindling/index.json', 'utf8'));

// Find relevant pins
const pins = memory.pins.filter(p =>
  p.scopeIds.repoId === 'my-project'
);

console.log(\`Found \${pins.length} pinned items\`);
\`\`\`

## Security

- Redacted observations are NEVER synced
- Only last 30 days by default (configurable)
- Private repo recommended
`,
  );

  // Create .gitignore
  writeFileSync(
    join(syncDir, '.gitignore'),
    `# Ignore large files
*.db
*.db-shm
*.db-wal

# Ignore OS files
.DS_Store
Thumbs.db
`,
  );

  // Initial commit
  try {
    execSync('git add .', { cwd: syncDir, stdio: 'pipe' });
    execSync('git commit -m "Initial kindling memory sync setup"', {
      cwd: syncDir,
      stdio: 'pipe',
    });
  } catch {
    // No changes or already committed
  }

  // Save config
  saveSyncConfig({ repo: repoUrl });

  console.log('✅ Sync initialized!\n');
  console.log('📝 Next steps:');
  console.log(`   1. Create PRIVATE GitHub repo:`);
  console.log(`      https://github.com/new`);
  console.log(`      Name: kindling-memory`);
  console.log(`      Visibility: Private ✓\n`);
  console.log(`   2. Push to GitHub:`);
  console.log(`      kindling sync push\n`);
  console.log(`   3. Add to your projects:`);
  console.log(`      cd ~/projects/my-app`);
  console.log(`      kindling sync add-submodule`);
}

/**
 * Add kindling memory as submodule to current project
 *
 * Run this in EACH project that should have access to kindling memory.
 */
export async function syncAddSubmoduleCommand(_options: SyncOptions): Promise<void> {
  console.log('📦 Adding kindling memory submodule...\n');

  // Get repo URL from config
  const config = getSyncConfig();
  if (!config.repo) {
    console.error('Error: kindling sync not initialized.');
    console.error('\nRun first:');
    console.error('  kindling sync init --repo username/kindling-memory');
    process.exit(1);
  }

  // Check if in a git repo
  if (!existsSync('.git')) {
    console.error('Error: Not a git repository.');
    console.error('Run `git init` first.');
    process.exit(1);
  }

  // Check if .kindling already exists
  if (existsSync('.kindling')) {
    console.error('Error: .kindling already exists in this project.');
    console.error('Remove it first if you want to replace it with the submodule.');
    process.exit(1);
  }

  console.log(`Adding submodule: https://github.com/${config.repo}.git`);

  try {
    // Add submodule
    execSync(`git submodule add https://github.com/${config.repo}.git .kindling`, {
      stdio: 'inherit',
    });

    // Initialize and update submodule
    execSync('git submodule update --init', { stdio: 'pipe' });

    // Commit the change
    execSync('git add .gitmodules .kindling', { stdio: 'pipe' });
    execSync('git commit -m "Add kindling memory submodule"', { stdio: 'pipe' });

    console.log('\n✅ Submodule added successfully!\n');
    console.log('📂 Your project structure:');
    console.log('   my-app/');
    console.log('     ├── src/              ← Your code');
    console.log('     ├── .kindling/        ← Submodule (memory repo)');
    console.log('     │   ├── index.json');
    console.log('     │   ├── capsules/');
    console.log('     │   └── pins/');
    console.log('     ├── .gitmodules       ← Git submodule config');
    console.log('     └── ...\n');
    console.log('📤 Next steps:');
    console.log('   1. Push changes: git push');
    console.log('   2. Sync memory: kindling sync push');
    console.log('   3. Update submodule: git submodule update --remote --merge');
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    console.error('\n❌ Error adding submodule:', message);
    console.error('\nCommon issues:');
    console.error('  - GitHub repo not created yet');
    console.error('  - No push access to the repo');
    console.error('  - .kindling directory already exists');
    process.exit(1);
  }
}

/**
 * Push current kindling state to GitHub
 */
export async function syncPushCommand(options: SyncOptions): Promise<void> {
  const dbPath = options.db || getDefaultDbPath();
  const syncDir = join(homedir(), '.kindling-sync');

  if (!existsSync(syncDir)) {
    console.error('Error: kindling sync not initialized.');
    console.error('Run: kindling sync init --repo username/kindling-memory');
    process.exit(1);
  }

  console.log('🔄 Syncing kindling memory to GitHub...\n');

  const db = openDatabase({ path: dbPath });
  const store = new SqliteKindlingStore(db);
  const provider = new LocalFtsProvider(db);
  const service = new KindlingService({ store, provider });

  // Export kindling data
  const bundle = service.export({
    includeRedacted: false, // Never sync redacted data
  });

  // Build index
  const index = {
    version: '1.0',
    syncedAt: new Date().toISOString(),
    stats: {
      observations: bundle.dataset.observations.length,
      capsules: bundle.dataset.capsules.length,
      summaries: bundle.dataset.summaries.length,
      pins: bundle.dataset.pins.length,
    },
    pins: bundle.dataset.pins
      .map((pin: unknown) => pin as Record<string, unknown>)
      .map((pin: Record<string, unknown>) => ({
        id: pin.id,
        targetType: pin.targetType,
        targetId: pin.targetId,
        reason: pin.reason,
        scopeIds: pin.scopeIds,
        createdAt: pin.createdAt,
      })),
    recentCapsules: bundle.dataset.capsules
      .map((c: unknown) => c as Record<string, unknown>)
      .sort((a: Record<string, unknown>, b: Record<string, unknown>) => {
        const aTime = (a.closedAt as number) || (a.openedAt as number);
        const bTime = (b.closedAt as number) || (b.openedAt as number);
        return bTime - aTime;
      })
      .slice(0, 50) // Last 50 capsules
      .map((c: Record<string, unknown>) => ({
        id: c.id,
        type: c.type,
        intent: c.intent,
        status: c.status,
        openedAt: c.openedAt,
        closedAt: c.closedAt,
        scopeIds: c.scopeIds,
        summaryId: c.summaryId,
      })),
    summaries: bundle.dataset.summaries
      .map((s: unknown) => s as Record<string, unknown>)
      .map((s: Record<string, unknown>) => ({
        id: s.id,
        capsuleId: s.capsuleId,
        content: s.content,
        confidence: s.confidence,
        createdAt: s.createdAt,
      })),
  };

  // Write index
  writeFileSync(join(syncDir, '.kindling', 'index.json'), JSON.stringify(index, null, 2));

  // Write individual capsule files (for easy browsing)
  for (const capsule of bundle.dataset.capsules.slice(-50)) {
    writeFileSync(
      join(syncDir, '.kindling', 'capsules', `${capsule.id}.json`),
      JSON.stringify(capsule, null, 2),
    );
  }

  // Write pins
  for (const pin of bundle.dataset.pins) {
    writeFileSync(
      join(syncDir, '.kindling', 'pins', `${pin.id}.json`),
      JSON.stringify(pin, null, 2),
    );
  }

  console.log('✅ Memory exported to sync directory');
  console.log(`   ${index.stats.observations} observations`);
  console.log(`   ${index.stats.capsules} capsules`);
  console.log(`   ${index.stats.summaries} summaries`);
  console.log(`   ${index.stats.pins} pins\n`);

  // Git commit and push
  try {
    execSync('git add .', { cwd: syncDir, stdio: 'pipe' });
    execSync(`git commit -m "Sync kindling memory at ${new Date().toISOString()}"`, {
      cwd: syncDir,
      stdio: 'pipe',
    });
    execSync(`git push origin ${options.branch || 'main'}`, {
      cwd: syncDir,
      stdio: 'inherit',
    });
    console.log('✅ Pushed to GitHub\n');
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    if (typeof message === 'string' && message.includes('nothing to commit')) {
      console.log('ℹ️  No changes to sync\n');
    } else {
      console.error('❌ Error pushing to GitHub:', message);
      console.error('\nCommon issues:');
      console.error('  - GitHub repo not created yet');
      console.error('  - No push access');
      console.error('  - Wrong branch name');
      process.exit(1);
    }
  }

  db.close();
  console.log('🎉 Sync complete!');
}
