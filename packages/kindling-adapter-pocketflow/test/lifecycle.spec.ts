/**
 * Tests for the PocketFlow lifecycle integration against a REAL kindling daemon.
 *
 * The adapter now writes through the thin {@link Kindling} client (daemon-backed,
 * async) instead of an in-process store. These tests stand up the debug
 * `kindling` binary (`cargo build -p kindling --bin kindling` → `target/debug/kindling`),
 * give each test a fresh temp home + unique socket, and exercise
 * {@link KindlingNode}/{@link KindlingFlow} end-to-end:
 *   node lifecycle → client → daemon → store → retrieval.
 *
 * They SKIP when the binary is absent (e.g. the TS-only `pnpm -r test`, which
 * does not build Rust); a dedicated CI job builds it so they actually run.
 */

import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { afterEach, describe, it, expect } from 'vitest';

import { Kindling } from '@eddacraft/kindling';
import {
  KindlingNode,
  KindlingFlow,
  type KindlingNodeContext,
} from '../src/pocketflow/lifecycle.js';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..', '..');
const BINARY = join(repoRoot, 'target', 'debug', 'kindling');
const HAS_BINARY = existsSync(BINARY);

if (!HAS_BINARY) {
  console.warn(
    `[pocketflow-adapter] daemon binary not found at ${BINARY} — skipping live-daemon ` +
      `integration tests. Build with: cargo build -p kindling --bin kindling`,
  );
}

/** A fixed project root so DB routing is deterministic per test. */
const PROJECT_ROOT = '/tmp/kindling-pocketflow-test/repo';

const tempHomes: string[] = [];

/** Fresh temp home + unique socket + a client wired to the built binary. */
function freshClient(): Kindling {
  const home = mkdtempSync(join(tmpdir(), 'kindling-pf-'));
  tempHomes.push(home);
  return new Kindling({
    socketPath: join(home, 'kindling.sock'),
    projectRoot: PROJECT_ROOT,
    binaryPath: BINARY,
    connectTimeoutMs: 5000,
  });
}

/** Build a node context backed by a live client. */
function freshContext(): KindlingNodeContext {
  return {
    kindling: freshClient(),
    scopeIds: { sessionId: 'pf-session', repoId: PROJECT_ROOT },
  };
}

afterEach(() => {
  for (const home of tempHomes.splice(0)) {
    rmSync(home, { recursive: true, force: true });
  }
});

describe.skipIf(!HAS_BINARY)('KindlingNode (live daemon)', () => {
  describe('successful run', () => {
    it('opens a pocketflow_node capsule on prep', async () => {
      const context = freshContext();
      const node = new KindlingNode({ name: 'test-node', intent: 'test' });
      await node.prep(context);

      expect(context.capsuleId).toBeTruthy();
    });

    it('records node_start, node_output, node_end and surfaces them via retrieval', async () => {
      const context = freshContext();
      // Single-token, FTS-safe node name so MATCH queries stay unambiguous.
      const node = new KindlingNode({ name: 'searchablenode' });
      await node.prep(context);
      await node.post(context, undefined, { result: 'success' });

      const startHits = await context.kindling.retrieve({
        query: 'searchablenode',
        scopeIds: context.scopeIds,
      });
      expect(startHits.candidates.some((c) => c.entity.content.includes('searchablenode'))).toBe(
        true,
      );

      const endHits = await context.kindling.retrieve({
        query: 'completed',
        scopeIds: context.scopeIds,
      });
      expect(
        endHits.candidates.some((c) => c.entity.content.includes('completed successfully')),
      ).toBe(true);
    });

    it('closes the capsule on post', async () => {
      const context = freshContext();
      const node = new KindlingNode({ name: 'close-node' });
      await node.prep(context);
      await node.post(context, undefined, { result: 'success' });

      // The open-by-session lookup no longer finds a session capsule, and a
      // re-close would 404 — both confirm the node capsule is closed.
      await expect(context.kindling.closeCapsule(context.capsuleId!)).rejects.toThrow();
    });

    it('truncates large outputs', async () => {
      const context = freshContext();
      const node = new KindlingNode({ name: 'large-output-node' });
      await node.prep(context);

      const largeOutput = 'x'.repeat(3000);
      await node.post(context, undefined, largeOutput);

      const hits = await context.kindling.retrieve({
        query: 'truncated',
        scopeIds: context.scopeIds,
      });
      const truncated = hits.candidates.find((c) => c.entity.content.includes('[truncated]'));
      expect(truncated).toBeDefined();
      expect(truncated!.entity.content.length).toBeLessThan(3000);
    });
  });

  describe('failed run', () => {
    it('records node_error and node_end (error) on failure, then closes', async () => {
      const context = freshContext();
      const node = new KindlingNode({ name: 'failingnode' });
      await node.prep(context);

      await expect(node.execFallback(undefined, new Error('boomtoken'))).rejects.toThrow(
        'boomtoken',
      );

      const hits = await context.kindling.retrieve({
        query: 'boomtoken',
        scopeIds: context.scopeIds,
      });
      expect(hits.candidates.some((c) => c.entity.content.includes('boomtoken'))).toBe(true);

      // Capsule should be closed (re-close 404s).
      await expect(context.kindling.closeCapsule(context.capsuleId!)).rejects.toThrow();
    });
  });

  describe('full lifecycle via run()', () => {
    it('runs a custom node end to end', async () => {
      const context = freshContext();

      class TestNode extends KindlingNode {
        override async exec(): Promise<string> {
          return 'run result';
        }
      }

      const node = new TestNode({ name: 'custom-node' });
      await node.run(context);

      const hits = await context.kindling.retrieve({
        query: 'run result',
        scopeIds: context.scopeIds,
      });
      expect(hits.candidates.some((c) => c.entity.content.includes('run result'))).toBe(true);
    });
  });

  describe('scope propagation', () => {
    it('tags observations with the context scope', async () => {
      const context = freshContext();
      context.scopeIds = {
        sessionId: 'session-123',
        repoId: PROJECT_ROOT,
        userId: 'user-789',
      };
      const node = new KindlingNode({ name: 'scopednode' });
      await node.prep(context);

      const hits = await context.kindling.retrieve({
        query: 'scopednode',
        scopeIds: context.scopeIds,
      });
      const obs = hits.candidates.find((c) => c.entity.content.includes('scopednode'));
      expect(obs).toBeDefined();
      expect(obs!.entity.scopeIds.userId).toBe('user-789');
      expect(obs!.entity.scopeIds.sessionId).toBe('session-123');
    });
  });
});

describe.skipIf(!HAS_BINARY)('KindlingFlow (live daemon)', () => {
  function freshFlowContext(): KindlingNodeContext {
    return {
      kindling: freshClient(),
      scopeIds: { sessionId: 'pf-flow-session', repoId: PROJECT_ROOT },
    };
  }

  it('opens its own flow-level capsule with the flow intent', async () => {
    const context = freshFlowContext();
    const node = new KindlingNode({ name: 'innernode' });
    const flow = new KindlingFlow(node, { name: 'testflow', intent: 'workflow' });

    await flow.prep(context);

    const hits = await context.kindling.retrieve({
      query: 'testflow',
      scopeIds: context.scopeIds,
    });
    const flowStart = hits.candidates.find(
      (c) =>
        c.entity.content.includes('testflow') &&
        (c.entity as { provenance?: Record<string, unknown> }).provenance?.nodeType === 'flow',
    );
    expect(flowStart).toBeDefined();
  });

  it('emits flow start and end observations', async () => {
    const context = freshFlowContext();
    const node = new KindlingNode({ name: 'innernode' });
    const flow = new KindlingFlow(node, { name: 'lifecycleflow' });

    await flow.prep(context);
    await flow.post(context, undefined, undefined);

    const startHits = await context.kindling.retrieve({
      query: 'lifecycleflow',
      scopeIds: context.scopeIds,
    });
    expect(startHits.candidates.some((c) => c.entity.content.includes('lifecycleflow'))).toBe(true);

    const endHits = await context.kindling.retrieve({
      query: 'completed',
      scopeIds: context.scopeIds,
    });
    expect(endHits.candidates.some((c) => c.entity.content.includes('lifecycleflow'))).toBe(true);
  });

  it('closes the flow capsule and records a failure end when a child node throws', async () => {
    const context = freshFlowContext();

    // A child node whose exec always throws. Single-token names keep FTS
    // MATCH queries unambiguous.
    class FailingNode extends KindlingNode {
      override async exec(): Promise<never> {
        throw new Error('orchestrationboom');
      }
    }

    const node = new FailingNode({ name: 'failingchild' });
    const flow = new KindlingFlow(node, { name: 'failingflow' });

    // Orchestration failure must propagate out of _run / run.
    await expect(flow.run(context)).rejects.toThrow('orchestrationboom');

    // The flow-level capsule must have been closed (a re-close 404s).
    await expect(
      context.kindling.closeCapsule((flow as unknown as { flowCapsuleId: string }).flowCapsuleId),
    ).rejects.toThrow();

    // A failure end observation for the flow must have been written.
    const errorHits = await context.kindling.retrieve({
      query: 'failingflow',
      scopeIds: context.scopeIds,
    });
    const failedEnd = errorHits.candidates.find(
      (c) =>
        c.entity.content.includes('failingflow') &&
        (c.entity as { provenance?: Record<string, unknown> }).provenance?.nodeType === 'flow' &&
        (c.entity as { provenance?: Record<string, unknown> }).provenance?.status === 'error',
    );
    expect(failedEnd).toBeDefined();
  });
});
