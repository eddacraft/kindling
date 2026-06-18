/**
 * PocketFlow Adapter — pure helper tests.
 *
 * Intent inference and confidence tracking are pure functions with no daemon
 * dependency, so they live here and run in every test pass. The daemon-backed
 * {@link KindlingNode}/{@link KindlingFlow} lifecycle is covered by
 * `test/lifecycle.spec.ts` against a real `kindling` binary.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  inferIntent,
  ConfidenceTracker,
  calculateConfidence,
  DEFAULT_INTENT_PATTERNS,
} from '../index.js';

describe('inferIntent', () => {
  it('should infer test intent from test-related names', () => {
    expect(inferIntent('run-tests')).toBe('test');
    expect(inferIntent('runTests')).toBe('test');
    expect(inferIntent('test_authentication')).toBe('test');
    expect(inferIntent('specRunner')).toBe('test');
    expect(inferIntent('verify-output')).toBe('test');
    expect(inferIntent('validateInput')).toBe('test');
  });

  it('should infer build intent from build-related names', () => {
    expect(inferIntent('build-app')).toBe('build');
    expect(inferIntent('buildApp')).toBe('build');
    expect(inferIntent('compile_typescript')).toBe('build');
    expect(inferIntent('bundleAssets')).toBe('build');
  });

  it('should infer deploy intent from deploy-related names', () => {
    expect(inferIntent('deploy-production')).toBe('deploy');
    expect(inferIntent('deployProduction')).toBe('deploy');
    expect(inferIntent('releaseVersion')).toBe('deploy');
    expect(inferIntent('ship-to-prod')).toBe('deploy');
  });

  it('should infer debug intent from fix/debug-related names', () => {
    expect(inferIntent('fix-auth-bug')).toBe('debug');
    expect(inferIntent('fixAuthBug')).toBe('debug');
    expect(inferIntent('debug_issue')).toBe('debug');
    expect(inferIntent('troubleshootConnection')).toBe('debug');
  });

  it('should infer feature intent from implementation-related names', () => {
    expect(inferIntent('implement-login')).toBe('feature');
    expect(inferIntent('implementLogin')).toBe('feature');
    expect(inferIntent('add_feature')).toBe('feature');
    expect(inferIntent('createUserProfile')).toBe('feature');
  });

  it('should infer refactor intent from refactor-related names', () => {
    expect(inferIntent('refactor-auth')).toBe('refactor');
    expect(inferIntent('refactorAuth')).toBe('refactor');
    expect(inferIntent('cleanup_code')).toBe('refactor');
    expect(inferIntent('restructureProject')).toBe('refactor');
  });

  it('should return general for unknown patterns', () => {
    expect(inferIntent('unknownNode')).toBe('general');
    expect(inferIntent('myCustomThing')).toBe('general');
    expect(inferIntent('xyz')).toBe('general');
  });

  it('should handle empty and whitespace input', () => {
    expect(inferIntent('')).toBe('general');
    expect(inferIntent('   ')).toBe('general');
  });

  it('should support custom patterns', () => {
    const customPatterns = [{ keywords: ['magic'], intent: 'magical' }];
    expect(inferIntent('doMagicStuff', customPatterns)).toBe('magical');
    expect(inferIntent('doOtherStuff', customPatterns)).toBe('general');
  });

  it('should export DEFAULT_INTENT_PATTERNS', () => {
    expect(DEFAULT_INTENT_PATTERNS).toBeDefined();
    expect(Array.isArray(DEFAULT_INTENT_PATTERNS)).toBe(true);
    expect(DEFAULT_INTENT_PATTERNS.length).toBeGreaterThan(0);
  });
});

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
      history: [
        { success: true, timestamp: Date.now() },
        { success: true, timestamp: Date.now() - 1000 },
        { success: true, timestamp: Date.now() - 2000 },
      ],
    };
    const confidence = calculateConfidence(state);
    expect(confidence).toBeGreaterThan(0.5);
    expect(confidence).toBeLessThanOrEqual(0.95);
  });

  it('should decrease confidence with failures', () => {
    const state = {
      successCount: 0,
      failureCount: 5,
      consecutiveFailures: 3,
      history: [
        { success: false, timestamp: Date.now() },
        { success: false, timestamp: Date.now() - 1000 },
        { success: false, timestamp: Date.now() - 2000 },
      ],
    };
    const confidence = calculateConfidence(state);
    expect(confidence).toBeLessThan(0.5);
    expect(confidence).toBeGreaterThanOrEqual(0.1);
  });

  it('should apply consecutive failure penalty', () => {
    const stateWithoutConsecutive = {
      successCount: 3,
      failureCount: 3,
      consecutiveFailures: 0,
      history: [],
    };
    const stateWithConsecutive = {
      successCount: 3,
      failureCount: 3,
      consecutiveFailures: 3,
      history: [],
    };

    const withoutPenalty = calculateConfidence(stateWithoutConsecutive);
    const withPenalty = calculateConfidence(stateWithConsecutive);

    expect(withPenalty).toBeLessThan(withoutPenalty);
  });
});

describe('ConfidenceTracker', () => {
  let tracker: ConfidenceTracker;

  beforeEach(() => {
    tracker = new ConfidenceTracker();
  });

  it('should return base confidence for unknown nodes', () => {
    expect(tracker.getConfidence('unknown-node')).toBe(0.5);
  });

  it('should increase confidence on success', () => {
    const initial = tracker.getConfidence('test-node');
    tracker.recordSuccess('test-node');
    const after = tracker.getConfidence('test-node');
    expect(after).toBeGreaterThan(initial);
  });

  it('should decrease confidence on failure', () => {
    tracker.recordSuccess('test-node');
    tracker.recordSuccess('test-node');
    const afterSuccess = tracker.getConfidence('test-node');

    tracker.recordFailure('test-node', 'Test error');
    const afterFailure = tracker.getConfidence('test-node');

    expect(afterFailure).toBeLessThan(afterSuccess);
  });

  it('should track consecutive failures', () => {
    tracker.recordFailure('test-node');
    let state = tracker.getState('test-node');
    expect(state?.consecutiveFailures).toBe(1);

    tracker.recordFailure('test-node');
    state = tracker.getState('test-node');
    expect(state?.consecutiveFailures).toBe(2);

    tracker.recordSuccess('test-node');
    state = tracker.getState('test-node');
    expect(state?.consecutiveFailures).toBe(0);
  });

  it('should maintain execution history', () => {
    tracker.recordSuccess('test-node');
    tracker.recordFailure('test-node', 'Error 1');
    tracker.recordSuccess('test-node');

    const state = tracker.getState('test-node');
    expect(state?.history).toHaveLength(3);
    expect(state?.history[0].success).toBe(true);
    expect(state?.history[1].success).toBe(false);
    expect(state?.history[2].success).toBe(true);
  });

  it('should trim history to configured size', () => {
    const smallTracker = new ConfidenceTracker({ historySize: 3 });

    for (let i = 0; i < 5; i++) {
      smallTracker.recordSuccess('test-node');
    }

    const state = smallTracker.getState('test-node');
    expect(state?.history).toHaveLength(3);
  });

  it('should provide provenance metadata', () => {
    tracker.recordSuccess('test-node');
    tracker.recordFailure('test-node');

    const metadata = tracker.getProvenanceMetadata('test-node');
    expect(metadata.confidence).toBeDefined();
    expect(metadata.successCount).toBe(1);
    expect(metadata.failureCount).toBe(1);
    expect(metadata.consecutiveFailures).toBe(1);
    expect(metadata.historyLength).toBe(2);
  });

  it('should return new node metadata for unknown nodes', () => {
    const metadata = tracker.getProvenanceMetadata('unknown-node');
    expect(metadata.isNewNode).toBe(true);
    expect(metadata.confidence).toBe(0.5);
  });

  it('should reset individual nodes', () => {
    tracker.recordSuccess('test-node');
    expect(tracker.getState('test-node')).toBeDefined();

    tracker.reset('test-node');
    expect(tracker.getState('test-node')).toBeUndefined();
  });

  it('should clear all tracking data', () => {
    tracker.recordSuccess('node-1');
    tracker.recordSuccess('node-2');

    tracker.clear();

    expect(tracker.getState('node-1')).toBeUndefined();
    expect(tracker.getState('node-2')).toBeUndefined();
  });

  it('should accept custom configuration', () => {
    const customTracker = new ConfidenceTracker({
      baseConfidence: 0.7,
      minConfidence: 0.2,
      maxConfidence: 0.9,
    });

    expect(customTracker.getConfidence('new-node')).toBe(0.7);
  });
});
