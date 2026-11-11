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
 *
 * Staging is a unique `.vsix-staging-*` directory per run so concurrent
 * packaging (even with separate `--out` directories) cannot delete one
 * another's intermediates. vsce is invoked as `node <vsce-cli> ...` with an
 * argv array and `shell: false`, so Windows paths with spaces or
 * metacharacters are not re-parsed by cmd.exe.
 */

import { spawnSync } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { build } from 'esbuild';

const pkgDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(pkgDir, '..', '..');
const require = createRequire(import.meta.url);

const EXTENSION_NAME = 'kindling';
// VS Code 1.85 runs extensions on Node 18.
const NODE_TARGET = 'node18';
const REPO_URL = 'https://github.com/eddacraft/kindling';
const PACKAGE_PATH = 'packages/kindling-adapter-vscode';
const STAGING_PREFIX = join(pkgDir, '.vsix-staging-');

export function parseOutDir(argv) {
  const index = argv.indexOf('--out');
  if (index === -1) return pkgDir;
  const value = argv[index + 1];
  if (!value) throw new Error('--out requires a directory argument');
  return resolve(value);
}

export function createStagingDir() {
  return mkdtempSync(STAGING_PREFIX);
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

/** Resolve @vscode/vsce's JS CLI so we never spawn `vsce.cmd` through a shell. */
export function resolveVsceCli() {
  const pkgJsonPath = require.resolve('@vscode/vsce/package.json');
  const pkg = readJson(pkgJsonPath);
  const binField = pkg.bin;
  const rel = typeof binField === 'string' ? binField : binField?.vsce;
  if (!rel) {
    throw new Error(`could not resolve @vscode/vsce bin from ${pkgJsonPath}`);
  }
  const cli = resolve(dirname(pkgJsonPath), rel);
  if (!existsSync(cli)) {
    throw new Error(`vsce not found at ${cli}; run pnpm install first`);
  }
  return cli;
}

function vscePackageArgs(outFile) {
  return [
    'package',
    '--no-dependencies',
    '--no-update-package-json',
    '--baseContentUrl',
    `${REPO_URL}/blob/main/${PACKAGE_PATH}`,
    '--baseImagesUrl',
    `${REPO_URL}/raw/main/${PACKAGE_PATH}`,
    '--out',
    outFile,
  ];
}

/**
 * Command + argv used to invoke vsce. `shell` is always false: paths are
 * discrete arguments, not interpolated into a cmd.exe command line.
 */
export function vsceSpawnInvocation(stagingDir, outFile, cli = resolveVsceCli()) {
  return {
    command: process.execPath,
    args: [cli, ...vscePackageArgs(outFile)],
    options: { cwd: stagingDir, stdio: 'inherit', shell: false },
  };
}

export function runVsce(stagingDir, outFile, { spawn = spawnSync, cli = resolveVsceCli() } = {}) {
  const { command, args, options } = vsceSpawnInvocation(stagingDir, outFile, cli);
  const result = spawn(command, args, options);
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`vsce package exited with status ${result.status}`);
  }
}

async function fillStaging(stagingDir) {
  const source = readJson(join(pkgDir, 'package.json'));
  mkdirSync(join(stagingDir, 'dist', 'schema'), { recursive: true });
  await bundle(join(stagingDir, 'dist', 'extension.cjs'));
  copyFileSync(schemaVersionFile(), join(stagingDir, 'dist', 'schema', 'version.json'));
  copyFileSync(join(pkgDir, 'README.md'), join(stagingDir, 'README.md'));
  copyFileSync(join(repoRoot, 'LICENSE'), join(stagingDir, 'LICENSE'));
  writeFileSync(
    join(stagingDir, 'package.json'),
    `${JSON.stringify(extensionManifest(source), null, 2)}\n`,
  );
}

export async function packageVsix({
  argv = process.argv.slice(2),
  spawn = spawnSync,
  prepareStaging = fillStaging,
} = {}) {
  const outDir = parseOutDir(argv);
  const source = readJson(join(pkgDir, 'package.json'));
  const outFile = join(outDir, `${EXTENSION_NAME}-${source.version}.vsix`);
  const stagingDir = createStagingDir();

  try {
    mkdirSync(outDir, { recursive: true });
    await prepareStaging(stagingDir);
    runVsce(stagingDir, outFile, { spawn });
    return { outFile, stagingDir };
  } finally {
    rmSync(stagingDir, { recursive: true, force: true });
  }
}

function invokedDirectly() {
  const entry = process.argv[1];
  if (!entry) return false;
  return import.meta.url === pathToFileURL(resolve(entry)).href;
}

if (invokedDirectly()) {
  packageVsix()
    .then(({ outFile }) => {
      console.log(`VSIX written to ${outFile}`);
    })
    .catch((error) => {
      console.error(error instanceof Error ? error.message : error);
      process.exit(1);
    });
}
