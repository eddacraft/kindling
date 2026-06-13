/**
 * Tests for the high-signal intent emitters (KINTENT-002)
 *
 * The describe block name contains "intent emitter" to satisfy the APS
 * validation hook: `vitest run -- --testNamePattern="intent emitter"`.
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { IntentStore, IntentEmitter } from '../src/intent/index.js';
import type { IntentContext, IntentActor } from '../src/types/index.js';

describe('intent emitter', () => {
  let dir: string;
  let store: IntentStore;
  let emitter: IntentEmitter;

  const context: IntentContext = {
    workspace_id: 'ws-1',
    repo: 'eddacraft/kindling',
    branch: 'feat/intent-emitters-store',
    session_id: 'sess-1',
  };
  const actor: IntentActor = { kind: 'agent', tool: 'claude-code', model: 'opus-4.8' };

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'kindling-emitter-'));
    let n = 0;
    store = new IntentStore(join(dir, 'intent.jsonl'), {
      now: () => '2026-06-14T00:00:00.000Z',
      idFactory: () => `evt-${String(n++).padStart(4, '0')}`,
    });
    emitter = new IntentEmitter({ store, context, actor });
  });

  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  it('emits a session_started event with the configured context and actor', () => {
    const result = emitter.sessionStarted({ objective: 'start work on intent capture' });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value.event_type).toBe('intent.session_started');
    expect(result.value.context.repo).toBe('eddacraft/kindling');
    expect(result.value.actor.tool).toBe('claude-code');
    expect(result.value.sequence).toBe(0);
  });

  it('emits a prompt_submitted event carrying the prompt objective', () => {
    const result = emitter.promptSubmitted({ objective: 'add hash chaining to the store' });
    expect(result.ok && result.value.event_type).toBe('intent.prompt_submitted');
    expect(result.ok && result.value.intent.objective).toBe('add hash chaining to the store');
  });

  it('emits a constraints_updated event preserving constraints', () => {
    const result = emitter.constraintsUpdated({
      objective: 'tighten scope',
      constraints: ['no new deps', 'keep core browser-safe'],
    });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value.event_type).toBe('intent.constraints_updated');
    expect(result.value.intent.constraints).toEqual(['no new deps', 'keep core browser-safe']);
  });

  it('emits a task_reframed event that links to its parent event', () => {
    const first = emitter.sessionStarted({ objective: 'original goal' });
    if (!first.ok) throw new Error('setup failed');

    const result = emitter.taskReframed(
      { objective: 'revised goal' },
      { parent_event_id: first.value.event_id },
    );
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value.event_type).toBe('intent.task_reframed');
    expect(result.value.provenance.parent_event_id).toBe(first.value.event_id);
  });

  it('emits a checkpoint_created event stamped with the commit', () => {
    const result = emitter.checkpointCreated(
      { objective: 'checkpoint after store impl' },
      { commit: 'abc1234' },
    );
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value.event_type).toBe('intent.checkpoint_created');
    expect(result.value.context.commit).toBe('abc1234');
  });

  it('appends every emitted event to the underlying store as a chain', () => {
    emitter.sessionStarted({ objective: 'a' });
    emitter.promptSubmitted({ objective: 'b' });
    emitter.checkpointCreated({ objective: 'c' }, { commit: 'deadbee' });

    const all = store.readAll();
    expect(all.map((e) => e.event_type)).toEqual([
      'intent.session_started',
      'intent.prompt_submitted',
      'intent.checkpoint_created',
    ]);
    expect(store.verify().ok).toBe(true);
  });

  it('allows per-call actor and context overrides', () => {
    const result = emitter.promptSubmitted(
      { objective: 'human-authored prompt' },
      { actor: { kind: 'human', id: 'josh' }, context: { thread_id: 'thr-9' } },
    );
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value.actor.kind).toBe('human');
    expect(result.value.actor.id).toBe('josh');
    // Overrides merge onto the base context rather than replacing it.
    expect(result.value.context.repo).toBe('eddacraft/kindling');
    expect(result.value.context.thread_id).toBe('thr-9');
  });

  it('shallow-merges actor overrides, carrying over unset base fields', () => {
    // Documented behaviour: overriding kind/id keeps the base tool/model.
    const result = emitter.promptSubmitted(
      { objective: 'human prompt' },
      { actor: { kind: 'human', id: 'josh' } },
    );
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.value.actor.kind).toBe('human');
    expect(result.value.actor.id).toBe('josh');
    expect(result.value.actor.tool).toBe('claude-code'); // carried over from base
  });

  it('propagates validation errors from invalid intent payloads', () => {
    const result = emitter.sessionStarted({ objective: '' });
    expect(result.ok).toBe(false);
    expect(store.count()).toBe(0);
  });
});
