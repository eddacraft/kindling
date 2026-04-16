#!/usr/bin/env node
/**
 * Build the Kindling plugin bundle using esbuild.
 *
 * Bundles all @eddacraft/kindling-* packages into a single CJS file,
 * with better-sqlite3 marked as external (native module).
 *
 * Includes a plugin to inline SQL migration files that would
 * otherwise fail to resolve at runtime in the bundled context.
 */

const path = require('path');
const fs = require('fs');

const pluginRoot = path.resolve(__dirname, '..');
const monorepoRoot = path.resolve(pluginRoot, '..', '..');
const distDir = path.join(pluginRoot, 'dist');
const entryPoint = path.join(pluginRoot, 'scripts', 'bundle-entry.js');
const outFile = path.join(distDir, 'kindling-bundle.cjs');

// Ensure dist directory exists
if (!fs.existsSync(distDir)) {
  fs.mkdirSync(distDir, { recursive: true });
}

// Check if we're in the monorepo (packages available)
const packagesDir = path.join(monorepoRoot, 'packages');
const requiredPackages = [
  'kindling-core',
  'kindling-store-sqlite',
  'kindling-provider-local',
  'kindling-adapter-claude-code',
];

for (const pkg of requiredPackages) {
  const distPath = path.join(packagesDir, pkg, 'dist');
  if (!fs.existsSync(distPath)) {
    // Not in monorepo or packages not built — use existing bundle if available
    if (fs.existsSync(outFile)) {
      console.log('[kindling] Using existing bundle (monorepo packages not available).');
      process.exit(0);
    }
    console.error(
      `[kindling] Package ${pkg} not built. Run 'pnpm build' from the monorepo root first.`,
    );
    process.exit(1);
  }
}

// Resolve @eddacraft/kindling-* packages to monorepo dist directories
const alias = {};
for (const pkg of requiredPackages) {
  const scopedName = `@eddacraft/${pkg}`;
  alias[scopedName] = path.join(packagesDir, pkg, 'dist', 'index.js');
}

// Read migration SQL files and generate an inline migration module
const migrationsDir = path.join(packagesDir, 'kindling-store-sqlite', 'migrations');
const migrationFiles = fs
  .readdirSync(migrationsDir)
  .filter((f) => f.endsWith('.sql'))
  .sort();
const inlineMigrations = migrationFiles.map((file) => {
  const sql = fs.readFileSync(path.join(migrationsDir, file), 'utf-8');
  const name = file.replace('.sql', '');
  const versionMatch = file.match(/^(\d+)_/);
  const version = versionMatch ? parseInt(versionMatch[1], 10) : 0;
  return `{ version: ${version}, name: ${JSON.stringify(name)}, sql: ${JSON.stringify(sql)} }`;
});

// Create inline migration module content
const inlineMigrateModule = `
function getMigrations() {
  return [${inlineMigrations.join(',\n')}];
}

function getCurrentVersion(db) {
  try {
    const row = db.prepare('SELECT MAX(version) as version FROM schema_migrations').get();
    return row?.version ?? 0;
  } catch {
    return 0;
  }
}

export function runMigrations(db) {
  const currentVersion = getCurrentVersion(db);
  const migrations = getMigrations();
  let applied = 0;
  for (const migration of migrations) {
    if (migration.version > currentVersion) {
      const applyMigration = db.transaction(() => {
        db.exec(migration.sql);
      });
      applyMigration();
      applied++;
    }
  }
  return applied;
}

export function getMigrationStatus(db) {
  const currentVersion = getCurrentVersion(db);
  const migrations = getMigrations();
  const latestVersion = Math.max(...migrations.map(m => m.version));
  const appliedMigrations = [];
  const pendingMigrations = [];
  for (const migration of migrations) {
    if (migration.version <= currentVersion) {
      appliedMigrations.push(migration.name);
    } else {
      pendingMigrations.push(migration.name);
    }
  }
  return { currentVersion, latestVersion, appliedMigrations, pendingMigrations };
}
`;

// esbuild plugin to replace the migrate module with inlined SQL
const inlineMigrationsPlugin = {
  name: 'inline-migrations',
  setup(build) {
    // Intercept resolves for the migrate module
    build.onResolve({ filter: /migrate\.js$/ }, (args) => {
      if (args.resolveDir.includes('kindling-store-sqlite')) {
        return { path: 'inline-migrate', namespace: 'inline' };
      }
      return null;
    });

    // Return the inlined module
    build.onLoad({ filter: /^inline-migrate$/, namespace: 'inline' }, () => {
      return { contents: inlineMigrateModule, loader: 'js' };
    });
  },
};

async function build() {
  const esbuild = require('esbuild');

  await esbuild.build({
    entryPoints: [entryPoint],
    bundle: true,
    format: 'cjs',
    platform: 'node',
    target: 'node18',
    outfile: outFile,
    external: ['better-sqlite3'],
    alias,
    plugins: [inlineMigrationsPlugin],
    minify: false,
    sourcemap: false,
    logLevel: 'info',
  });

  const stats = fs.statSync(outFile);
  const sizeKB = Math.round(stats.size / 1024);
  console.log(`[kindling] Bundle built: ${outFile} (${sizeKB} KB)`);
}

build().catch((err) => {
  console.error('[kindling] Bundle build failed:', err.message);
  process.exit(1);
});
