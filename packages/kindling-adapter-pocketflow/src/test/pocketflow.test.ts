/**
 * PocketFlow Adapter Tests
 *
 * Tests for KindlingNode, KindlingFlow, intent inference, and confidence tracking.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  KindlingNode,
  KindlingFlow,
  inferIntent,
  ConfidenceTracker,
  calculateConfidence,
  DEFAULT_INTENT_PATTERNS,
  type KindlingNodeContext,
  type PocketFlowStore,
} from '../index.js';

/**
 * Mock store for testing
 */
function createMockStore(): PocketFlowStore & {
  observations: Array<{ id: string; kind: string; content: string }>;
  capsules: Array<{ id: string; status: string }>;
  attachments: Array<{ capsuleId: string; observationId: string }>;
} {
  const observations: Array<{ id: string; kind: string; content: string }> = [];
  const capsules: Array<{ id: string; status: string }> = [];
  const attachments: Array<{ capsuleId: string; observationId: string }> = [];

  return {
    observations,
    capsules,
    attachments,
    insertObservation(observation) {
      observations.push({
        id: observation.id,
        kind: observation.kind,
        content: observation.content,
      });
    },
    createCapsule(capsule) {
      capsules.push({
        id: capsule.id,
        status: capsule.status,
      });
    },
    closeCapsule(capsuleId) {
      const capsule = capsules.find((c) => c.id === capsuleId);
      if (capsule) {
        capsule.status = 'closed';
      }
    },
    attachObservationToCapsule(capsuleId, observationId) {
      attachments.push({ capsuleId, observationId });
    },
  };
}

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
    // First build up some confidence
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
    expect(state?.history[0].success).toBe(true); // Most recent first
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

describe('KindlingNode', () => {
  let mockStore: ReturnType<typeof createMockStore>;
  let context: KindlingNodeContext;

  beforeEach(() => {
    mockStore = createMockStore();
    context = {
      store: mockStore,
      scopeIds: { sessionId: 'test-session', repoId: 'test-repo' },
    };
  });

  it('should create a capsule on prep', async () => {
    class TestNode extends KindlingNode {
      override async exec() {
        return 'test result';
      }
    }

    const node = new TestNode({ name: 'test-node', intent: 'test' });
    await node.run(context);

    expect(mockStore.capsules).toHaveLength(1);
    expect(mockStore.capsules[0].status).toBe('closed');
  });

  it('should record node_start observation', async () => {
    class TestNode extends KindlingNode {
      override async exec() {
        return 'result';
      }
    }

    const node = new TestNode({ name: 'my-test-node' });
    await node.run(context);

    const startObs = mockStore.observations.find((o) => o.kind === 'node_start');
    expect(startObs).toBeDefined();
    expect(startObs?.content).toContain('my-test-node');
  });

  it('should record node_output and node_end observations on success', async () => {
    class TestNode extends KindlingNode {
      override async exec() {
        return { data: 'test output' };
      }
    }

    const node = new TestNode({ name: 'output-test' });
    await node.run(context);

    const outputObs = mockStore.observations.find((o) => o.kind === 'node_output');
    expect(outputObs).toBeDefined();

    const endObs = mockStore.observations.find((o) => o.kind === 'node_end');
    expect(endObs).toBeDefined();
    expect(endObs?.content).toContain('completed successfully');
  });

  it('should record node_error observation on failure', async () => {
    class FailingNode extends KindlingNode {
      override async exec() {
        throw new Error('Test failure');
      }
    }

    const node = new FailingNode({ name: 'failing-node' });

    await expect(node.run(context)).rejects.toThrow('Test failure');

    const errorObs = mockStore.observations.find((o) => o.kind === 'node_error');
    expect(errorObs).toBeDefined();
    expect(errorObs?.content).toContain('Test failure');
  });

  it('should attach all observations to the capsule', async () => {
    class TestNode extends KindlingNode {
      override async exec() {
        return 'result';
      }
    }

    const node = new TestNode({ name: 'attach-test' });
    await node.run(context);

    // Should have node_start, node_output, and node_end attached
    expect(mockStore.attachments.length).toBeGreaterThanOrEqual(3);

    // All attachments should be to the same capsule
    const capsuleId = mockStore.attachments[0].capsuleId;
    expect(mockStore.attachments.every((a) => a.capsuleId === capsuleId)).toBe(true);
  });

  it('should close capsule after execution', async () => {
    class TestNode extends KindlingNode {
      override async exec() {
        return 'done';
      }
    }

    const node = new TestNode({ name: 'close-test' });
    await node.run(context);

    expect(mockStore.capsules[0].status).toBe('closed');
  });

  it('should truncate long output', async () => {
    class LongOutputNode extends KindlingNode {
      override async exec() {
        return 'x'.repeat(5000);
      }
    }

    const node = new LongOutputNode({ name: 'long-output' });
    await node.run(context);

    const outputObs = mockStore.observations.find((o) => o.kind === 'node_output');
    expect(outputObs?.content.length).toBeLessThan(5000);
    expect(outputObs?.content).toContain('truncated');
  });
});

describe('KindlingFlow', () => {
  let mockStore: ReturnType<typeof createMockStore>;
  let context: KindlingNodeContext;

  beforeEach(() => {
    mockStore = createMockStore();
    context = {
      store: mockStore,
      scopeIds: { sessionId: 'flow-session' },
    };
  });

  it('should create a flow-level capsule', async () => {
    class StepNode extends KindlingNode {
      override async exec() {
        return 'step done';
      }
    }

    const startNode = new StepNode({ name: 'step-1' });
    const flow = new KindlingFlow(startNode, { name: 'test-flow', intent: 'test' });

    await flow.run(context);

    // Should have capsules for both the flow and the node
    expect(mockStore.capsules.length).toBeGreaterThanOrEqual(2);
  });

  it('should record flow start and end observations', async () => {
    class StepNode extends KindlingNode {
      override async exec() {
        return 'done';
      }
    }

    const startNode = new StepNode({ name: 'flow-step' });
    const flow = new KindlingFlow(startNode, { name: 'my-flow' });

    await flow.run(context);

    const flowStartObs = mockStore.observations.find(
      (o) => o.kind === 'node_start' && o.content.includes('Flow'),
    );
    expect(flowStartObs).toBeDefined();
    expect(flowStartObs?.content).toContain('my-flow');

    const flowEndObs = mockStore.observations.find(
      (o) => o.kind === 'node_end' && o.content.includes('Flow'),
    );
    expect(flowEndObs).toBeDefined();
  });

  it('should orchestrate multiple nodes in sequence', async () => {
    const executionOrder: string[] = [];

    class Node1 extends KindlingNode {
      override async exec() {
        executionOrder.push('node1');
        return 'default';
      }

      override async post(shared: KindlingNodeContext, prepRes: unknown, execRes: unknown) {
        await super.post(shared, prepRes, execRes);
        return 'default';
      }
    }

    class Node2 extends KindlingNode {
      override async exec() {
        executionOrder.push('node2');
        return 'done';
      }
    }

    const node1 = new Node1({ name: 'first' });
    const node2 = new Node2({ name: 'second' });
    node1.next(node2);

    const flow = new KindlingFlow(node1, { name: 'sequence-flow' });
    await flow.run(context);

    expect(executionOrder).toEqual(['node1', 'node2']);
  });
});
