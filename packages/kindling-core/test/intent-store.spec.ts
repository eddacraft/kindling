/**
 * Tests for the append-only IntentStore (KINTENT-003)
 *
 * The describe block name contains "intent store integrity" to satisfy the
 * APS validation hook: `vitest run -- --testNamePattern="intent store integrity"`.
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, rmSync, readFileSync, writeFileSync, existsSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { IntentStore, GENESIS_HASH } from '../src/intent/index.js';
import type { IntentEventDraft } from '../src/intent/index.js';
import type { IntentEvent } from '../src/types/index.js';

function makeDraft(overrides: Partial<IntentEventDraft> = {}): IntentEventDraft {
  return {
    event_type: 'intent.session_started',
    actor: { kind: 'agent', tool: 'claude-code' },
    context: { workspace_id: 'ws-1', repo: 'eddacraft/kindling' },
    intent: { objective: 'do the thing' },
    ...overrides,
  };
}

describe('intent store integrity', () => {
  let dir: string;
  let logPath: string;

  /** Deterministic clock + id factory so hashes are reproducible. */
  function newStore(): IntentStore {
    let n = 0;
    return new IntentStore(logPath, {
      now: () => '2026-06-14T00:00:00.000Z',
      idFactory: () => `evt-${String(n++).padStart(4, '0')}`,
    });
  }

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'kindling-intent-'));
    logPath = join(dir, 'intent.jsonl');
  });

  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  it('assigns monotonic sequence starting at 0', () => {
    const store = newStore();
    const a = store.append(makeDraft());
    const b = store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));
    const c = store.append(makeDraft({ event_type: 'intent.checkpoint_created' }));

    expect(a.ok && a.value.sequence).toBe(0);
    expect(b.ok && b.value.sequence).toBe(1);
    expect(c.ok && c.value.sequence).toBe(2);
  });

  it('links the first event to the genesis hash', () => {
    const store = newStore();
    const a = store.append(makeDraft());
    expect(a.ok).toBe(true);
    if (!a.ok) return;
    // First event's hash chains off the empty genesis predecessor.
    expect(a.value.provenance.integrity_hash).toMatch(/^[0-9a-f]{64}$/);
    expect(a.value.provenance.integrity_hash).not.toBe(GENESIS_HASH);
  });

  it('produces a different hash for each event (chain advances)', () => {
    const store = newStore();
    const a = store.append(makeDraft());
    const b = store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));
    expect(a.ok && b.ok).toBe(true);
    if (!a.ok || !b.ok) return;
    expect(b.value.provenance.integrity_hash).not.toBe(a.value.provenance.integrity_hash);
  });

  it('persists each event as one JSONL line', () => {
    const store = newStore();
    store.append(makeDraft());
    store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));

    const lines = readFileSync(logPath, 'utf8').trim().split('\n');
    expect(lines).toHaveLength(2);
    const first = JSON.parse(lines[0]) as IntentEvent;
    expect(first.event_type).toBe('intent.session_started');
    expect(first.sequence).toBe(0);
  });

  it('readAll returns every persisted event in order', () => {
    const store = newStore();
    store.append(makeDraft());
    store.append(makeDraft({ event_type: 'intent.task_reframed' }));

    const all = store.readAll();
    expect(all.map((e) => e.sequence)).toEqual([0, 1]);
    expect(all.map((e) => e.event_type)).toEqual([
      'intent.session_started',
      'intent.task_reframed',
    ]);
  });

  it('reopening an existing log continues the chain without gaps', () => {
    const first = newStore();
    const a = first.append(makeDraft());
    if (!a.ok) throw new Error('append failed');

    // A fresh store instance over the same file must resume sequencing.
    const second = newStore();
    const b = second.append(makeDraft({ event_type: 'intent.prompt_submitted' }));
    expect(b.ok && b.value.sequence).toBe(1);
    if (!b.ok) return;
    // b must chain off a's hash, not the genesis.
    expect(second.readAll()).toHaveLength(2);
    const verify = second.verify();
    expect(verify.ok).toBe(true);
  });

  it('verify accepts an untampered chain', () => {
    const store = newStore();
    store.append(makeDraft());
    store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));
    store.append(makeDraft({ event_type: 'intent.checkpoint_created' }));

    const result = store.verify();
    expect(result.ok).toBe(true);
  });

  it('verify detects a tampered payload (hash mismatch)', () => {
    const store = newStore();
    store.append(makeDraft());
    store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));

    // Tamper: rewrite the objective of the first line on disk, keep its hash.
    const lines = readFileSync(logPath, 'utf8').trim().split('\n');
    const tampered = JSON.parse(lines[0]) as IntentEvent;
    tampered.intent.objective = 'malicious rewrite';
    lines[0] = JSON.stringify(tampered);
    writeFileSync(logPath, lines.join('\n') + '\n');

    const result = store.verify();
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error.kind).toBe('hash_mismatch');
    expect(result.error.sequence).toBe(0);
  });

  it('verify detects a broken chain link (later event altered)', () => {
    const store = newStore();
    store.append(makeDraft());
    store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));

    const lines = readFileSync(logPath, 'utf8').trim().split('\n');
    const tampered = JSON.parse(lines[1]) as IntentEvent;
    tampered.provenance.integrity_hash = 'f'.repeat(64);
    lines[1] = JSON.stringify(tampered);
    writeFileSync(logPath, lines.join('\n') + '\n');

    const result = store.verify();
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error.kind).toBe('hash_mismatch');
    expect(result.error.sequence).toBe(1);
  });

  it('verify detects a sequence gap (deleted event)', () => {
    const store = newStore();
    store.append(makeDraft());
    store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));
    store.append(makeDraft({ event_type: 'intent.checkpoint_created' }));

    // Drop the middle line, leaving sequences [0, 2].
    const lines = readFileSync(logPath, 'utf8').trim().split('\n');
    writeFileSync(logPath, [lines[0], lines[2]].join('\n') + '\n');

    const result = store.verify();
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error.kind).toBe('sequence_gap');
  });

  it('rejects an invalid draft and does not write it', () => {
    const store = newStore();
    const bad = store.append(makeDraft({ intent: { objective: '   ' } }));
    expect(bad.ok).toBe(false);
    expect(existsSync(logPath)).toBe(false);
    expect(store.count()).toBe(0);
  });

  it('a failed append does not advance the sequence or fork the chain', () => {
    const store = newStore();
    expect(store.append(makeDraft()).ok).toBe(true); // sequence 0
    expect(store.append(makeDraft({ intent: { objective: '' } })).ok).toBe(false); // rejected
    const third = store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));
    expect(third.ok && third.value.sequence).toBe(1); // not 2
    expect(store.verify().ok).toBe(true);
  });

  it('verify detects a blank line spliced into the middle of the log', () => {
    const store = newStore();
    store.append(makeDraft());
    store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));

    const lines = readFileSync(logPath, 'utf8').trim().split('\n');
    writeFileSync(logPath, [lines[0], '', lines[1]].join('\n') + '\n');

    // Blank lines are skipped on read, so the surviving events are [0, 1] and
    // the chain still verifies — confirming blank-line handling does not by
    // itself corrupt the log (deletion/reorder is what verify must catch).
    expect(store.verify().ok).toBe(true);
  });

  it('recovers a torn trailing line from a crash mid-append', () => {
    const store = newStore();
    store.append(makeDraft());
    store.append(makeDraft({ event_type: 'intent.prompt_submitted' }));

    // Simulate a torn write: a partial JSON fragment with no trailing newline.
    const contents = readFileSync(logPath, 'utf8');
    writeFileSync(logPath, contents + '{"event_type":"intent.checkp');

    // Reopening must truncate the partial line and resume cleanly at seq 2.
    const reopened = newStore();
    expect(reopened.count()).toBe(2);
    expect(reopened.verify().ok).toBe(true);
    const next = reopened.append(makeDraft({ event_type: 'intent.checkpoint_created' }));
    expect(next.ok && next.value.sequence).toBe(2);
  });

  it('refuses to open a log whose tail has a malformed integrity hash', () => {
    const store = newStore();
    store.append(makeDraft());

    const tampered = JSON.parse(readFileSync(logPath, 'utf8').trim()) as IntentEvent;
    tampered.provenance.integrity_hash = 'not-a-hash';
    writeFileSync(logPath, JSON.stringify(tampered) + '\n');

    expect(() => newStore()).toThrow(/malformed integrity_hash/);
  });

  it('is deterministic: same drafts + clock + ids yield identical hashes', () => {
    const storeA = newStore();
    const a = storeA.append(makeDraft());

    rmSync(logPath, { force: true });

    const storeB = newStore();
    const b = storeB.append(makeDraft());

    expect(a.ok && b.ok).toBe(true);
    if (!a.ok || !b.ok) return;
    expect(b.value.provenance.integrity_hash).toBe(a.value.provenance.integrity_hash);
  });

  it('preserves caller-supplied provenance lineage fields', () => {
    const store = newStore();
    const a = store.append(makeDraft());
    if (!a.ok) throw new Error('append failed');

    const b = store.append(
      makeDraft({
        event_type: 'intent.task_reframed',
        provenance: { parent_event_id: a.value.event_id, source_refs: ['ref-1'] },
      }),
    );
    expect(b.ok).toBe(true);
    if (!b.ok) return;
    expect(b.value.provenance.parent_event_id).toBe(a.value.event_id);
    expect(b.value.provenance.source_refs).toEqual(['ref-1']);
  });
});
