#!/usr/bin/env node
// Generate the per-platform binary packages and inject the main package's
// `optionalDependencies` at publish time.
//
// `@eddacraft/kindling` ships NO binary itself. The PUBLISHED package declares
// one `optionalDependencies` entry per supported host — `@eddacraft/kindling-
// <os>-<arch>[-musl]` — each a tiny package containing a single prebuilt
// `kindling` binary plus `os`/`cpu`/`libc` fields. npm/pnpm install ONLY the
// entry matching the host (the rest are skipped, no postinstall, works under
// `--ignore-scripts`). At runtime `config.ts:resolveBinaryPath` resolves the
// matching package via `require.resolve`. This mirrors esbuild's `@esbuild/*`
// model.
//
// IMPORTANT — why these are NOT committed: the `optionalDependencies` are
// injected into package.json at publish time, never committed to source. The
// platform packages don't exist on the registry until a release publishes them,
// so committing them would put unresolvable specifiers in package.json and break
// `pnpm install --frozen-lockfile` (the lockfile can't record packages that
// 404). Instead, `publish.yml` runs `--write` to inject them right before
// `pnpm publish -r`, so the published artifact carries them while the source
// tree stays installable. `--check` enforces that they stay out of the committed
// manifest.
//
// The 7 targets here are the SAME targets cross-built by
// `.github/workflows/_cross-build.yml` and detected by `install.sh`'s
// `detect_target`.
//
// Usage:
//   # Inject optionalDependencies into package.json (done at publish time):
//   node scripts/build-platform-packages.mjs --write
//
//   # CI guard: fail if optionalDependencies have been committed to source:
//   node scripts/build-platform-packages.mjs --check
//
//   # Publish-time: inject deps AND materialize the 7 packages with their
//   # binaries, reading each from <bin-dir>/<rust-target>/kindling[.exe]:
//   node scripts/build-platform-packages.mjs --write --bin-dir <dir>
//
// The materialized packages are written under `platforms/` (git-ignored) and are
// published directly with `npm publish` per directory — they are NOT pnpm
// workspace members.

import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const packageRoot = join(here, '..');
const mainManifestPath = join(packageRoot, 'package.json');
const platformsDir = join(packageRoot, 'platforms');

const SCOPE = '@eddacraft';

/**
 * The canonical target table — one row per published platform package, in
 * lock-step with `.github/workflows/release.yml`'s build matrix.
 */
const TARGETS = [
  { rustTarget: 'x86_64-unknown-linux-gnu', pkg: 'kindling-linux-x64', os: 'linux', cpu: 'x64', libc: 'glibc', exe: false },
  { rustTarget: 'aarch64-unknown-linux-gnu', pkg: 'kindling-linux-arm64', os: 'linux', cpu: 'arm64', libc: 'glibc', exe: false },
  { rustTarget: 'x86_64-unknown-linux-musl', pkg: 'kindling-linux-x64-musl', os: 'linux', cpu: 'x64', libc: 'musl', exe: false },
  { rustTarget: 'aarch64-unknown-linux-musl', pkg: 'kindling-linux-arm64-musl', os: 'linux', cpu: 'arm64', libc: 'musl', exe: false },
  { rustTarget: 'x86_64-apple-darwin', pkg: 'kindling-darwin-x64', os: 'darwin', cpu: 'x64', exe: false },
  { rustTarget: 'aarch64-apple-darwin', pkg: 'kindling-darwin-arm64', os: 'darwin', cpu: 'arm64', exe: false },
  { rustTarget: 'x86_64-pc-windows-gnu', pkg: 'kindling-win32-x64', os: 'win32', cpu: 'x64', exe: true },
];

function parseArgs(argv) {
  const args = { check: false, write: false, binDir: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--check') args.check = true;
    else if (a === '--write') args.write = true;
    else if (a === '--bin-dir') args.binDir = argv[++i];
    else throw new Error(`unknown argument: ${a}`);
  }
  if (!args.check && !args.write) {
    throw new Error('specify --check or --write (optionally with --bin-dir <dir>)');
  }
  return args;
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

/** The full scoped package name for a target. */
function fullName(target) {
  return `${SCOPE}/${target.pkg}`;
}

/** Build the canonical optionalDependencies block for a given version. */
function optionalDepsFor(version) {
  const deps = {};
  for (const t of TARGETS) deps[fullName(t)] = version;
  return deps;
}

/** Human-readable platform label, e.g. "linux x64 (musl)". */
function platformLabel(t) {
  const arch = t.cpu === 'x64' ? 'x64' : t.cpu;
  const os = t.os === 'win32' ? 'windows' : t.os;
  return t.libc ? `${os} ${arch} (${t.libc})` : `${os} ${arch}`;
}

/** The platform package.json manifest object. */
function platformManifest(t, version, mainManifest) {
  return {
    name: fullName(t),
    version,
    description: `Prebuilt kindling binary for ${platformLabel(t)}.`,
    license: mainManifest.license,
    homepage: mainManifest.homepage,
    repository: mainManifest.repository,
    bugs: mainManifest.bugs,
    os: [t.os],
    cpu: [t.cpu],
    // `libc` (npm 10+/pnpm) selects glibc vs musl on Linux; omitted elsewhere.
    ...(t.libc ? { libc: [t.libc] } : {}),
    files: ['bin'],
    // Yarn PnP: keep the binary unzipped on disk so it's executable.
    preferUnplugged: true,
  };
}

/** Inject the canonical optionalDependencies into the main package manifest. */
function writeOptionalDeps(version) {
  const manifest = readJson(mainManifestPath);
  manifest.optionalDependencies = optionalDepsFor(version);
  writeFileSync(mainManifestPath, JSON.stringify(manifest, null, 2) + '\n');
}

/** Materialize the 7 platform packages, copying each binary from binDir. */
function materializePackages(version, mainManifest, binDir) {
  rmSync(platformsDir, { recursive: true, force: true });
  mkdirSync(platformsDir, { recursive: true });
  for (const t of TARGETS) {
    const binName = t.exe ? 'kindling.exe' : 'kindling';
    const src = join(binDir, t.rustTarget, binName);
    if (!existsSync(src)) {
      throw new Error(`missing binary for ${t.rustTarget}: expected ${src}`);
    }
    const pkgDir = join(platformsDir, t.pkg);
    const binDest = join(pkgDir, 'bin');
    mkdirSync(binDest, { recursive: true });
    copyFileSync(src, join(binDest, binName));
    if (!t.exe) chmodSync(join(binDest, binName), 0o755);
    const manifest = platformManifest(t, version, mainManifest);
    writeFileSync(
      join(pkgDir, 'package.json'),
      JSON.stringify(manifest, null, 2) + '\n',
    );
    console.log(`  ${fullName(t)} -> ${pkgDir} (${binName})`);
  }
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const mainManifest = readJson(mainManifestPath);
  const { version } = mainManifest;

  if (args.check) {
    // The platform packages don't exist on the registry until a release
    // publishes them, so `optionalDependencies` must NOT be committed to source
    // — they would put unresolvable specifiers in package.json and break
    // `pnpm install --frozen-lockfile`. They are injected at publish time.
    const committed = mainManifest.optionalDependencies;
    if (committed && Object.keys(committed).length > 0) {
      console.error(
        `${mainManifestPath} must NOT commit optionalDependencies — they are ` +
          `injected at publish time (see this script's header). Remove the ` +
          `optionalDependencies block from the committed manifest.`,
      );
      process.exit(1);
    }
    console.log(
      `OK: no committed optionalDependencies (${TARGETS.length} targets are injected at publish).`,
    );
    return;
  }

  // --write: inject the optionalDependencies (publish-time step).
  writeOptionalDeps(version);
  console.log(`Injected optionalDependencies for ${TARGETS.length} targets @ ${version}.`);

  if (args.binDir) {
    console.log(`Materializing platform packages from ${args.binDir}:`);
    materializePackages(version, mainManifest, args.binDir);
  }
}

main();
