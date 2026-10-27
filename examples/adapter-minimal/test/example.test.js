/**
 * Runs the built minimal adapter example against a REAL kindling daemon and
 * checks it completes the documented journey: health → openCapsule →
 * appendObservation → retrieve → closeCapsule.
 *
 * The example takes its socket and binary from the thin client's environment
 * overrides (`KINDLING_SOCK`, `KINDLING_BIN`), so each run gets a fresh temp
 * home and never touches the developer's ~/.kindling. The suite skips when the
 * debug binary (`cargo build -p eddacraft-kindling --bin kindling`) is absent.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const exampleDir = join(here, '..');
const repoRoot = join(exampleDir, '..', '..');
const BINARY = process.env.KINDLING_BIN || join(repoRoot, 'target', 'debug', 'kindling');
const ENTRY = join(exampleDir, 'dist', 'index.js');

const skip = existsSync(BINARY)
  ? false
  : `daemon binary not found at ${BINARY}; build with: cargo build -p eddacraft-kindling --bin kindling`;

/** Stop the daemon the example auto-spawned, so no process outlives the test. */
function stopDaemon(home) {
  const pidFile = join(home, 'kindling.pid');
  if (!existsSync(pidFile)) return;
  const pid = Number.parseInt(readFileSync(pidFile, 'utf8').trim(), 10);
  if (Number.isInteger(pid) && pid > 0) {
    try {
      process.kill(pid, 'SIGTERM');
    } catch {
      // Already gone.
    }
  }
}

test('minimal adapter example completes its journey against a live daemon', { skip }, () => {
  assert.ok(existsSync(ENTRY), `build the example first (missing ${ENTRY})`);

  const home = mkdtempSync(join(tmpdir(), 'kindling-example-'));
  const project = mkdtempSync(join(tmpdir(), 'kindling-example-repo-'));
  try {
    const run = spawnSync(process.execPath, [ENTRY], {
      cwd: project,
      encoding: 'utf8',
      timeout: 30_000,
      env: {
        ...process.env,
        HOME: home,
        USERPROFILE: home,
        KINDLING_SOCK: join(home, 'kindling.sock'),
        KINDLING_BIN: BINARY,
      },
    });

    assert.equal(run.status, 0, `example exited ${run.status}\n${run.stderr}`);
    assert.match(run.stdout, /^Connected to kindling \S+ \(schema v\d+\)$/m);
    assert.match(run.stdout, /^Opened capsule \S+$/m);
    const hits = /^Retrieve returned (\d+) candidate\(s\):$/m.exec(run.stdout);
    assert.ok(hits, `no retrieve line in:\n${run.stdout}`);
    assert.ok(Number(hits[1]) >= 1, 'retrieve found nothing for the logged JWT error');
    assert.match(run.stdout, /JWT validation failed/);
    assert.match(run.stdout, /^Closed capsule \S+ \(status: closed\)$/m);
  } finally {
    stopDaemon(home);
    rmSync(home, { recursive: true, force: true });
    rmSync(project, { recursive: true, force: true });
  }
});
