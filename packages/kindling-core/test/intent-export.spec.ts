/**
 * Tests for the signed intent export bundle (KINTENT-005)
 *
 * The describe block name contains "intent export" to satisfy the APS
 * validation hook: `vitest run -- --testNamePattern="intent export"`.
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import {
  IntentStore,
  createIntentExport,
  verifyIntentExport,
  serializeIntentExport,
  parseIntentExport,
  canonicalize,
  INTENT_EXPORT_BUNDLE_VERSION,
  INTENT_EXPORT_SIGNATURE_ALG,
} from '../src/intent/index.js';
import type { IntentEventDraft } from '../src/intent/index.js';
import type { IntentEvent } from '../src/types/index.js';

const KEY = 'workspace-secret-key';

function makeDraft(overrides: Partial<IntentEventDraft> = {}): IntentEventDraft {
  return {
    event_type: 'intent.session_started',
    actor: { kind: 'agent', tool: 'claude-code' },
    context: { workspace_id: 'ws-1', repo: 'eddacraft/kindling' },
    intent: { objective: 'do the thing' },
    ...overrides,
  };
}

describe('intent export', () => {
  let dir: string;
  let logPath: string;

  /** Append `count` events through a deterministic store, return them in order. */
  function buildEvents(count: number): IntentEvent[] {
    let n = 0;
    const store = new IntentStore(logPath, {
      now: () => '2026-06-17T00:00:00.000Z',
      idFactory: () => `evt-${String(n++).padStart(4, '0')}`,
    });
    for (let i = 0; i < count; i += 1) {
      const result = store.append(
        makeDraft({ intent: { objective: `objective ${i}`, constraints: [`c${i}`] } }),
      );
      if (!result.ok) {
        throw new Error(`setup append failed: ${JSON.stringify(result.error)}`);
      }
    }
    return store.readAll();
  }

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'kindling-export-'));
    logPath = join(dir, 'intent.log.jsonl');
  });

  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  it('builds a manifest describing the exported events', () => {
    const events = buildEvents(3);
    const { manifest, jsonl } = createIntentExport(events, {
      key: KEY,
      keyId: 'ws-1',
      now: () => '2026-06-17T12:00:00.000Z',
    });

    expect(manifest.bundle_version).toBe(INTENT_EXPORT_BUNDLE_VERSION);
    expect(manifest.created_at).toBe('2026-06-17T12:00:00.000Z');
    expect(manifest.event_count).toBe(3);
    expect(manifest.sequence_range).toEqual([0, 2]);
    expect(manifest.tip_integrity_hash).toBe(events[2].provenance.integrity_hash);
    expect(manifest.signature.alg).toBe(INTENT_EXPORT_SIGNATURE_ALG);
    expect(manifest.signature.key_id).toBe('ws-1');
    expect(manifest.signature.value).toMatch(/^[0-9a-f]{64}$/);
    expect(jsonl.split('\n')).toHaveLength(3);
  });

  it('emits canonical JSONL: one sorted-key event per line, ascending by sequence', () => {
    const events = buildEvents(3);
    const { jsonl } = createIntentExport(events, { key: KEY });
    const lines = jsonl.split('\n');

    expect(lines).toHaveLength(3);
    lines.forEach((line, i) => {
      expect(line).toBe(canonicalize(events[i]));
    });
    // Canonical objects begin with the alphabetically-first key.
    expect(lines[0].startsWith('{"actor":')).toBe(true);
  });

  it('verifies a freshly created bundle', () => {
    const events = buildEvents(4);
    const bundle = createIntentExport(events, { key: KEY });
    expect(verifyIntentExport(bundle, KEY)).toEqual({ ok: true, value: true });
  });

  it('seals an empty range with null range/tip and remains verifiable', () => {
    const events = buildEvents(2);
    const bundle = createIntentExport(events, { key: KEY, fromSequence: 99 });

    expect(bundle.jsonl).toBe('');
    expect(bundle.manifest.event_count).toBe(0);
    expect(bundle.manifest.sequence_range).toBeNull();
    expect(bundle.manifest.tip_integrity_hash).toBeNull();
    expect(verifyIntentExport(bundle, KEY).ok).toBe(true);
  });

  it('filters to the inclusive [fromSequence, toSequence] range', () => {
    const events = buildEvents(5);
    const bundle = createIntentExport(events, { key: KEY, fromSequence: 1, toSequence: 3 });

    expect(bundle.manifest.event_count).toBe(3);
    expect(bundle.manifest.sequence_range).toEqual([1, 3]);
    expect(bundle.manifest.tip_integrity_hash).toBe(events[3].provenance.integrity_hash);
    expect(verifyIntentExport(bundle, KEY).ok).toBe(true);
  });

  it('detects a tampered body via bundle_hash_mismatch', () => {
    const events = buildEvents(3);
    const bundle = createIntentExport(events, { key: KEY });
    const tampered = {
      ...bundle,
      jsonl: bundle.jsonl.replace('objective 0', 'objective HACKED'),
    };

    const result = verifyIntentExport(tampered, KEY);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.kind).toBe('bundle_hash_mismatch');
    }
  });

  it('rejects verification under the wrong key via signature_mismatch', () => {
    const events = buildEvents(3);
    const bundle = createIntentExport(events, { key: KEY });

    const result = verifyIntentExport(bundle, 'a-different-key');
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.kind).toBe('signature_mismatch');
    }
  });

  it('detects manifest metadata that disagrees with the body', () => {
    const events = buildEvents(3);
    const bundle = createIntentExport(events, { key: KEY });
    const tampered = {
      ...bundle,
      manifest: { ...bundle.manifest, event_count: 99 },
    };

    const result = verifyIntentExport(tampered, KEY);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.kind).toBe('manifest_mismatch');
    }
  });

  it('detects a forged signed-but-not-body field via signature_mismatch', () => {
    const events = buildEvents(3);
    const bundle = createIntentExport(events, { key: KEY });
    // created_at is signed but not cross-checked against the body, so altering
    // it can only be caught by the signature.
    const tampered = {
      ...bundle,
      manifest: { ...bundle.manifest, created_at: '1999-01-01T00:00:00.000Z' },
    };

    const result = verifyIntentExport(tampered, KEY);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.kind).toBe('signature_mismatch');
    }
  });

  it('rejects an unsupported signature algorithm', () => {
    const events = buildEvents(2);
    const bundle = createIntentExport(events, { key: KEY });
    const tampered = {
      ...bundle,
      manifest: {
        ...bundle.manifest,
        signature: { ...bundle.manifest.signature, alg: 'MD5' as never },
      },
    };

    const result = verifyIntentExport(tampered, KEY);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.kind).toBe('unsupported_alg');
    }
  });

  it('is deterministic for the same events, key, and clock', () => {
    const events = buildEvents(3);
    const opts = { key: KEY, keyId: 'ws-1', now: () => '2026-06-17T12:00:00.000Z' };
    const a = createIntentExport(events, opts);
    const b = createIntentExport(events, opts);
    expect(a).toEqual(b);
  });

  it('round-trips through serialize/parse and still verifies', () => {
    const events = buildEvents(4);
    const bundle = createIntentExport(events, { key: KEY });
    const text = serializeIntentExport(bundle);
    const restored = parseIntentExport(text);

    expect(restored).toEqual(bundle);
    expect(verifyIntentExport(restored, KEY).ok).toBe(true);
  });

  it('anchors the bundle to the source chain tip', () => {
    const events = buildEvents(5);
    const bundle = createIntentExport(events, { key: KEY });
    const tip = events[events.length - 1].provenance.integrity_hash;
    expect(bundle.manifest.tip_integrity_hash).toBe(tip);
  });

  it('requires a non-empty signing key', () => {
    const events = buildEvents(1);
    expect(() => createIntentExport(events, { key: '' })).toThrow(/non-empty signing key/);
  });
});
