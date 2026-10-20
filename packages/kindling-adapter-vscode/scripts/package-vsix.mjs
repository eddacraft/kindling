#!/usr/bin/env node
/**
 * Package the kindling editor extension as a VSIX for VS Code, Cursor and
 * Windsurf.
 *
 * The npm package (`@eddacraft/kindling-adapter-vscode`) is ESM, scoped, and
 * depends on the `@eddacraft/kindling` workspace package. None of that suits a
 * VSIX directly: `vsce` rejects scoped names, does not follow pnpm workspace
 * links, and editors based on older VS Code builds load extensions as
 * CommonJS. So this script:
 *
 *   1. bundles `src/extension.ts` (and the thin client it imports) into a single
 *      CommonJS file with esbuild, leaving only `vscode` external;
 *   2. stages a minimal extension manifest, the bundle, the schema version file
 *      the client reads at load time, the README and the licence;
 *   3. runs `vsce package --no-dependencies` in that staging directory.
 *
 * Output: `kindling-<version>.vsix` in this package directory, or in the
 * directory given by `--out <dir>`.
 */

import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { build } from 'esbuild';

const pkgDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(pkgDir, '..', '..');
const stagingDir = join(pkgDir, '.vsix-staging');

const EXTENSION_NAME = 'kindling';
// VS Code 1.85 runs extensions on Node 18.
const NODE_TARGET = 'node18';
const REPO_URL = 'https://github.com/eddacraft/kindling';
const PACKAGE_PATH = 'packages/kindling-adapter-vscode';

function parseOutDir(argv) {
  const index = argv.indexOf('--out');
  if (index === -1) return pkgDir;
  const value = argv[index + 1];
  if (!value) throw new Error('--out requires a directory argument');
  return resolve(value);
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

/** Locate the schema version file the thin client resolves next to its code. */
function schemaVersionFile() {
  const candidates = [
    join(pkgDir, 'node_modules', '@eddacraft', 'kindling', 'schema', 'version.json'),
    join(repoRoot, 'schema', 'version.json'),
  ];
  const found = candidates.find((candidate) => existsSync(candidate));
  if (!found) {
    throw new Error(`could not find schema/version.json (looked in: ${candidates.join(', ')})`);
  }
  return found;
}

function extensionManifest(source) {
  return {
    name: EXTENSION_NAME,
    displayName: source.displayName,
    description: source.description,
    version: source.version,
    publisher: source.publisher,
    author: source.author,
    license: source.license,
    homepage: source.homepage,
    repository: source.repository,
    bugs: source.bugs,
    keywords: source.keywords,
    engines: { vscode: source.engines.vscode },
    categories: source.categories,
    activationEvents: source.activationEvents,
    contributes: source.contributes,
    main: './dist/extension.cjs',
    files: ['dist', 'README.md', 'LICENSE'],
  };
}

async function bundle(outfile) {
  await build({
    entryPoints: [join(pkgDir, 'src', 'extension.ts')],
    outfile,
    bundle: true,
    platform: 'node',
    format: 'cjs',
    target: NODE_TARGET,
    external: ['vscode'],
    sourcemap: false,
    minify: false,
    logLevel: 'warning',
    // The thin client resolves files relative to `import.meta.url`, which has
    // no CommonJS equivalent. Point it at the bundle's own location instead.
    banner: {
      js: "const __kindlingImportMetaUrl = require('node:url').pathToFileURL(__filename).href;",
    },
    define: { 'import.meta.url': '__kindlingImportMetaUrl' },
  });
}

function runVsce(outFile) {
  const vsce = join(
    pkgDir,
    'node_modules',
    '.bin',
    process.platform === 'win32' ? 'vsce.cmd' : 'vsce',
  );
  if (!existsSync(vsce)) {
    throw new Error(`vsce not found at ${vsce}; run pnpm install first`);
  }
  const result = spawnSync(
    vsce,
    [
      'package',
      '--no-dependencies',
      '--no-update-package-json',
      '--baseContentUrl',
      `${REPO_URL}/blob/main/${PACKAGE_PATH}`,
      '--baseImagesUrl',
      `${REPO_URL}/raw/main/${PACKAGE_PATH}`,
      '--out',
      outFile,
    ],
    { cwd: stagingDir, stdio: 'inherit', shell: process.platform === 'win32' },
  );
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`vsce package exited with status ${result.status}`);
  }
}

async function main() {
  const outDir = parseOutDir(process.argv.slice(2));
  const source = readJson(join(pkgDir, 'package.json'));
  const outFile = join(outDir, `${EXTENSION_NAME}-${source.version}.vsix`);

  rmSync(stagingDir, { recursive: true, force: true });
  mkdirSync(join(stagingDir, 'dist', 'schema'), { recursive: true });
  mkdirSync(outDir, { recursive: true });

  await bundle(join(stagingDir, 'dist', 'extension.cjs'));
  copyFileSync(schemaVersionFile(), join(stagingDir, 'dist', 'schema', 'version.json'));
  copyFileSync(join(pkgDir, 'README.md'), join(stagingDir, 'README.md'));
  copyFileSync(join(repoRoot, 'LICENSE'), join(stagingDir, 'LICENSE'));
  writeFileSync(
    join(stagingDir, 'package.json'),
    `${JSON.stringify(extensionManifest(source), null, 2)}\n`,
  );

  runVsce(outFile);
  rmSync(stagingDir, { recursive: true, force: true });
  console.log(`VSIX written to ${outFile}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
