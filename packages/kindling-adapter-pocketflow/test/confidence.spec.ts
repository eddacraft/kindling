/**
 * Tests for confidence tracking
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { ConfidenceTracker, calculateConfidence } from '../src/pocketflow/confidence.js';

describe('calculateConfidence', () => {
  it('should return base confidence for new nodes', () => {
    const state = {
      successCount: 0,
      failureCount: 0,
      consecutiveFailures: 0,
      history: [],
    };
    expect(calculateConfidence(state)).toBe(0.5);
  });

  it('should increase confidence with successes', () => {
    const state = {
      successCount: 5,
      failureCount: 0,
      consecutiveFailures: 0,
      history: Array(5).fill({ success: true, timestamp: Date.now() }),
    };
    expect(calculateConfidence(state)).toBeGreaterThan(0.5);
  });

  it('should decrease confidence with failures', () => {
    const state = {
      successCount: 0,
      failureCount: 5,
      consecutiveFailures: 5,
      history: Array(5).fill({ success: false, timestamp: Date.now() }),
    };
    expect(calculateConfidence(state)).toBeLessThan(0.5);
  });

  it('should apply consecutive failure penalty', () => {
    const stateNoConsecutive = {
      successCount: 5,
      failureCount: 5,
      consecutiveFailures: 0,
      history: [
        { success: true, timestamp: Date.now() },
        { success: false, timestamp: Date.now() },
        { success: true, timestamp: Date.now() },
        { success: false, timestamp: Date.now() },
        { success: true, timestamp: Date.now() },
      ],
    };

    const stateWithConsecutive = {
      successCount: 5,
      failureCount: 5,
      consecutiveFailures: 3,
      history: [
        { success: false, timestamp: Date.now() },
        { success: false, timestamp: Date.now() },
        { success: false, timestamp: Date.now() },
        { success: true, timestamp: Date.now() },
        { success: true, timestamp: Date.now() },
      ],
    };

    expect(calculateConfidence(stateWithConsecutive)).toBeLessThan(
      calculateConfidence(stateNoConsecutive),
    );
  });

  it('should respect min confidence floor', () => {
    const state = {
      successCount: 0,
      failureCount: 100,
      consecutiveFailures: 100,
      history: Array(10).fill({ success: false, timestamp: Date.now() }),
    };
    expect(calculateConfidence(state)).toBeGreaterThanOrEqual(0.1);
  });

  it('should respect max confidence ceiling', () => {
    const state = {
      successCount: 100,
      failureCount: 0,
      consecutiveFailures: 0,
      history: Array(10).fill({ success: true, timestamp: Date.now() }),
    };
    expect(calculateConfidence(state)).toBeLessThanOrEqual(0.95);
  });

  it('should weight recent results more heavily', () => {
    // Recent successes after old failures should improve confidence
    const recentSuccesses = {
      successCount: 5,
      failureCount: 5,
      consecutiveFailures: 0,
      history: [
        { success: true, timestamp: Date.now() },
        { success: true, timestamp: Date.now() },
        { success: true, timestamp: Date.now() },
        { success: false, timestamp: Date.now() - 10000 },
        { success: false, timestamp: Date.now() - 20000 },
      ],
    };

    // Recent failures after old successes should decrease confidence
    const recentFailures = {
      successCount: 5,
      failureCount: 5,
      consecutiveFailures: 3,
      history: [
        { success: false, timestamp: Date.now() },
        { success: false, timestamp: Date.now() },
        { success: false, timestamp: Date.now() },
        { success: true, timestamp: Date.now() - 10000 },
        { success: true, timestamp: Date.now() - 20000 },
      ],
    };

    expect(calculateConfidence(recentSuccesses)).toBeGreaterThan(
      calculateConfidence(recentFailures),
    );
  });
});

describe('ConfidenceTracker', () => {
  let tracker: ConfidenceTracker;

  beforeEach(() => {
    tracker = new ConfidenceTracker();
  });

  describe('recordSuccess', () => {
    it('should create state for new node', () => {
      const state = tracker.recordSuccess('test-node');
      expect(state.nodeId).toBe('test-node');
      expect(state.successCount).toBe(1);
      expect(state.failureCount).toBe(0);
    });

    it('should increment success count', () => {
      tracker.recordSuccess('test-node');
      tracker.recordSuccess('test-node');
      const state = tracker.recordSuccess('test-node');
      expect(state.successCount).toBe(3);
    });

    it('should reset consecutive failures', () => {
      tracker.recordFailure('test-node');
      tracker.recordFailure('test-node');
      const state = tracker.recordSuccess('test-node');
      expect(state.consecutiveFailures).toBe(0);
    });

    it('should add to history', () => {
      const state = tracker.recordSuccess('test-node');
      expect(state.history.length).toBe(1);
      expect(state.history[0].success).toBe(true);
    });

    it('should increase confidence', () => {
      const initial = tracker.getConfidence('test-node');
      tracker.recordSuccess('test-node');
      const after = tracker.getConfidence('test-node');
      expect(after).toBeGreaterThan(initial);
    });
  });

  describe('recordFailure', () => {
    it('should create state for new node', () => {
      const state = tracker.recordFailure('test-node');
      expect(state.nodeId).toBe('test-node');
      expect(state.successCount).toBe(0);
      expect(state.failureCount).toBe(1);
    });

    it('should increment failure count', () => {
      tracker.recordFailure('test-node');
      tracker.recordFailure('test-node');
      const state = tracker.recordFailure('test-node');
      expect(state.failureCount).toBe(3);
    });

    it('should increment consecutive failures', () => {
      tracker.recordFailure('test-node');
      const state = tracker.recordFailure('test-node');
      expect(state.consecutiveFailures).toBe(2);
    });

    it('should store error message', () => {
      const state = tracker.recordFailure('test-node', 'Connection timeout');
      expect(state.history[0].error).toBe('Connection timeout');
    });

    it('should decrease confidence', () => {
      tracker.recordSuccess('test-node'); // Start with a success
      const initial = tracker.getConfidence('test-node');
      tracker.recordFailure('test-node');
      const after = tracker.getConfidence('test-node');
      expect(after).toBeLessThan(initial);
    });
  });

  describe('getConfidence', () => {
    it('should return base confidence for unknown node', () => {
      expect(tracker.getConfidence('unknown-node')).toBe(0.5);
    });

    it('should return current confidence for known node', () => {
      tracker.recordSuccess('test-node');
      const confidence = tracker.getConfidence('test-node');
      expect(confidence).toBeGreaterThan(0.5);
    });
  });

  describe('getState', () => {
    it('should return undefined for unknown node', () => {
      expect(tracker.getState('unknown-node')).toBeUndefined();
    });

    it('should return full state for known node', () => {
      tracker.recordSuccess('test-node');
      const state = tracker.getState('test-node');
      expect(state).toBeDefined();
      expect(state?.nodeId).toBe('test-node');
      expect(state?.confidence).toBeDefined();
      expect(state?.history).toBeInstanceOf(Array);
    });
  });

  describe('getProvenanceMetadata', () => {
    it('should return base data for new node', () => {
      const metadata = tracker.getProvenanceMetadata('unknown-node');
      expect(metadata.confidence).toBe(0.5);
      expect(metadata.isNewNode).toBe(true);
    });

    it('should return tracking data for known node', () => {
      tracker.recordSuccess('test-node');
      tracker.recordSuccess('test-node');
      tracker.recordFailure('test-node');

      const metadata = tracker.getProvenanceMetadata('test-node');
      expect(metadata.confidence).toBeDefined();
      expect(metadata.successCount).toBe(2);
      expect(metadata.failureCount).toBe(1);
      expect(metadata.consecutiveFailures).toBe(1);
      expect(metadata.historyLength).toBe(3);
    });
  });

  describe('history management', () => {
    it('should limit history to configured size', () => {
      const tracker = new ConfidenceTracker({ historySize: 5 });
      for (let i = 0; i < 10; i++) {
        tracker.recordSuccess('test-node');
      }
      const state = tracker.getState('test-node');
      expect(state?.history.length).toBe(5);
    });

    it('should keep most recent entries', () => {
      const tracker = new ConfidenceTracker({ historySize: 3 });
      tracker.recordSuccess('test-node');
      tracker.recordSuccess('test-node');
      tracker.recordFailure('test-node');

      const state = tracker.getState('test-node');
      expect(state?.history[0].success).toBe(false); // Most recent
      expect(state?.history[1].success).toBe(true);
      expect(state?.history[2].success).toBe(true);
    });
  });

  describe('reset and clear', () => {
    it('should reset single node', () => {
      tracker.recordSuccess('node-a');
      tracker.recordSuccess('node-b');
      tracker.reset('node-a');

      expect(tracker.getState('node-a')).toBeUndefined();
      expect(tracker.getState('node-b')).toBeDefined();
    });

    it('should clear all nodes', () => {
      tracker.recordSuccess('node-a');
      tracker.recordSuccess('node-b');
      tracker.clear();

      expect(tracker.getState('node-a')).toBeUndefined();
      expect(tracker.getState('node-b')).toBeUndefined();
    });
  });

  describe('custom configuration', () => {
    it('should use custom base confidence', () => {
      const tracker = new ConfidenceTracker({ baseConfidence: 0.7 });
      expect(tracker.getConfidence('new-node')).toBe(0.7);
    });

    it('should respect custom min/max bounds', () => {
      const tracker = new ConfidenceTracker({
        minConfidence: 0.2,
        maxConfidence: 0.8,
      });

      // Many failures should hit min
      for (let i = 0; i < 20; i++) {
        tracker.recordFailure('failing-node');
      }
      expect(tracker.getConfidence('failing-node')).toBeGreaterThanOrEqual(0.2);

      // Many successes should hit max
      for (let i = 0; i < 20; i++) {
        tracker.recordSuccess('succeeding-node');
      }
      expect(tracker.getConfidence('succeeding-node')).toBeLessThanOrEqual(0.8);
    });
  });
});
