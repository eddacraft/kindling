/**
 * Regression tests for VSIX packaging isolation and Windows path handling
 * (eddacraft/kindling#200).
 *
 * The packager used a fixed `.vsix-staging` directory (so concurrent runs
 * deleted one another's intermediates) and spawned `vsce.cmd` with `shell:
 * true` on Windows (so paths with spaces or metacharacters were re-parsed by
 * cmd.exe).
 */

import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { afterEach, describe, expect, it } from 'vitest';

import {
  createStagingDir,
  packageVsix,
  parseOutDir,
  resolveVsceCli,
  runVsce,
  vsceSpawnInvocation,
} from '../scripts/package-vsix.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const pkgDir = resolve(here, '..');
const overlapChild = join(here, 'package-vsix-overlap-child.mjs');
const sharedStaging = join(pkgDir, '.vsix-staging');

const tempDirs: string[] = [];

function tempDir(prefix: string): string {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true });
  }
});

describe('package-vsix staging', () => {
  it('allocates a unique directory per call, not the shared .vsix-staging path', () => {
    const first = createStagingDir();
    const second = createStagingDir();
    tempDirs.push(first, second);

    expect(first).not.toBe(sharedStaging);
    expect(second).not.toBe(sharedStaging);
    expect(first).not.toBe(second);
    expect(first.startsWith(join(pkgDir, '.vsix-staging-'))).toBe(true);
    expect(second.startsWith(join(pkgDir, '.vsix-staging-'))).toBe(true);
    expect(existsSync(first)).toBe(true);
    expect(existsSync(second)).toBe(true);

    writeFileSync(join(first, 'owner'), 'a');
    writeFileSync(join(second, 'owner'), 'b');
    rmSync(first, { recursive: true, force: true });
    expect(readFileSync(join(second, 'owner'), 'utf8')).toBe('b');
  });

  it('runs vsce in a unique staging directory and removes it afterwards', async () => {
    const outDir = tempDir('kindling-vsix-out-');
    let cwd = '';
    await packageVsix({
      argv: ['--out', outDir],
      prepareStaging: async () => {},
      spawn: (_command, _args, options) => {
        cwd = options.cwd;
        expect(existsSync(options.cwd)).toBe(true);
        return { status: 0 };
      },
    });
    expect(cwd).not.toBe(sharedStaging);
    expect(cwd.startsWith(join(pkgDir, '.vsix-staging-'))).toBe(true);
    expect(existsSync(cwd)).toBe(false);
  });

  it('overlapping packaging runs keep isolated staging directories', async () => {
    const outA = tempDir('kindling-vsix-out-a-');
    const outB = tempDir('kindling-vsix-out-b-');

    const [runA, runB] = await Promise.all([runOverlapChild(outA), runOverlapChild(outB)]);

    expect(runA.status, runA.stderr).toBe(0);
    expect(runB.status, runB.stderr).toBe(0);
    expect(runA.stagingDir).toBeTruthy();
    expect(runB.stagingDir).toBeTruthy();
    expect(runA.stagingDir).not.toBe(runB.stagingDir);
    expect(runA.stagingDir).not.toBe(sharedStaging);
    expect(runB.stagingDir).not.toBe(sharedStaging);
    expect(existsSync(runA.stagingDir)).toBe(false);
    expect(existsSync(runB.stagingDir)).toBe(false);
  }, 60_000);
});

describe('package-vsix vsce invocation', () => {
  it('invokes vsce via node with shell disabled and argv, not vsce.cmd', () => {
    const outFile = join(tmpdir(), 'Kindling Out Dir & more', 'kindling-0.0.0.vsix');
    const invocation = vsceSpawnInvocation('/tmp/staging', outFile);

    expect(invocation.command).toBe(process.execPath);
    expect(invocation.options.shell).toBe(false);
    expect(invocation.args[0]).toBe(resolveVsceCli());
    expect(invocation.args[0].endsWith('vsce.cmd')).toBe(false);
    expect(invocation.args[0].endsWith('.cmd')).toBe(false);

    const outIndex = invocation.args.indexOf('--out');
    expect(outIndex).toBeGreaterThan(-1);
    expect(invocation.args[outIndex + 1]).toBe(outFile);
    expect(invocation.args.join(' ')).not.toBe(invocation.args[outIndex + 1]);
  });

  it('delivers --out paths with spaces and metacharacters as a single argument', () => {
    const work = tempDir('kindling-vsix-probe-');
    const probe = join(work, 'probe.mjs');
    const captured = join(work, 'argv.json');
    const outFile = join(work, 'out dir & | ; %PATH% (more)', 'kindling-0.0.0.vsix');
    mkdirSync(dirname(outFile), { recursive: true });
    writeFileSync(
      probe,
      [
        "import { writeFileSync } from 'node:fs';",
        'writeFileSync(process.env.KINDLING_ARGV_OUT, JSON.stringify(process.argv));',
      ].join('\n'),
    );

    runVsce(work, outFile, {
      cli: probe,
      spawn: (command, args, options) =>
        spawnSync(command, args, {
          ...options,
          env: { ...process.env, KINDLING_ARGV_OUT: captured },
          stdio: 'pipe',
        }),
    });

    const argv = JSON.parse(readFileSync(captured, 'utf8')) as string[];
    expect(argv[0]).toBe(process.execPath);
    expect(argv[1]).toBe(probe);
    const outIndex = argv.indexOf('--out');
    expect(argv[outIndex + 1]).toBe(outFile);
  });

  it('resolves --out directories that contain spaces', () => {
    const dir = join(tmpdir(), 'kindling vsix out');
    expect(parseOutDir(['--out', dir])).toBe(resolve(dir));
  });
});

function runOverlapChild(
  outDir: string,
): Promise<{ status: number; stagingDir: string; stderr: string }> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(process.execPath, [overlapChild, outDir, '400'], {
      cwd: pkgDir,
      env: process.env,
    });
    let stdout = '';
    let stderr = '';
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (chunk: string) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk: string) => {
      stderr += chunk;
    });
    child.on('error', reject);
    child.on('close', (status) => {
      const recorded = join(outDir, 'staging-dir.txt');
      resolvePromise({
        status: status ?? 1,
        stagingDir: existsSync(recorded) ? readFileSync(recorded, 'utf8') : stdout.trim(),
        stderr,
      });
    });
  });
}
