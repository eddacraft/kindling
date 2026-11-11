/**
 * Child process for the overlapping VSIX packaging test.
 *
 * Two of these run at once, each with its own `--out` directory. The fake
 * vsce holds the staging directory open long enough that a shared
 * `.vsix-staging` folder would have been deleted by the sibling run.
 */

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { packageVsix } from '../scripts/package-vsix.mjs';

const outDir = process.argv[2];
const holdMs = Number(process.argv[3] ?? '400');
if (!outDir) {
  throw new Error('usage: package-vsix-overlap-child.mjs <outDir> [holdMs]');
}

await packageVsix({
  argv: ['--out', outDir],
  // Skip esbuild/vsce: this child only asserts staging isolation.
  prepareStaging(stagingDir) {
    mkdirSync(join(stagingDir, 'dist'), { recursive: true });
    writeFileSync(join(stagingDir, 'dist', 'extension.cjs'), '// overlap-test stub\n');
    writeFileSync(join(stagingDir, 'package.json'), '{}\n');
  },
  spawn(_command, _args, options) {
    const canary = join(options.cwd, '.overlap-canary');
    writeFileSync(canary, 'ok');
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, holdMs);
    if (!existsSync(canary)) {
      throw new Error(`staging directory was destroyed during vsce: ${options.cwd}`);
    }
    if (!existsSync(join(options.cwd, 'dist', 'extension.cjs'))) {
      throw new Error(`bundle was destroyed during vsce: ${options.cwd}`);
    }
    if (!existsSync(join(options.cwd, 'package.json'))) {
      throw new Error(`manifest was destroyed during vsce: ${options.cwd}`);
    }
    writeFileSync(join(outDir, 'staging-dir.txt'), options.cwd);
    return { status: 0 };
  },
});
