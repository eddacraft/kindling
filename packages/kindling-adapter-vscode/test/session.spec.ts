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

    it('shares an in-flight start so concurrent callers reuse one capsule', async () => {
      let releaseLookup: (value: Capsule | null) => void = () => {};
      vi.mocked(kindling.getOpenCapsule).mockImplementation(
        () =>
          new Promise((resolve) => {
            releaseLookup = resolve;
          }),
      );

      const first = manager.onSessionStart({ sessionId: 's1', intent: 'Fix bug' });
      const second = manager.onSessionStart({ sessionId: 's1', intent: 'Other' });

      expect(kindling.getOpenCapsule).toHaveBeenCalledTimes(1);

      releaseLookup(null);

      const [a, b] = await Promise.all([first, second]);
      expect(a).toBe(b);
      expect(a.activeCapsuleId).toBe(b.activeCapsuleId);
      expect(kindling.getOpenCapsule).toHaveBeenCalledTimes(1);
      expect(kindling.openCapsule).toHaveBeenCalledOnce();
      expect(kindling.openCapsule).toHaveBeenCalledWith(
        expect.objectContaining({ intent: 'Fix bug' }),
      );
    });

    it('lets concurrent starts for different sessions proceed independently', async () => {
      const lookups: Array<(value: Capsule | null) => void> = [];
      vi.mocked(kindling.getOpenCapsule).mockImplementation(
        () =>
          new Promise((resolve) => {
            lookups.push(resolve);
          }),
      );

      const first = manager.onSessionStart({ sessionId: 's1' });
      const second = manager.onSessionStart({ sessionId: 's2' });

      expect(kindling.getOpenCapsule).toHaveBeenCalledTimes(2);

      for (const release of lookups) {
        release(null);
      }

      const [a, b] = await Promise.all([first, second]);
      expect(a.sessionId).toBe('s1');
      expect(b.sessionId).toBe('s2');
      expect(a.activeCapsuleId).not.toBe(b.activeCapsuleId);
      expect(kindling.openCapsule).toHaveBeenCalledTimes(2);
    });

    it('clears a failed in-flight start so a later call can retry', async () => {
      vi.mocked(kindling.getOpenCapsule).mockRejectedValueOnce(new Error('daemon down'));

      const first = manager.onSessionStart({ sessionId: 's1' });
      const second = manager.onSessionStart({ sessionId: 's1' });

      await expect(first).rejects.toThrow('daemon down');
      await expect(second).rejects.toThrow('daemon down');
      expect(kindling.getOpenCapsule).toHaveBeenCalledTimes(1);

      const retried = await manager.onSessionStart({ sessionId: 's1' });
      expect(retried.activeCapsuleId).toBeTruthy();
      expect(kindling.openCapsule).toHaveBeenCalledOnce();
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
      expect(kindling.openCapsule).not.toHaveBeenCalled();
      expect(kindling.appendObservation).not.toHaveBeenCalled();
    });

    it('starts a session on save when ensureSession is set and none is active', async () => {
      const result = await manager.onFileSave({
        sessionId: 's1',
        filePath: '/repo/src/index.ts',
        repoId: '/repo',
        ensureSession: true,
      });

      expect(result.error).toBeUndefined();
      expect(result.observation).toBeDefined();
      expect(manager.isSessionActive('s1')).toBe(true);
      expect(kindling.openCapsule).toHaveBeenCalledOnce();
      expect(kindling.appendObservation).toHaveBeenCalledOnce();
    });

    it('recovers and records a save after an initial session start failure', async () => {
      vi.mocked(kindling.getOpenCapsule).mockRejectedValueOnce(new Error('daemon unavailable'));

      await expect(manager.onSessionStart({ sessionId: 's1', repoId: '/repo' })).rejects.toThrow(
        'daemon unavailable',
      );
      expect(manager.isSessionActive('s1')).toBe(false);

      const result = await manager.onFileSave({
        sessionId: 's1',
        filePath: '/repo/src/index.ts',
        repoId: '/repo',
        ensureSession: true,
      });

      expect(result.error).toBeUndefined();
      expect(result.observation?.kind).toBe('file_diff');
      expect(manager.isSessionActive('s1')).toBe(true);
      expect(kindling.openCapsule).toHaveBeenCalledOnce();
      expect(kindling.appendObservation).toHaveBeenCalledOnce();
    });

    it('returns an error when ensureSession is set but start still fails', async () => {
      vi.mocked(kindling.getOpenCapsule).mockRejectedValue(new Error('daemon unavailable'));

      const result = await manager.onFileSave({
        sessionId: 's1',
        filePath: '/repo/src/index.ts',
        ensureSession: true,
      });

      expect(result.observation).toBeUndefined();
      expect(result.error).toContain('No active session found');
      expect(result.error).toContain('daemon unavailable');
      expect(manager.isSessionActive('s1')).toBe(false);
      expect(kindling.openCapsule).not.toHaveBeenCalled();
      expect(kindling.appendObservation).not.toHaveBeenCalled();
    });

    it('does not start a second session on a later save after recovery', async () => {
      vi.mocked(kindling.getOpenCapsule).mockRejectedValueOnce(new Error('daemon unavailable'));
      await expect(manager.onSessionStart({ sessionId: 's1' })).rejects.toThrow(
        'daemon unavailable',
      );

      await manager.onFileSave({
        sessionId: 's1',
        filePath: '/repo/src/a.ts',
        ensureSession: true,
      });
      await manager.onFileSave({
        sessionId: 's1',
        filePath: '/repo/src/b.ts',
        ensureSession: true,
      });

      expect(kindling.openCapsule).toHaveBeenCalledOnce();
      expect(kindling.appendObservation).toHaveBeenCalledTimes(2);
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
