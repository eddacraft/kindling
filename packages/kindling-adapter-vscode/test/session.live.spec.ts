/**
 * VS Code / Cursor / Windsurf capture journey against a REAL kindling daemon
 * (KFIT-002).
 *
 * The extension (`src/extension.ts`) wires editor events into
 * {@link EditorSessionManager} over the thin {@link Kindling} client: a session
 * capsule on activation, a `file_diff` observation per save, `kindling.search`
 * through `retrieve`, `kindling.status` through `health`, and a capsule close
 * on deactivation. This suite drives that same manager → client → daemon path
 * in a fresh profile (temp kindling home, unique socket, temp project root),
 * so the quickstart's journey is exercised end to end without a VS Code host.
 *
 * It SKIPS when the debug binary is absent (the TS-only `pnpm -r test`); the
 * `thin-client` CI job builds it so the suite actually runs.
 */

import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { afterEach, describe, expect, it } from 'vitest';

import { Kindling } from '@eddacraft/kindling';
import { EditorSessionManager } from '../src/session.js';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..', '..');
const BINARY = join(repoRoot, 'target', 'debug', 'kindling');
const HAS_BINARY = existsSync(BINARY);

if (!HAS_BINARY) {
  console.warn(
    `[vscode-adapter] daemon binary not found at ${BINARY} — skipping live-daemon ` +
      `journey tests. Build with: cargo build -p eddacraft-kindling --bin kindling`,
  );
}

const tempDirs: string[] = [];

/** A fresh profile: temp kindling home + socket, and a temp project root. */
function freshProfile(): { client: Kindling; home: string; projectRoot: string } {
  const home = mkdtempSync(join(tmpdir(), 'kindling-vsc-'));
  const projectRoot = mkdtempSync(join(tmpdir(), 'kindling-vsc-repo-'));
  tempDirs.push(home, projectRoot);
  const client = new Kindling({
    socketPath: join(home, 'kindling.sock'),
    projectRoot,
    binaryPath: BINARY,
    connectTimeoutMs: 5000,
  });
  return { client, home, projectRoot };
}

/** Stop the daemon a profile spawned, so the suite leaves no process behind. */
function stopDaemon(home: string): void {
  const pidFile = join(home, 'kindling.pid');
  if (!existsSync(pidFile)) return;
  const pid = Number.parseInt(readFileSync(pidFile, 'utf8').trim(), 10);
  if (Number.isInteger(pid) && pid > 0) {
    try {
      process.kill(pid, 'SIGTERM');
    } catch {
      /* already gone */
    }
  }
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    stopDaemon(dir);
    rmSync(dir, { recursive: true, force: true });
  }
});

describe.skipIf(!HAS_BINARY)('VS Code capture journey (live daemon)', () => {
  it('captures a save, finds it by search, reports status and closes the session', async () => {
    const { client, projectRoot } = freshProfile();
    const manager = new EditorSessionManager(client);
    const sessionId = 'vscode-journey';

    // Activation: the extension opens a session capsule scoped to the workspace.
    const context = await manager.onSessionStart({ sessionId, repoId: projectRoot });
    expect(context.activeCapsuleId).toBeTruthy();

    // A save lands a file_diff observation. The payload matches what the
    // extension's onDidSaveTextDocument handler sends: path, repo, timestamp.
    const saved = await manager.onFileSave({
      sessionId,
      filePath: join(projectRoot, 'src', 'markdownHeadingTokenizer.ts'),
      repoId: projectRoot,
      timestamp: Date.now(),
    });
    expect(saved.error).toBeUndefined();
    expect(saved.observation?.kind).toBe('file_diff');

    // kindling.search: retrieve scoped to the workspace finds the save.
    const result = await client.retrieve({
      query: 'markdownHeadingTokenizer',
      scopeIds: { sessionId, repoId: projectRoot },
      maxCandidates: 10,
    });
    const ids = result.candidates.map((c) => ('id' in c.entity ? c.entity.id : undefined));
    expect(ids).toContain(saved.observation?.id);

    // kindling.status: the daemon reports its version and schema.
    const health = await client.health();
    expect(health.version).toBeTruthy();
    expect(health.schemaVersion).toBeTruthy();

    // Deactivation: the session capsule closes.
    const closed = await manager.onSessionEnd(sessionId, { reason: 'window closed' });
    expect(closed.status).toBe('closed');
    expect(await client.getOpenCapsule(sessionId)).toBeNull();
  });
});
