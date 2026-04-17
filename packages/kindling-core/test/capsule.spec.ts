/**
 * Tests for capsule lifecycle management
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  CapsuleManager,
  openCapsule,
  closeCapsule,
  getCapsule,
  getOpenCapsule,
} from '../src/capsule/index.js';
import type { CapsuleStore } from '../src/capsule/lifecycle.js';
import type { Capsule } from '../src/types/capsule.js';
import type { Summary } from '../src/types/summary.js';
import type { ID } from '../src/types/common.js';

/**
 * Mock store implementation for testing
 */
class MockCapsuleStore implements CapsuleStore {
  private capsules: Map<ID, Capsule> = new Map();
  private summaries: Map<ID, Summary> = new Map();

  createCapsule(capsule: Capsule): void {
    this.capsules.set(capsule.id, capsule);
  }

  closeCapsule(capsuleId: ID, closedAt: number): void {
    const capsule = this.capsules.get(capsuleId);
    if (capsule) {
      this.capsules.set(capsuleId, {
        ...capsule,
        status: 'closed',
        closedAt,
      });
    }
  }

  getCapsuleById(capsuleId: ID): Capsule | undefined {
    return this.capsules.get(capsuleId);
  }

  getOpenCapsuleForSession(sessionId: string): Capsule | undefined {
    for (const capsule of this.capsules.values()) {
      if (capsule.status === 'open' && capsule.scopeIds.sessionId === sessionId) {
        return capsule;
      }
    }
    return undefined;
  }

  insertSummary(summary: Summary): void {
    this.summaries.set(summary.id, summary);
  }

  // Helper methods for testing
  getSummary(id: ID): Summary | undefined {
    return this.summaries.get(id);
  }

  getAllCapsules(): Capsule[] {
    return Array.from(this.capsules.values());
  }

  clear(): void {
    this.capsules.clear();
    this.summaries.clear();
  }
}

describe('Capsule Lifecycle', () => {
  let store: MockCapsuleStore;

  beforeEach(() => {
    store = new MockCapsuleStore();
  });

  describe('openCapsule', () => {
    it('should create a new open capsule', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test session',
        scopeIds: { sessionId: 's1' },
      });

      expect(capsule.id).toBeDefined();
      expect(capsule.type).toBe('session');
      expect(capsule.intent).toBe('Test session');
      expect(capsule.status).toBe('open');
      expect(capsule.scopeIds.sessionId).toBe('s1');
      expect(capsule.openedAt).toBeDefined();
      expect(capsule.closedAt).toBeUndefined();
    });

    it('should persist capsule to store', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Fix bug',
        scopeIds: { sessionId: 's1' },
      });

      const retrieved = store.getCapsuleById(capsule.id);
      expect(retrieved).toEqual(capsule);
    });

    it('should accept pre-generated ID', () => {
      const customId = 'custom-capsule-id';

      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
        id: customId,
      });

      expect(capsule.id).toBe(customId);
    });

    it('should throw error if session already has open capsule', () => {
      openCapsule(store, {
        type: 'session',
        intent: 'First capsule',
        scopeIds: { sessionId: 's1' },
      });

      expect(() => {
        openCapsule(store, {
          type: 'session',
          intent: 'Second capsule',
          scopeIds: { sessionId: 's1' },
        });
      }).toThrow('already has an open capsule');
    });

    it('should allow multiple open capsules for different sessions', () => {
      const capsule1 = openCapsule(store, {
        type: 'session',
        intent: 'Session 1',
        scopeIds: { sessionId: 's1' },
      });

      const capsule2 = openCapsule(store, {
        type: 'session',
        intent: 'Session 2',
        scopeIds: { sessionId: 's2' },
      });

      expect(capsule1.id).not.toBe(capsule2.id);
      expect(store.getAllCapsules()).toHaveLength(2);
    });

    it('should validate capsule input', () => {
      expect(() => {
        openCapsule(store, {
          type: 'invalid' as any,
          intent: 'Test',
          scopeIds: {},
        });
      }).toThrow('validation failed');
    });
  });

  describe('closeCapsule', () => {
    it('should close an open capsule', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      const closed = closeCapsule(store, capsule.id);

      expect(closed.status).toBe('closed');
      expect(closed.closedAt).toBeDefined();
      expect(closed.closedAt!).toBeGreaterThanOrEqual(capsule.openedAt);
    });

    it('should persist closed status to store', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      closeCapsule(store, capsule.id);

      const retrieved = store.getCapsuleById(capsule.id);
      expect(retrieved?.status).toBe('closed');
      expect(retrieved?.closedAt).toBeDefined();
    });

    it('should throw error if capsule not found', () => {
      expect(() => {
        closeCapsule(store, 'non-existent-id');
      }).toThrow('not found');
    });

    it('should throw error if capsule already closed', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      closeCapsule(store, capsule.id);

      expect(() => {
        closeCapsule(store, capsule.id);
      }).toThrow('already closed');
    });

    it('should create summary if content provided', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      closeCapsule(store, capsule.id, {
        summaryContent: 'Completed testing feature X',
        summaryConfidence: 0.9,
        evidenceRefs: [],
      });

      const summaries = Array.from((store as any).summaries.values());
      expect(summaries).toHaveLength(1);
      expect(summaries[0].content).toBe('Completed testing feature X');
      expect(summaries[0].confidence).toBe(0.9);
      expect(summaries[0].capsuleId).toBe(capsule.id);
    });

    it('should accept reason signal', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      const closed = closeCapsule(store, capsule.id, {
        reason: 'timeout',
      });

      expect(closed.status).toBe('closed');
    });
  });

  describe('getCapsule', () => {
    it('should retrieve capsule by ID', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      const retrieved = getCapsule(store, capsule.id);
      expect(retrieved).toEqual(capsule);
    });

    it('should return undefined for non-existent ID', () => {
      const retrieved = getCapsule(store, 'non-existent-id');
      expect(retrieved).toBeUndefined();
    });
  });

  describe('getOpenCapsule', () => {
    it('should return open capsule for session', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      const retrieved = getOpenCapsule(store, 's1');
      expect(retrieved).toEqual(capsule);
    });

    it('should return undefined if no open capsule for session', () => {
      const retrieved = getOpenCapsule(store, 's1');
      expect(retrieved).toBeUndefined();
    });

    it('should return undefined if capsule is closed', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      closeCapsule(store, capsule.id);

      const retrieved = getOpenCapsule(store, 's1');
      expect(retrieved).toBeUndefined();
    });
  });
});

describe('CapsuleManager', () => {
  let store: MockCapsuleStore;
  let manager: CapsuleManager;

  beforeEach(() => {
    store = new MockCapsuleStore();
    manager = new CapsuleManager(store);
  });

  describe('open', () => {
    it('should open a new capsule', () => {
      const capsule = manager.open({
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      expect(capsule.status).toBe('open');
      expect(capsule.id).toBeDefined();
    });

    it('should cache opened capsule', () => {
      const capsule = manager.open({
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      expect(manager.getCacheSize()).toBe(1);

      // Should retrieve from cache
      const retrieved = manager.get(capsule.id);
      expect(retrieved).toBe(capsule); // Same reference
    });
  });

  describe('close', () => {
    it('should close an open capsule', () => {
      const capsule = manager.open({
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      const closed = manager.close(capsule.id);
      expect(closed.status).toBe('closed');
    });

    it('should remove from cache when closed', () => {
      const capsule = manager.open({
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      expect(manager.getCacheSize()).toBe(1);

      manager.close(capsule.id);

      expect(manager.getCacheSize()).toBe(0);
    });

    it('should create summary on close', () => {
      const capsule = manager.open({
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      manager.close(capsule.id, {
        summaryContent: 'Task completed successfully',
        summaryConfidence: 0.95,
      });

      const summaries = Array.from((store as any).summaries.values());
      expect(summaries).toHaveLength(1);
      expect(summaries[0].content).toBe('Task completed successfully');
    });
  });

  describe('get', () => {
    it('should retrieve capsule from cache', () => {
      const capsule = manager.open({
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      const retrieved = manager.get(capsule.id);
      expect(retrieved).toBe(capsule);
    });

    it('should fall back to store if not in cache', () => {
      const capsule = openCapsule(store, {
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      const retrieved = manager.get(capsule.id);
      expect(retrieved?.id).toBe(capsule.id);
    });
  });

  describe('getOpen', () => {
    it('should return open capsule for session', () => {
      const capsule = manager.open({
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      const retrieved = manager.getOpen({ sessionId: 's1' });
      expect(retrieved?.id).toBe(capsule.id);
    });

    it('should throw error if sessionId not provided', () => {
      expect(() => {
        manager.getOpen({});
      }).toThrow('only supports sessionId lookup');
    });
  });

  describe('cache management', () => {
    it('should clear cache', () => {
      manager.open({
        type: 'session',
        intent: 'Test 1',
        scopeIds: { sessionId: 's1' },
      });

      manager.open({
        type: 'session',
        intent: 'Test 2',
        scopeIds: { sessionId: 's2' },
      });

      expect(manager.getCacheSize()).toBe(2);

      manager.clearCache();

      expect(manager.getCacheSize()).toBe(0);
    });

    it('should report correct cache size', () => {
      expect(manager.getCacheSize()).toBe(0);

      manager.open({
        type: 'session',
        intent: 'Test',
        scopeIds: { sessionId: 's1' },
      });

      expect(manager.getCacheSize()).toBe(1);
    });
  });

  describe('concurrent access', () => {
    it('should handle multiple open capsules for different sessions', () => {
      const capsule1 = manager.open({
        type: 'session',
        intent: 'Session 1',
        scopeIds: { sessionId: 's1' },
      });

      const capsule2 = manager.open({
        type: 'session',
        intent: 'Session 2',
        scopeIds: { sessionId: 's2' },
      });

      expect(capsule1.id).not.toBe(capsule2.id);
      expect(manager.getCacheSize()).toBe(2);
    });

    it('should prevent duplicate open capsules for same session', () => {
      manager.open({
        type: 'session',
        intent: 'First',
        scopeIds: { sessionId: 's1' },
      });

      expect(() => {
        manager.open({
          type: 'session',
          intent: 'Second',
          scopeIds: { sessionId: 's1' },
        });
      }).toThrow('already has an open capsule');
    });

    it('should allow reopening after close', () => {
      const capsule1 = manager.open({
        type: 'session',
        intent: 'First',
        scopeIds: { sessionId: 's1' },
      });

      manager.close(capsule1.id);

      const capsule2 = manager.open({
        type: 'session',
        intent: 'Second',
        scopeIds: { sessionId: 's1' },
      });

      expect(capsule2.id).not.toBe(capsule1.id);
      expect(capsule2.status).toBe('open');
    });
  });
});
