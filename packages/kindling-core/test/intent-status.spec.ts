/**
 * Tests for the intent capture health report (KINTENT-006)
 *
 * The describe block name contains "intent status" to satisfy the APS
 * validation hook: `vitest run -- --testNamePattern="intent status"`.
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, rmSync, readFileSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { IntentStore, computeIntentStatus, formatIntentStatus } from '../src/intent/index.js';
import type { IntentEventDraft } from '../src/intent/index.js';

function makeDraft(overrides: Partial<IntentEventDraft> = {}): IntentEventDraft {
  return {
    event_type: 'intent.session_started',
    actor: { kind: 'agent', tool: 'claude-code' },
    context: { workspace_id: 'ws-1', repo: 'eddacraft/kindling' },
    intent: { objective: 'do the thing' },
    ...overrides,
  };
}

describe('intent status', () => {
  let dir: string;
  let logPath: string;

  /** Deterministic clock + id factory so the report is reproducible. */
  function newStore(occurredAt = '2026-06-18T00:00:00.000Z'): IntentStore {
    let n = 0;
    return new IntentStore(logPath, {
      now: () => occurredAt,
      idFactory: () => `evt-${String(n++).padStart(4, '0')}`,
    });
  }

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'kindling-intent-status-'));
    logPath = join(dir, 'intent.jsonl');
  });

  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  describe('empty / uninitialized log', () => {
    it('reports not initialized with no events', () => {
      const store = newStore();
      const report = computeIntentStatus(store);

      expect(report.initialized).toBe(false);
      expect(report.event_count).toBe(0);
      expect(report.last_event).toBeNull();
      expect(report.last_event_age_ms).toBeNull();
      expect(report.backlog).toBe(0);
    });

    it('is not healthy when uninitialized (silent capture failure)', () => {
      const store = newStore();
      const report = computeIntentStatus(store);
      expect(report.healthy).toBe(false);
    });

    it('reports integrity ok for an empty log', () => {
      const store = newStore();
      const report = computeIntentStatus(store);
      expect(report.integrity.ok).toBe(true);
      expect(report.integrity.error).toBeUndefined();
    });

    it('zeroes every known event type', () => {
      const store = newStore();
      const report = computeIntentStatus(store);
      expect(report.counts_by_type).toEqual({
        'intent.session_started': 0,
        'intent.prompt_submitted': 0,
        'intent.constraints_updated': 0,
        'intent.task_reframed': 0,
        'intent.checkpoint_created': 0,
      });
    });
  });

  describe('populated log', () => {
    it('reports event count and the last event', () => {
      const store = newStore();
      store.append(makeDraft());
      store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));
      const last = store.append(makeDraft({ event_type: 'intent.checkpoint_created' }));

      const report = computeIntentStatus(store);

      expect(report.initialized).toBe(true);
      expect(report.event_count).toBe(3);
      expect(last.ok).toBe(true);
      if (!last.ok) return;
      expect(report.last_event).toEqual({
        sequence: 2,
        occurred_at: '2026-06-18T00:00:00.000Z',
        event_type: 'intent.checkpoint_created',
        event_id: last.value.event_id,
      });
    });

    it('counts events by type', () => {
      const store = newStore();
      store.append(makeDraft());
      store.append(makeDraft());
      store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));
      store.append(makeDraft({ event_type: 'intent.checkpoint_created' }));

      const report = computeIntentStatus(store);

      expect(report.counts_by_type['intent.session_started']).toBe(2);
      expect(report.counts_by_type['intent.prompt_submitted']).toBe(1);
      expect(report.counts_by_type['intent.checkpoint_created']).toBe(1);
      expect(report.counts_by_type['intent.task_reframed']).toBe(0);
      expect(report.counts_by_type['intent.constraints_updated']).toBe(0);
    });

    it('is healthy when initialized, intact, and fresh', () => {
      const store = newStore();
      store.append(makeDraft());

      const report = computeIntentStatus(store, {
        now: () => '2026-06-18T00:00:00.000Z',
      });

      expect(report.healthy).toBe(true);
      expect(report.integrity.ok).toBe(true);
      expect(report.stale).toBe(false);
    });
  });

  describe('staleness', () => {
    it('computes the age of the last event against now', () => {
      const store = newStore('2026-06-18T00:00:00.000Z');
      store.append(makeDraft());

      const report = computeIntentStatus(store, {
        now: () => '2026-06-18T00:00:05.000Z',
      });

      expect(report.last_event_age_ms).toBe(5000);
    });

    it('flags a log as stale past the threshold', () => {
      const store = newStore('2026-06-18T00:00:00.000Z');
      store.append(makeDraft());

      const report = computeIntentStatus(store, {
        now: () => '2026-06-18T01:00:00.000Z', // 1h later
        staleAfterMs: 60_000, // 1 min
      });

      expect(report.stale).toBe(true);
      expect(report.healthy).toBe(false);
    });

    it('is not stale within the threshold', () => {
      const store = newStore('2026-06-18T00:00:00.000Z');
      store.append(makeDraft());

      const report = computeIntentStatus(store, {
        now: () => '2026-06-18T00:00:30.000Z', // 30s later
        staleAfterMs: 60_000,
      });

      expect(report.stale).toBe(false);
    });

    it('never reports stale without a threshold', () => {
      const store = newStore('2020-01-01T00:00:00.000Z');
      store.append(makeDraft());

      const report = computeIntentStatus(store, {
        now: () => '2026-06-18T00:00:00.000Z', // years later
      });

      expect(report.stale).toBe(false);
    });

    it('is not stale exactly at the threshold (strict greater-than)', () => {
      const store = newStore('2026-06-18T00:00:00.000Z');
      store.append(makeDraft());

      const report = computeIntentStatus(store, {
        now: () => '2026-06-18T00:01:00.000Z', // exactly 60s later
        staleAfterMs: 60_000,
      });

      expect(report.last_event_age_ms).toBe(60_000);
      expect(report.stale).toBe(false);
    });

    it('reports null age for an unparseable occurred_at', () => {
      const store = newStore();
      // Hand-write a log line whose occurred_at is not RFC3339, then read it
      // back through a fresh store (mirrors the tamper/migration path).
      store.append(makeDraft());
      const line = JSON.parse(readFileSync(logPath, 'utf8').trim());
      line.occurred_at = 'June 18 2026'; // V8 Date.parse accepts this; chrono would not
      writeFileSync(logPath, JSON.stringify(line) + '\n');

      const fresh = new IntentStore(logPath, { now: () => '2026-06-18T00:00:00.000Z' });
      const report = computeIntentStatus(fresh, { now: () => '2026-06-18T00:00:10.000Z' });

      expect(report.last_event_age_ms).toBeNull();
      expect(report.stale).toBe(false);
    });
  });

  describe('backlog', () => {
    it('treats every event as backlog when nothing has been exported', () => {
      const store = newStore();
      store.append(makeDraft());
      store.append(makeDraft());
      store.append(makeDraft());

      const report = computeIntentStatus(store);
      expect(report.backlog).toBe(3);
    });

    it('counts only events newer than the export watermark', () => {
      const store = newStore();
      store.append(makeDraft()); // seq 0
      store.append(makeDraft()); // seq 1
      store.append(makeDraft()); // seq 2
      store.append(makeDraft()); // seq 3

      // Exported through sequence 1 → seq 2 and 3 remain.
      const report = computeIntentStatus(store, { exportedThrough: 1 });
      expect(report.backlog).toBe(2);
    });

    it('reports zero backlog when fully exported', () => {
      const store = newStore();
      store.append(makeDraft()); // seq 0
      store.append(makeDraft()); // seq 1

      const report = computeIntentStatus(store, { exportedThrough: 1 });
      expect(report.backlog).toBe(0);
    });
  });

  describe('integrity', () => {
    it('surfaces a broken chain in the report', () => {
      const store = newStore();
      store.append(makeDraft());
      store.append(makeDraft());

      // Tamper with the first persisted line.
      const lines = readFileSync(logPath, 'utf8').trim().split('\n');
      const first = JSON.parse(lines[0]);
      first.intent.objective = 'tampered';
      lines[0] = JSON.stringify(first);
      writeFileSync(logPath, lines.join('\n') + '\n');

      const report = computeIntentStatus(store);

      expect(report.integrity.ok).toBe(false);
      expect(report.integrity.error?.kind).toBe('hash_mismatch');
      expect(report.healthy).toBe(false);
    });
  });

  describe('unrecognized event type', () => {
    it('excludes unknown types from counts_by_type but keeps them in totals', () => {
      const store = newStore();
      store.append(makeDraft());
      store.append(makeDraft());

      // Hand-edit the second line to an unknown type (only reachable off-path).
      const lines = readFileSync(logPath, 'utf8').trim().split('\n');
      const second = JSON.parse(lines[1]);
      second.event_type = 'intent.unknown_future_type';
      lines[1] = JSON.stringify(second);
      writeFileSync(logPath, lines.join('\n') + '\n');

      const fresh = new IntentStore(logPath, { now: () => '2026-06-18T00:00:00.000Z' });
      const report = computeIntentStatus(fresh);

      expect(report.event_count).toBe(2);
      expect(report.backlog).toBe(2);
      // Only the recognized event is tallied by type.
      expect(report.counts_by_type['intent.session_started']).toBe(1);
      const tallied = Object.values(report.counts_by_type).reduce((a, b) => a + b, 0);
      expect(tallied).toBe(1);
      expect(tallied).toBeLessThan(report.event_count);
    });
  });

  describe('formatIntentStatus', () => {
    it('renders a human-readable report', () => {
      const store = newStore();
      store.append(makeDraft());
      store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));

      const report = computeIntentStatus(store, {
        now: () => '2026-06-18T00:00:00.000Z',
      });
      const text = formatIntentStatus(report);

      expect(text).toContain('Events: 2');
      expect(text).toContain('Integrity: ok');
      expect(text).toContain('intent.prompt_submitted');
    });

    it('flags an unhealthy report', () => {
      const store = newStore();
      const text = formatIntentStatus(computeIntentStatus(store));
      expect(text).toMatch(/no events|uninitialized/i);
    });

    it('marks the last event [STALE] when past the threshold', () => {
      const store = newStore('2026-06-18T00:00:00.000Z');
      store.append(makeDraft());

      const report = computeIntentStatus(store, {
        now: () => '2026-06-18T02:00:00.000Z',
        staleAfterMs: 60_000,
      });
      const text = formatIntentStatus(report);

      expect(text).toContain('[STALE]');
      expect(text).toContain('Capture: unhealthy');
      expect(text).toContain('2h ago');
    });

    it('renders a broken integrity chain with its kind and message', () => {
      const store = newStore();
      store.append(makeDraft());
      store.append(makeDraft());

      const lines = readFileSync(logPath, 'utf8').trim().split('\n');
      const first = JSON.parse(lines[0]);
      first.intent.objective = 'tampered';
      lines[0] = JSON.stringify(first);
      writeFileSync(logPath, lines.join('\n') + '\n');

      const text = formatIntentStatus(computeIntentStatus(store));

      expect(text).toContain('Integrity: BROKEN');
      expect(text).toContain('hash_mismatch');
    });
  });
});
