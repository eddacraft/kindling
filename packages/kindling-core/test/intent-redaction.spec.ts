/**
 * Tests for the intent redaction boundary (KINTENT-004)
 *
 * The describe block name contains "intent redaction" to satisfy the APS
 * validation hook: `vitest run -- --testNamePattern="intent redaction"`.
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import {
  IntentRedactor,
  IntentStore,
  DEFAULT_REDACTION_POLICY,
  type IntentEventDraft,
  type RedactionPolicy,
} from '../src/intent/index.js';

function makeDraft(overrides: Partial<IntentEventDraft> = {}): IntentEventDraft {
  return {
    event_type: 'intent.prompt_submitted',
    actor: { kind: 'agent', tool: 'claude-code' },
    context: { workspace_id: 'ws-1', repo: 'eddacraft/kindling' },
    intent: { objective: 'do the thing' },
    ...overrides,
  };
}

describe('intent redaction', () => {
  describe('value-pattern masking', () => {
    it('masks an OpenAI-style key inside free text but preserves surrounding content', () => {
      const redactor = new IntentRedactor();
      const out = redactor.redactDraft(
        makeDraft({
          intent: {
            objective: 'use key sk-abcdefghijklmnopqrstuvwxyz012345 to call the API',
          },
        }),
      );

      expect(out.intent.objective).toBe('use key [REDACTED] to call the API');
      expect(out.redaction?.redacted_fields).toEqual(['intent.objective']);
    });

    it('masks an Anthropic key without being swallowed by the OpenAI pattern', () => {
      const redactor = new IntentRedactor();
      const secret = 'sk-ant-' + 'a1b2c3d4e5f6g7h8i9j0k1l2m3n4';
      const out = redactor.redactDraft(
        makeDraft({ intent: { objective: `token ${secret} here` } }),
      );

      expect(out.intent.objective).toBe('token [REDACTED] here');
    });

    it('masks AWS access key ids, GitHub tokens, and bearer tokens', () => {
      const redactor = new IntentRedactor();
      const out = redactor.redactDraft(
        makeDraft({
          intent: {
            objective: 'creds',
            constraints: [
              'AKIAIOSFODNN7EXAMPLE',
              'ghp_0123456789abcdefghijklmnopqrstuvwxyzAB',
              'Authorization: Bearer abc.def.ghi',
            ],
          },
        }),
      );

      expect(out.intent.constraints).toEqual([
        '[REDACTED]',
        '[REDACTED]',
        'Authorization: [REDACTED]',
      ]);
      expect(out.redaction?.redacted_fields).toEqual([
        'intent.constraints[0]',
        'intent.constraints[1]',
        'intent.constraints[2]',
      ]);
    });

    it('masks labeled secrets like password: ...', () => {
      const redactor = new IntentRedactor();
      const out = redactor.redactDraft(
        makeDraft({ intent: { objective: 'set password: hunter2pass and continue' } }),
      );

      expect(out.intent.objective).toBe('set [REDACTED] and continue');
    });

    it('scans source_refs for tokens embedded in URLs', () => {
      const redactor = new IntentRedactor();
      const out = redactor.redactDraft(
        makeDraft({
          provenance: {
            source_refs: ['https://api.example.com/v1?token=supersecretvalue123'],
          },
        }),
      );

      expect(out.provenance?.source_refs?.[0]).toContain('[REDACTED]');
      expect(out.redaction?.redacted_fields).toEqual(['provenance.source_refs[0]']);
    });

    it('leaves a clean draft untouched but still stamps the policy version', () => {
      const redactor = new IntentRedactor();
      const draft = makeDraft();
      const out = redactor.redactDraft(draft);

      expect(out.intent.objective).toBe('do the thing');
      expect(out.redaction?.redacted_fields).toBeUndefined();
      expect(out.redaction?.policy_version).toBe(DEFAULT_REDACTION_POLICY.version);
    });

    it('preserves the actor kind and required context fields', () => {
      const redactor = new IntentRedactor();
      const out = redactor.redactDraft(makeDraft());

      expect(out.actor.kind).toBe('agent');
      expect(out.context.workspace_id).toBe('ws-1');
      expect(out.context.repo).toBe('eddacraft/kindling');
    });

    it('does not mutate the caller-supplied draft', () => {
      const redactor = new IntentRedactor();
      const draft = makeDraft({ intent: { objective: 'sk-abcdefghijklmnopqrstuvwxyz012345' } });
      redactor.redactDraft(draft);

      expect(draft.intent.objective).toBe('sk-abcdefghijklmnopqrstuvwxyz012345');
      expect(draft.redaction).toBeUndefined();
    });

    it('is idempotent — redacting an already-redacted draft is a no-op on content', () => {
      const redactor = new IntentRedactor();
      const once = redactor.redactDraft(
        makeDraft({ intent: { objective: 'sk-abcdefghijklmnopqrstuvwxyz012345' } }),
      );
      const twice = redactor.redactDraft(once);

      expect(twice.intent.objective).toBe(once.intent.objective);
      expect(twice.redaction?.redacted_fields).toEqual(once.redaction?.redacted_fields);
    });
  });

  describe('path-based redaction', () => {
    it('redacts an entire configured field path regardless of content', () => {
      const policy: RedactionPolicy = {
        version: 'test/1',
        redactPaths: ['context.commit', 'intent.objective'],
      };
      const redactor = new IntentRedactor(policy);
      const out = redactor.redactDraft(
        makeDraft({
          context: { workspace_id: 'ws-1', repo: 'r', commit: 'deadbeef' },
          intent: { objective: 'totally benign text' },
        }),
      );

      expect(out.context.commit).toBe('[REDACTED]');
      expect(out.intent.objective).toBe('[REDACTED]');
      expect(out.redaction?.redacted_fields).toEqual(['context.commit', 'intent.objective']);
    });

    it('treats a path entry as a prefix, redacting every element of a list field', () => {
      const policy: RedactionPolicy = { version: 'test/1', redactPaths: ['intent.scope_out'] };
      const redactor = new IntentRedactor(policy);
      const out = redactor.redactDraft(
        makeDraft({ intent: { objective: 'ok', scope_out: ['secret-a', 'secret-b'] } }),
      );

      expect(out.intent.scope_out).toEqual(['[REDACTED]', '[REDACTED]']);
      expect(out.redaction?.redacted_fields).toEqual([
        'intent.scope_out[0]',
        'intent.scope_out[1]',
      ]);
    });
  });

  describe('redaction metadata', () => {
    it('merges with caller-supplied redaction metadata and keeps the existing policy version', () => {
      const redactor = new IntentRedactor();
      const out = redactor.redactDraft(
        makeDraft({
          intent: { objective: 'sk-abcdefghijklmnopqrstuvwxyz012345' },
          redaction: { redacted_fields: ['actor.id'], policy_version: 'caller/9' },
        }),
      );

      expect(out.redaction?.policy_version).toBe('caller/9');
      expect(out.redaction?.redacted_fields).toEqual(['actor.id', 'intent.objective']);
    });

    it('honours a custom placeholder and pattern set', () => {
      const policy: RedactionPolicy = {
        version: 'custom/1',
        placeholder: '***',
        patterns: [{ name: 'ticket', pattern: /JIRA-\d+/g }],
      };
      const redactor = new IntentRedactor(policy);
      const out = redactor.redactDraft(makeDraft({ intent: { objective: 'fixes JIRA-1234 now' } }));

      expect(out.intent.objective).toBe('fixes *** now');
      // The default secret patterns are replaced, not augmented.
      const out2 = redactor.redactDraft(
        makeDraft({ intent: { objective: 'sk-abcdefghijklmnopqrstuvwxyz012345' } }),
      );
      expect(out2.intent.objective).toBe('sk-abcdefghijklmnopqrstuvwxyz012345');
    });
  });

  describe('store integration', () => {
    let dir: string;
    let logPath: string;

    beforeEach(() => {
      dir = mkdtempSync(join(tmpdir(), 'kindling-redact-'));
      logPath = join(dir, 'intent.jsonl');
    });

    afterEach(() => {
      rmSync(dir, { recursive: true, force: true });
    });

    it('persists redacted content and a verifiable hash chain', () => {
      let n = 0;
      const store = new IntentStore(logPath, {
        now: () => '2026-06-16T00:00:00.000Z',
        idFactory: () => `evt-${String(n++).padStart(4, '0')}`,
        redactor: new IntentRedactor(),
      });

      const result = store.append(
        makeDraft({ intent: { objective: 'deploy with sk-abcdefghijklmnopqrstuvwxyz012345' } }),
      );

      expect(result.ok).toBe(true);
      if (!result.ok) return;
      expect(result.value.intent.objective).toBe('deploy with [REDACTED]');
      expect(result.value.redaction.redacted_fields).toEqual(['intent.objective']);
      expect(result.value.redaction.policy_version).toBe(DEFAULT_REDACTION_POLICY.version);

      // The secret must never reach disk.
      const persisted = store.readAll();
      expect(JSON.stringify(persisted)).not.toContain('sk-abcdefghijklmnopqrstuvwxyz012345');

      // The integrity chain covers the redacted form.
      const verified = store.verify();
      expect(verified.ok).toBe(true);
    });
  });
});
