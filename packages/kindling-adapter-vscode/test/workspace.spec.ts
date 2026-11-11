/**
 * Unit tests for per-document workspace routing.
 *
 * These tests do not require a running daemon or a VS Code host.
 */

import { join, resolve } from 'node:path';

import { describe, it, expect, beforeEach, vi } from 'vitest';

import type { Capsule, Kindling, Observation } from '@eddacraft/kindling';

import { WorkspaceCapture, resolveWorkspaceRoot } from '../src/workspace.js';

function makeCapsule(overrides: Partial<Capsule> = {}): Capsule {
  return {
    id: 'cap-1',
    type: 'session',
    intent: 'Editor session',
    status: 'open',
    scopeIds: { sessionId: 's1' },
    observationIds: [],
    createdAt: Date.now(),
    updatedAt: Date.now(),
    ...overrides,
  };
}

function makeObservation(overrides: Partial<Observation> = {}): Observation {
  return {
    id: 'obs-1',
    kind: 'file_diff',
    content: 'Modified files:\n  /tmp/file.ts',
    scopeIds: { sessionId: 's1' },
    ts: Date.now(),
    redacted: false,
    ...overrides,
  };
}

function createMockKindling(): Kindling {
  const openCapsules = new Map<string, Capsule>();
  let capsuleCounter = 0;
  let observationCounter = 0;

  return {
    getOpenCapsule: vi.fn(async (sessionId: string) => openCapsules.get(sessionId) ?? null),
    openCapsule: vi.fn(async (args) => {
      capsuleCounter += 1;
      const capsule = makeCapsule({
        id: `cap-${capsuleCounter}`,
        intent: args.intent,
        scopeIds: args.scopeIds ?? {},
      });
      const sid = args.scopeIds?.sessionId;
      if (sid) {
        openCapsules.set(sid, capsule);
      }
      return capsule;
    }),
    appendObservation: vi.fn(async (input, args) => {
      observationCounter += 1;
      const observation = makeObservation({
        id: `obs-${observationCounter}`,
        kind: input.kind,
        content: input.content,
        scopeIds: input.scopeIds,
        ts: input.ts ?? Date.now(),
      });

      if (args?.capsuleId) {
        for (const capsule of openCapsules.values()) {
          if (capsule.id === args.capsuleId) {
            capsule.observationIds.push(observation.id);
          }
        }
      }

      return observation;
    }),
    closeCapsule: vi.fn(async (id, _args) => {
      for (const [sessionId, capsule] of openCapsules.entries()) {
        if (capsule.id === id) {
          const closed = { ...capsule, status: 'closed' as const };
          openCapsules.delete(sessionId);
          return closed;
        }
      }
      return makeCapsule({ id, status: 'closed' });
    }),
  } as unknown as Kindling;
}

describe('resolveWorkspaceRoot', () => {
  const app = resolve('/workspace/app');
  const lib = resolve('/workspace/lib');
  const nested = join(app, 'packages', 'core');
  const folders = [app, lib, nested];

  it('returns the folder that contains the file', () => {
    expect(resolveWorkspaceRoot(join(lib, 'src', 'index.ts'), folders)).toBe(lib);
  });

  it('picks the longest match when folders are nested', () => {
    expect(resolveWorkspaceRoot(join(nested, 'src', 'util.ts'), folders)).toBe(nested);
    expect(resolveWorkspaceRoot(join(app, 'README.md'), folders)).toBe(app);
  });

  it('does not match a sibling path that only shares a prefix', () => {
    const appV2 = resolve('/workspace/app-v2');
    expect(resolveWorkspaceRoot(join(appV2, 'file.ts'), [app, lib])).toBeUndefined();
  });

  it('returns undefined for a file outside every workspace folder', () => {
    expect(resolveWorkspaceRoot(resolve('/tmp/scratch.ts'), folders)).toBeUndefined();
  });

  it('returns undefined when there are no workspace folders', () => {
    expect(resolveWorkspaceRoot(join(app, 'file.ts'), [])).toBeUndefined();
  });
});

describe('WorkspaceCapture (mocked Kindling)', () => {
  let clients: Map<string, Kindling>;
  let capture: WorkspaceCapture;

  beforeEach(() => {
    clients = new Map();
    capture = new WorkspaceCapture({
      sessionId: 's1',
      createKindling: (projectRoot) => {
        const kindling = createMockKindling();
        clients.set(projectRoot ?? '', kindling);
        return kindling;
      },
    });
  });

  it('sends each save to the Kindling client for the containing workspace', async () => {
    const app = resolve('/workspace/app');
    const lib = resolve('/workspace/lib');
    const folders = [app, lib];

    const savedApp = await capture.captureSave(join(app, 'src', 'a.ts'), folders, 1);
    const savedLib = await capture.captureSave(join(lib, 'src', 'b.ts'), folders, 2);

    expect(savedApp.observation?.scopeIds.repoId).toBe(app);
    expect(savedLib.observation?.scopeIds.repoId).toBe(lib);

    const appClient = clients.get(app);
    const libClient = clients.get(lib);
    expect(appClient).toBeDefined();
    expect(libClient).toBeDefined();
    expect(appClient).not.toBe(libClient);
    expect(appClient!.appendObservation).toHaveBeenCalledOnce();
    expect(libClient!.appendObservation).toHaveBeenCalledOnce();
    expect(appClient!.openCapsule).toHaveBeenCalledOnce();
    expect(libClient!.openCapsule).toHaveBeenCalledOnce();
  });

  it('skips files outside the workspace without touching any client', async () => {
    const app = resolve('/workspace/app');
    const result = await capture.captureSave(resolve('/tmp/outside.ts'), [app]);

    expect(result.skipped).toBe(true);
    expect(result.error).toBe('outside workspace');
    expect(result.observation).toBeUndefined();
    expect(clients.size).toBe(0);
  });

  it('reuses the session already opened for a workspace', async () => {
    const app = resolve('/workspace/app');
    await capture.ensureSession(app);
    await capture.captureSave(join(app, 'a.ts'), [app]);
    await capture.captureSave(join(app, 'b.ts'), [app]);

    expect(clients.get(app)!.openCapsule).toHaveBeenCalledOnce();
    expect(clients.get(app)!.appendObservation).toHaveBeenCalledTimes(2);
  });
});
