/**
 * Unit tests for EditorSessionManager with a mocked Kindling client.
 *
 * These tests do not require a running daemon. Live-daemon integration can be
 * added later following the opencode adapter pattern.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

import type { Capsule, Kindling, Observation } from '@eddacraft/kindling';

import { EditorSessionManager } from '../src/session.js';

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

describe('EditorSessionManager (mocked Kindling)', () => {
  let kindling: Kindling;
  let manager: EditorSessionManager;

  beforeEach(() => {
    kindling = createMockKindling();
    manager = new EditorSessionManager(kindling);
  });

  describe('onSessionStart', () => {
    it('opens a session capsule on first start', async () => {
      const context = await manager.onSessionStart({
        sessionId: 's1',
        intent: 'Fix bug',
        repoId: '/repo',
      });

      expect(context.sessionId).toBe('s1');
      expect(context.repoId).toBe('/repo');
      expect(context.activeCapsuleId).toBeTruthy();
      expect(context.observationCount).toBe(0);
      expect(kindling.openCapsule).toHaveBeenCalledOnce();
    });

    it('returns existing context if session already active', async () => {
      const first = await manager.onSessionStart({ sessionId: 's1' });
      const second = await manager.onSessionStart({ sessionId: 's1' });
      expect(first.activeCapsuleId).toBe(second.activeCapsuleId);
      expect(kindling.openCapsule).toHaveBeenCalledOnce();
    });

    it('recovers an existing open capsule from the daemon', async () => {
      vi.mocked(kindling.getOpenCapsule).mockResolvedValueOnce(
        makeCapsule({ id: 'cap-existing', observationIds: ['obs-0'] }),
      );

      const context = await manager.onSessionStart({ sessionId: 's1' });

      expect(context.activeCapsuleId).toBe('cap-existing');
      expect(context.observationCount).toBe(1);
      expect(kindling.openCapsule).not.toHaveBeenCalled();
    });
  });

  describe('onFileSave', () => {
    it('appends a file_diff observation', async () => {
      await manager.onSessionStart({ sessionId: 's1', repoId: '/repo' });

      const result = await manager.onFileSave({
        sessionId: 's1',
        filePath: '/repo/src/index.ts',
        repoId: '/repo',
        timestamp: 1_700_000_000_000,
      });

      expect(result.observation).toBeDefined();
      expect(result.observation!.kind).toBe('file_diff');
      expect(result.observation!.content).toContain('/repo/src/index.ts');
      expect(kindling.appendObservation).toHaveBeenCalledOnce();
    });

    it('returns an error when no active session exists', async () => {
      const result = await manager.onFileSave({
        sessionId: 'unknown',
        filePath: '/repo/src/index.ts',
      });

      expect(result.error).toContain('No active session found');
    });
  });

  describe('onTerminalCommand', () => {
    it('appends a command observation', async () => {
      await manager.onSessionStart({ sessionId: 's1' });

      const result = await manager.onTerminalCommand({
        sessionId: 's1',
        command: 'pnpm test',
        exitCode: 0,
        stdout: 'ok',
      });

      expect(result.observation).toBeDefined();
      expect(result.observation!.kind).toBe('command');
      expect(result.observation!.content).toContain('pnpm test');
    });
  });

  describe('onSessionEnd', () => {
    it('closes the active capsule and removes the session', async () => {
      const context = await manager.onSessionStart({ sessionId: 's1' });

      const closed = await manager.onSessionEnd('s1', {
        summaryContent: 'Done',
        summaryConfidence: 0.9,
      });

      expect(closed.id).toBe(context.activeCapsuleId);
      expect(closed.status).toBe('closed');
      expect(manager.isSessionActive('s1')).toBe(false);
      expect(kindling.closeCapsule).toHaveBeenCalledOnce();
    });

    it('throws for an unknown session', async () => {
      await expect(manager.onSessionEnd('unknown')).rejects.toThrow('No active session found');
    });
  });
});
