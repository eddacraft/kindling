/**
 * Tests for the OpenCode session lifecycle against a REAL kindling daemon.
 *
 * `SessionManager` now persists through the thin {@link Kindling} client
 * (daemon-backed, async) instead of an in-process `CapsuleStore`. These tests
 * stand up the debug `kindling` binary (`cargo build -p kindling --bin kindling`
 * → `target/debug/kindling`), give each test a fresh temp home + unique socket,
 * and exercise start → event → end through the manager → client → daemon path.
 *
 * They SKIP when the binary is absent (e.g. the TS-only `pnpm -r test`, which
 * does not build Rust); a dedicated CI job builds it so they actually run.
 */

import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, it, expect, beforeEach, afterEach } from 'vitest';

import { Kindling } from '@eddacraft/kindling';
import { SessionManager } from '../src/opencode/session.js';
import type { ToolCallEvent, SessionStartEvent, MessageEvent } from '../src/opencode/events.js';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..', '..');
const BINARY = join(repoRoot, 'target', 'debug', 'kindling');
const HAS_BINARY = existsSync(BINARY);

if (!HAS_BINARY) {
  console.warn(
    `[opencode-adapter] daemon binary not found at ${BINARY} — skipping live-daemon ` +
      `integration tests. Build with: cargo build -p kindling --bin kindling`,
  );
}

/** A fixed project root so DB routing is deterministic per test. */
const PROJECT_ROOT = '/tmp/kindling-opencode-test/repo';

const tempHomes: string[] = [];

/** Fresh temp home + unique socket + a client wired to the built binary. */
function freshClient(): Kindling {
  const home = mkdtempSync(join(tmpdir(), 'kindling-oc-'));
  tempHomes.push(home);
  return new Kindling({
    socketPath: join(home, 'kindling.sock'),
    projectRoot: PROJECT_ROOT,
    binaryPath: BINARY,
    connectTimeoutMs: 5000,
  });
}

afterEach(() => {
  for (const home of tempHomes.splice(0)) {
    rmSync(home, { recursive: true, force: true });
  }
});

describe.skipIf(!HAS_BINARY)('SessionManager (live daemon)', () => {
  let client: Kindling;
  let manager: SessionManager;

  beforeEach(() => {
    client = freshClient();
    manager = new SessionManager(client);
  });

  describe('onSessionStart', () => {
    it('opens a session capsule on first start', async () => {
      const context = await manager.onSessionStart({
        sessionId: 's1',
        intent: 'Fix bug in auth',
        repoId: PROJECT_ROOT,
      });

      expect(context.sessionId).toBe('s1');
      expect(context.repoId).toBe(PROJECT_ROOT);
      expect(context.activeCapsuleId).toBeTruthy();
      expect(context.eventCount).toBe(0);

      const open = await client.getOpenCapsule('s1');
      expect(open?.id).toBe(context.activeCapsuleId);
      expect(open?.type).toBe('session');
      expect(open?.intent).toBe('Fix bug in auth');
      expect(open?.status).toBe('open');
    });

    it('uses default intent if not provided', async () => {
      const context = await manager.onSessionStart({ sessionId: 's1' });
      const open = await client.getOpenCapsule('s1');
      expect(open?.intent).toBe('OpenCode session');
      expect(open?.id).toBe(context.activeCapsuleId);
    });

    it('returns existing context if session already active', async () => {
      const context1 = await manager.onSessionStart({ sessionId: 's1', intent: 'First' });
      const context2 = await manager.onSessionStart({ sessionId: 's1', intent: 'Second' });
      expect(context1.activeCapsuleId).toBe(context2.activeCapsuleId);
    });

    it('recovers an existing open capsule from the daemon', async () => {
      // Open a capsule directly through the client, then start a fresh manager
      // that does not yet track the session — it must discover the open capsule.
      const capsule = await client.openCapsule({
        kind: 'session',
        intent: 'Pre-existing',
        scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
      });
      await client.appendObservation(
        {
          kind: 'message',
          content: 'pre-existing observation',
          scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
        },
        { capsuleId: capsule.id },
      );

      const recoveredManager = new SessionManager(client);
      const context = await recoveredManager.onSessionStart({ sessionId: 's1' });

      expect(context.activeCapsuleId).toBe(capsule.id);
      expect(context.eventCount).toBe(1);
    });
  });

  describe('onEvent', () => {
    it('appends a tool_call observation to the active capsule', async () => {
      const context = await manager.onSessionStart({ sessionId: 's1', repoId: PROJECT_ROOT });

      const event: ToolCallEvent = {
        type: 'tool_call',
        timestamp: Date.now(),
        sessionId: 's1',
        repoId: PROJECT_ROOT,
        toolName: 'read_file',
        args: { path: 'test.ts' },
        result: 'searchableneedle contents',
        duration_ms: 100,
      };

      const result = await manager.onEvent(event);

      expect(result.observation).toBeDefined();
      expect(result.error).toBeUndefined();
      expect(result.observation!.kind).toBe('tool_call');
      expect(context.eventCount).toBe(1);

      // The observation is retrievable through the daemon.
      const hits = await client.retrieve({
        query: 'searchableneedle',
        scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
      });
      expect(hits.candidates.some((c) => c.entity.content.includes('searchableneedle'))).toBe(true);
    });

    it('skips session lifecycle events', async () => {
      await manager.onSessionStart({ sessionId: 's1' });
      const event: SessionStartEvent = {
        type: 'session_start',
        timestamp: Date.now(),
        sessionId: 's1',
        intent: 'Test',
      };
      const result = await manager.onEvent(event);
      expect(result.skipped).toBe(true);
      expect(result.observation).toBeUndefined();
    });

    it('returns an error for an event with no active session', async () => {
      const event: MessageEvent = {
        type: 'message',
        timestamp: Date.now(),
        sessionId: 'unknown-session',
        role: 'user',
        content: 'test',
      };
      const result = await manager.onEvent(event);
      expect(result.error).toContain('No active session found');
    });

    it('processes multiple events sequentially', async () => {
      const context = await manager.onSessionStart({ sessionId: 's1', repoId: PROJECT_ROOT });

      for (let i = 0; i < 3; i++) {
        const event: ToolCallEvent = {
          type: 'tool_call',
          timestamp: Date.now() + i,
          sessionId: 's1',
          repoId: PROJECT_ROOT,
          toolName: 'read_file',
          args: { path: `${i}.ts` },
        };
        const result = await manager.onEvent(event);
        expect(result.observation).toBeDefined();
      }

      expect(context.eventCount).toBe(3);
    });

    it('preserves observation timestamps from events', async () => {
      await manager.onSessionStart({ sessionId: 's1', repoId: PROJECT_ROOT });
      const eventTimestamp = 1234567890000;
      const event: MessageEvent = {
        type: 'message',
        timestamp: eventTimestamp,
        sessionId: 's1',
        repoId: PROJECT_ROOT,
        role: 'user',
        content: 'timestamped message',
      };
      const result = await manager.onEvent(event);
      expect(result.observation!.ts).toBe(eventTimestamp);
    });
  });

  describe('onSessionEnd', () => {
    it('closes the active capsule', async () => {
      const context = await manager.onSessionStart({ sessionId: 's1', repoId: PROJECT_ROOT });

      const closed = await manager.onSessionEnd('s1');

      expect(closed.id).toBe(context.activeCapsuleId);
      expect(closed.status).toBe('closed');

      // No longer open for the session.
      const open = await client.getOpenCapsule('s1');
      expect(open).toBeNull();
    });

    it('removes the session from active sessions', async () => {
      await manager.onSessionStart({ sessionId: 's1' });
      expect(manager.isSessionActive('s1')).toBe(true);
      await manager.onSessionEnd('s1');
      expect(manager.isSessionActive('s1')).toBe(false);
    });

    it('records a summary when provided', async () => {
      await manager.onSessionStart({ sessionId: 's1', repoId: PROJECT_ROOT });

      const closed = await manager.onSessionEnd('s1', {
        reason: 'completed',
        summaryContent: 'Fixed authsummarytoken bug in auth.ts',
        summaryConfidence: 0.9,
      });

      expect(closed.status).toBe('closed');

      // The daemon links the summary via summaries.capsule_id rather than
      // threading summaryId onto the closed capsule, so verify the summary
      // landed by retrieving its content.
      const hits = await client.retrieve({
        query: 'authsummarytoken',
        scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
      });
      expect(hits.candidates.some((c) => c.entity.content.includes('authsummarytoken'))).toBe(true);
    });

    it('masks secrets in the summary before persisting', async () => {
      await manager.onSessionStart({ sessionId: 's1', repoId: PROJECT_ROOT });

      await manager.onSessionEnd('s1', {
        reason: 'completed',
        summaryContent: 'Done masksummaryneedle work; leftover api_key: SECRET123abc',
        summaryConfidence: 0.9,
      });

      // The summary is retrievable by its non-secret token, but the secret
      // value must have been redacted before the daemon stored it.
      const hits = await client.retrieve({
        query: 'masksummaryneedle',
        scopeIds: { sessionId: 's1', repoId: PROJECT_ROOT },
      });
      const summaryHit = hits.candidates.find((c) =>
        c.entity.content.includes('masksummaryneedle'),
      );
      expect(summaryHit).toBeDefined();
      expect(summaryHit!.entity.content).not.toContain('SECRET123abc');
      expect(summaryHit!.entity.content).toContain('[REDACTED]');
    });

    it('throws for an unknown session', async () => {
      await expect(manager.onSessionEnd('unknown-session')).rejects.toThrow(
        'No active session found',
      );
    });
  });

  describe('session management', () => {
    it('tracks multiple active sessions', async () => {
      await manager.onSessionStart({ sessionId: 's1' });
      await manager.onSessionStart({ sessionId: 's2' });
      await manager.onSessionStart({ sessionId: 's3' });
      expect(manager.getActiveSessions().sort()).toEqual(['s1', 's2', 's3']);
    });

    it('gets and checks session context', async () => {
      const context = await manager.onSessionStart({ sessionId: 's1', repoId: PROJECT_ROOT });
      const retrieved = manager.getSession('s1');
      expect(retrieved?.activeCapsuleId).toBe(context.activeCapsuleId);
      expect(manager.getSession('unknown')).toBeUndefined();
      expect(manager.isSessionActive('s1')).toBe(true);
    });
  });
});
