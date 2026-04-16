/**
 * Hook handlers tests for Claude Code adapter
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createHookHandlers } from '../claude-code/hooks.js';
import type {
  SessionStartContext,
  PostToolUseContext,
  StopContext,
  UserPromptSubmitContext,
  SubagentStopContext,
} from '../claude-code/events.js';

// Mock store implementation
function createMockStore() {
  const observations = new Map();
  const capsules = new Map();
  const capsuleObservations = new Map<string, string[]>();
  const summaries = new Map();

  return {
    observations,
    capsules,
    summaries,

    createCapsule: vi.fn((capsule) => {
      capsules.set(capsule.id, capsule);
      capsuleObservations.set(capsule.id, []);
    }),

    getCapsuleById: vi.fn((id) => capsules.get(id)),

    closeCapsule: vi.fn((id, closedAt) => {
      const capsule = capsules.get(id);
      if (capsule) {
        capsule.status = 'closed';
        capsule.closedAt = closedAt;
      }
    }),

    getOpenCapsuleForSession: vi.fn((_sessionId): undefined => undefined),

    insertObservation: vi.fn((observation) => {
      observations.set(observation.id, observation);
    }),

    attachObservationToCapsule: vi.fn((capsuleId, observationId) => {
      const obs = capsuleObservations.get(capsuleId) || [];
      obs.push(observationId);
      capsuleObservations.set(capsuleId, obs);
    }),

    insertSummary: vi.fn((summary) => {
      summaries.set(summary.id, summary);
    }),
  };
}

describe('createHookHandlers', () => {
  let store: ReturnType<typeof createMockStore>;
  let handlers: ReturnType<typeof createHookHandlers>;

  beforeEach(() => {
    store = createMockStore();
    handlers = createHookHandlers(store);
  });

  describe('onSessionStart', () => {
    it('should create capsule and return continue signal', () => {
      const ctx: SessionStartContext = {
        sessionId: 'session-1',
        cwd: '/project',
      };

      const result = handlers.onSessionStart(ctx);

      expect(result.continue).toBe(true);
      expect(store.createCapsule).toHaveBeenCalledTimes(1);
    });

    it('should use default intent', () => {
      const ctx: SessionStartContext = {
        sessionId: 'session-1',
        cwd: '/project',
      };

      handlers.onSessionStart(ctx);

      expect(store.createCapsule).toHaveBeenCalledWith(
        expect.objectContaining({ intent: 'Claude Code session' }),
      );
    });

    it('should use custom default intent from config', () => {
      const customHandlers = createHookHandlers(store, {
        defaultIntent: 'Custom intent',
      });

      const ctx: SessionStartContext = {
        sessionId: 'session-1',
        cwd: '/project',
      };

      customHandlers.onSessionStart(ctx);

      expect(store.createCapsule).toHaveBeenCalledWith(
        expect.objectContaining({ intent: 'Custom intent' }),
      );
    });
  });

  describe('onPostToolUse', () => {
    beforeEach(() => {
      handlers.onSessionStart({
        sessionId: 'session-1',
        cwd: '/project',
      });
    });

    it('should capture tool use and return continue signal', () => {
      const ctx: PostToolUseContext = {
        sessionId: 'session-1',
        cwd: '/project',
        toolName: 'Read',
        toolInput: { file_path: '/src/index.ts' },
        toolResult: 'file contents',
      };

      const result = handlers.onPostToolUse(ctx);

      expect(result.continue).toBe(true);
      expect(store.insertObservation).toHaveBeenCalledTimes(1);
    });

    it('should skip capture when disabled', () => {
      const customHandlers = createHookHandlers(store, {
        captureResults: false,
      });

      customHandlers.onSessionStart({
        sessionId: 'session-1',
        cwd: '/project',
      });

      const ctx: PostToolUseContext = {
        sessionId: 'session-1',
        cwd: '/project',
        toolName: 'Read',
        toolInput: { file_path: '/src/index.ts' },
      };

      customHandlers.onPostToolUse(ctx);

      expect(store.insertObservation).not.toHaveBeenCalled();
    });
  });

  describe('onStop', () => {
    beforeEach(() => {
      handlers.onSessionStart({
        sessionId: 'session-1',
        cwd: '/project',
      });
    });

    it('should close capsule and return continue signal', () => {
      const ctx: StopContext = {
        sessionId: 'session-1',
        cwd: '/project',
        reason: 'complete',
      };

      const result = handlers.onStop(ctx);

      expect(result.continue).toBe(true);
      expect(store.closeCapsule).toHaveBeenCalledTimes(1);
    });

    it('should handle missing session gracefully', () => {
      const ctx: StopContext = {
        sessionId: 'unknown-session',
        cwd: '/project',
      };

      // Should not throw
      const result = handlers.onStop(ctx);
      expect(result.continue).toBe(true);
    });

    it('should create summary when provided', () => {
      const ctx: StopContext = {
        sessionId: 'session-1',
        cwd: '/project',
        summary: 'Session completed successfully',
      };

      handlers.onStop(ctx);

      expect(store.insertSummary).toHaveBeenCalledWith(
        expect.objectContaining({ content: 'Session completed successfully' }),
      );
    });
  });

  describe('onUserPromptSubmit', () => {
    beforeEach(() => {
      handlers.onSessionStart({
        sessionId: 'session-1',
        cwd: '/project',
      });
    });

    it('should capture user message', () => {
      const ctx: UserPromptSubmitContext = {
        sessionId: 'session-1',
        cwd: '/project',
        content: 'Please fix the bug',
      };

      const result = handlers.onUserPromptSubmit(ctx);

      expect(result.continue).toBe(true);
      expect(store.insertObservation).toHaveBeenCalledTimes(1);
    });

    it('should skip when disabled', () => {
      const customHandlers = createHookHandlers(store, {
        captureUserMessages: false,
      });

      customHandlers.onSessionStart({
        sessionId: 'session-1',
        cwd: '/project',
      });

      const ctx: UserPromptSubmitContext = {
        sessionId: 'session-1',
        cwd: '/project',
        content: 'Please fix the bug',
      };

      customHandlers.onUserPromptSubmit(ctx);

      expect(store.insertObservation).not.toHaveBeenCalled();
    });
  });

  describe('onSubagentStop', () => {
    beforeEach(() => {
      handlers.onSessionStart({
        sessionId: 'session-1',
        cwd: '/project',
      });
    });

    it('should capture subagent completion', () => {
      const ctx: SubagentStopContext = {
        sessionId: 'session-1',
        cwd: '/project',
        agentType: 'Explore',
        output: 'Found 3 files',
      };

      const result = handlers.onSubagentStop(ctx);

      expect(result.continue).toBe(true);
      expect(store.insertObservation).toHaveBeenCalledTimes(1);
    });

    it('should skip when disabled', () => {
      const customHandlers = createHookHandlers(store, {
        captureSubagents: false,
      });

      customHandlers.onSessionStart({
        sessionId: 'session-1',
        cwd: '/project',
      });

      const ctx: SubagentStopContext = {
        sessionId: 'session-1',
        cwd: '/project',
        agentType: 'Explore',
        output: 'Found 3 files',
      };

      customHandlers.onSubagentStop(ctx);

      expect(store.insertObservation).not.toHaveBeenCalled();
    });
  });

  describe('utility methods', () => {
    it('should expose session manager', () => {
      const manager = handlers.getSessionManager();
      expect(manager).toBeDefined();
    });

    it('should check session active status', () => {
      expect(handlers.isSessionActive('session-1')).toBe(false);

      handlers.onSessionStart({
        sessionId: 'session-1',
        cwd: '/project',
      });

      expect(handlers.isSessionActive('session-1')).toBe(true);
    });

    it('should return session stats', () => {
      handlers.onSessionStart({
        sessionId: 'session-1',
        cwd: '/project',
      });

      handlers.onPostToolUse({
        sessionId: 'session-1',
        cwd: '/project',
        toolName: 'Read',
        toolInput: { file_path: '/test' },
      });

      const stats = handlers.getSessionStats('session-1');
      expect(stats?.eventCount).toBe(1);
    });
  });
});
