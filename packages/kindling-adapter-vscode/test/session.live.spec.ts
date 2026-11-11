/**
 * VS Code / Cursor / Windsurf capture journey against a REAL kindling daemon
 * (KFIT-002).
 *
 * The extension (`src/extension.ts`) wires editor events into
 * {@link WorkspaceCapture}, which opens a session capsule per workspace folder
 * over the thin {@link Kindling} client: a `file_diff` observation per save,
 * `kindling.search` through `retrieve`, `kindling.status` through `health`, and
 * a capsule close on deactivation. This suite drives that same capture →
 * client → daemon path in a fresh profile (temp kindling home, unique socket,
 * temp project root), so the quickstart's journey is exercised end to end
 * without a VS Code host.
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
import { WorkspaceCapture } from '../src/workspace.js';

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

  it('routes multi-root saves to the containing project and skips files outside', async () => {
    const home = mkdtempSync(join(tmpdir(), 'kindling-vsc-'));
    const repoA = mkdtempSync(join(tmpdir(), 'kindling-vsc-a-'));
    const repoB = mkdtempSync(join(tmpdir(), 'kindling-vsc-b-'));
    const outside = mkdtempSync(join(tmpdir(), 'kindling-vsc-out-'));
    tempDirs.push(home, repoA, repoB, outside);

    const capture = new WorkspaceCapture({
      sessionId: 'vscode-multi-root',
      createKindling: (projectRoot) =>
        new Kindling({
          socketPath: join(home, 'kindling.sock'),
          ...(projectRoot ? { projectRoot } : {}),
          binaryPath: BINARY,
          connectTimeoutMs: 5000,
        }),
    });

    const folders = [repoA, repoB];
    const savedA = await capture.captureSave(
      join(repoA, 'src', 'alphaTokenizer.ts'),
      folders,
      Date.now(),
    );
    const savedB = await capture.captureSave(
      join(repoB, 'src', 'betaTokenizer.ts'),
      folders,
      Date.now(),
    );
    const skipped = await capture.captureSave(join(outside, 'scratch.ts'), folders, Date.now());

    expect(savedA.error).toBeUndefined();
    expect(savedB.error).toBeUndefined();
    expect(savedA.observation?.scopeIds.repoId).toBe(repoA);
    expect(savedB.observation?.scopeIds.repoId).toBe(repoB);
    expect(skipped.skipped).toBe(true);
    expect(skipped.error).toBe('outside workspace');

    const clientA = capture.kindlingFor(repoA);
    const clientB = capture.kindlingFor(repoB);

    const foundA = await clientA.retrieve({
      query: 'alphaTokenizer',
      scopeIds: { repoId: repoA },
      maxCandidates: 10,
    });
    expect(foundA.candidates.map((c) => c.entity.id)).toContain(savedA.observation?.id);

    const leakedB = await clientA.retrieve({
      query: 'betaTokenizer',
      scopeIds: { repoId: repoA },
      maxCandidates: 10,
    });
    expect(leakedB.candidates.map((c) => c.entity.id)).not.toContain(savedB.observation?.id);

    const foundB = await clientB.retrieve({
      query: 'betaTokenizer',
      scopeIds: { repoId: repoB },
      maxCandidates: 10,
    });
    expect(foundB.candidates.map((c) => c.entity.id)).toContain(savedB.observation?.id);

    const leakedA = await clientB.retrieve({
      query: 'alphaTokenizer',
      scopeIds: { repoId: repoB },
      maxCandidates: 10,
    });
    expect(leakedA.candidates.map((c) => c.entity.id)).not.toContain(savedA.observation?.id);

    const outsideHits = await clientA.retrieve({
      query: 'scratch',
      scopeIds: { repoId: repoA },
      maxCandidates: 10,
    });
    expect(outsideHits.candidates.some((c) => c.entity.content.includes('scratch.ts'))).toBe(false);

    await capture.closeAll({ reason: 'window closed' });
    expect(await clientA.getOpenCapsule('vscode-multi-root')).toBeNull();
    expect(await clientB.getOpenCapsule('vscode-multi-root')).toBeNull();
  });
});
