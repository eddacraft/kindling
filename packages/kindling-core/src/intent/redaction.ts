/**
 * Intent redaction boundary (KINTENT-004)
 *
 * Intent events capture free-text objectives, constraints, and source
 * references supplied by humans and agents. Any of those fields can
 * accidentally carry a secret (an API key pasted into a prompt, a token in a
 * URL). This module masks that content *before* it is persisted or exported,
 * while recording — in the event's existing {@link IntentRedaction} block —
 * exactly which field paths were touched and under which policy. Governance
 * consumers (anvil) thus learn *that* and *where* redaction happened without
 * ever seeing the secret.
 *
 * ## Mechanism, not policy
 *
 * The redactor is a configurable transform. It does not decide *whether* a
 * workspace should redact — callers opt in by passing an {@link IntentRedactor}
 * to the {@link IntentStore} (the single persistence choke point) or by running
 * it before export. The default policy ships a conservative set of secret
 * patterns; callers may supply their own paths, patterns, and placeholder.
 *
 * ## Two redaction modes
 *
 * - **Value patterns** mask only the matched substring inside a field,
 *   preserving the surrounding text so the objective stays legible.
 * - **Path redaction** replaces a whole field value regardless of content, for
 *   fields a workspace considers categorically sensitive.
 *
 * ## Determinism / Rust-port parity
 *
 * Redaction feeds the integrity hash chain, so it must be reproducible. Field
 * traversal order is fixed, `redacted_fields` is sorted, and the default
 * patterns deliberately avoid lookahead/lookbehind so they remain portable to
 * Rust's `regex` crate (which lacks them). Any reimplementation must match the
 * pattern set, placeholder, and dotted/`[index]` path syntax byte-for-byte.
 */

import type { IntentActor, IntentContext, IntentPayload, IntentRedaction } from '../types/index.js';
import type { IntentEventDraft } from './store.js';

/** Replacement inserted in place of redacted content. */
export const DEFAULT_REDACTION_PLACEHOLDER = '[REDACTED]';

/**
 * A value pattern. The matched substring is replaced with the policy
 * placeholder. Patterns must be global (`/g`) so every occurrence in a field
 * is masked.
 */
export interface RedactionPattern {
  /** Stable identifier for the kind of secret (diagnostics only). */
  name: string;
  /** Global RegExp; every match is replaced with the placeholder. */
  pattern: RegExp;
}

/**
 * A redaction policy. `version` is stamped into `redaction.policy_version` so
 * downstream consumers can tell which ruleset processed an event.
 */
export interface RedactionPolicy {
  /** Recorded in `redaction.policy_version` when the policy is applied. */
  version: string;
  /**
   * Field paths whose entire value is replaced regardless of content. Matched
   * as a prefix: `intent.scope_out` redacts every `intent.scope_out[i]`.
   */
  redactPaths?: string[];
  /** Value patterns scanned across every redactable string field. */
  patterns?: RedactionPattern[];
  /** Replacement text for redacted content. Defaults to `[REDACTED]`. */
  placeholder?: string;
}

/**
 * Conservative default secret patterns. Ordered so more specific keys are
 * consumed before generic ones. All are anchored or length-bounded to keep
 * false positives low, and none use lookaround (Rust-`regex` portable).
 */
export const DEFAULT_REDACTION_PATTERNS: readonly RedactionPattern[] = [
  { name: 'anthropic-api-key', pattern: /sk-ant-[A-Za-z0-9_-]{20,}/g },
  { name: 'openai-api-key', pattern: /sk-(?:proj-)?[A-Za-z0-9]{20,}/g },
  { name: 'aws-access-key-id', pattern: /\bAKIA[0-9A-Z]{16}\b/g },
  { name: 'github-token', pattern: /\bgh[pousr]_[A-Za-z0-9]{36,}\b/g },
  { name: 'github-fine-grained-pat', pattern: /\bgithub_pat_[A-Za-z0-9_]{22,}\b/g },
  { name: 'slack-token', pattern: /\bxox[baprs]-[A-Za-z0-9-]{10,}\b/g },
  { name: 'google-api-key', pattern: /\bAIza[0-9A-Za-z_-]{35}\b/g },
  { name: 'bearer-token', pattern: /\bBearer\s+[A-Za-z0-9\-._~+/]+=*/g },
  { name: 'private-key-block', pattern: /-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----/g },
  {
    name: 'labeled-secret',
    pattern:
      /(?:api[-_]?key|apikey|secret|token|password|passwd|pwd)["']?\s*[:=]\s*["']?[^\s"']{6,}/gi,
  },
];

/**
 * Default policy: the built-in secret patterns, the `[REDACTED]` placeholder,
 * and no categorical path redaction.
 */
export const DEFAULT_REDACTION_POLICY: RedactionPolicy = {
  version: 'kindling-intent-redaction/1',
  patterns: [...DEFAULT_REDACTION_PATTERNS],
};

/**
 * Masks sensitive content in an {@link IntentEventDraft} before it is persisted
 * or exported, recording the affected field paths and policy version in the
 * event's {@link IntentRedaction} block. Pure: the input draft is never
 * mutated.
 */
export class IntentRedactor {
  private readonly policy: RedactionPolicy;
  private readonly placeholder: string;
  private readonly patterns: readonly RedactionPattern[];

  constructor(policy: RedactionPolicy = DEFAULT_REDACTION_POLICY) {
    this.policy = policy;
    this.placeholder = policy.placeholder ?? DEFAULT_REDACTION_PLACEHOLDER;
    this.patterns = policy.patterns ?? DEFAULT_REDACTION_PATTERNS;
  }

  /** The version this redactor stamps onto events it processes. */
  get policyVersion(): string {
    return this.policy.version;
  }

  /**
   * Return a redacted copy of `draft`. Every redactable string field is masked
   * (whole-value for configured paths, matched-substring for value patterns);
   * touched paths are recorded in `redaction.redacted_fields` and the policy
   * version is stamped into `redaction.policy_version`.
   */
  redactDraft(draft: IntentEventDraft): IntentEventDraft {
    const redacted = new Set<string>();

    const actor: IntentActor = {
      kind: draft.actor.kind,
      id: this.scalar(draft.actor.id, 'actor.id', redacted),
      tool: this.scalar(draft.actor.tool, 'actor.tool', redacted),
      model: this.scalar(draft.actor.model, 'actor.model', redacted),
    };

    const context: IntentContext = {
      workspace_id: this.required(draft.context.workspace_id, 'context.workspace_id', redacted),
      repo: this.required(draft.context.repo, 'context.repo', redacted),
      branch: this.scalar(draft.context.branch, 'context.branch', redacted),
      commit: this.scalar(draft.context.commit, 'context.commit', redacted),
      session_id: this.scalar(draft.context.session_id, 'context.session_id', redacted),
      thread_id: this.scalar(draft.context.thread_id, 'context.thread_id', redacted),
    };

    const intent: IntentPayload = {
      objective: this.required(draft.intent.objective, 'intent.objective', redacted),
      constraints: this.list(draft.intent.constraints, 'intent.constraints', redacted),
      success_criteria: this.list(
        draft.intent.success_criteria,
        'intent.success_criteria',
        redacted,
      ),
      scope_in: this.list(draft.intent.scope_in, 'intent.scope_in', redacted),
      scope_out: this.list(draft.intent.scope_out, 'intent.scope_out', redacted),
    };

    const provenance: IntentEventDraft['provenance'] = draft.provenance
      ? {
          parent_event_id: this.scalar(
            draft.provenance.parent_event_id,
            'provenance.parent_event_id',
            redacted,
          ),
          source_refs: this.list(draft.provenance.source_refs, 'provenance.source_refs', redacted),
        }
      : draft.provenance;

    return {
      ...draft,
      actor,
      context,
      intent,
      provenance,
      redaction: this.mergeRedaction(draft.redaction, redacted),
    };
  }

  /** Redact a single required string field, recording its path if changed. */
  private required(value: string, path: string, redacted: Set<string>): string {
    const masked = this.redactString(value, path);
    if (masked !== value) {
      redacted.add(path);
    }
    return masked;
  }

  /** Redact a single optional string field, recording its path if changed. */
  private scalar(
    value: string | undefined,
    path: string,
    redacted: Set<string>,
  ): string | undefined {
    return value === undefined ? undefined : this.required(value, path, redacted);
  }

  /** Redact each element of an optional string-array field by `base[index]`. */
  private list(
    value: string[] | undefined,
    base: string,
    redacted: Set<string>,
  ): string[] | undefined {
    if (value === undefined) {
      return undefined;
    }
    return value.map((entry, index) => {
      const path = `${base}[${index}]`;
      const masked = this.redactString(entry, path);
      if (masked !== entry) {
        redacted.add(path);
      }
      return masked;
    });
  }

  /** Whole-value redaction for configured paths, else value-pattern masking. */
  private redactString(value: string, path: string): string {
    if (this.matchesRedactPath(path)) {
      return this.placeholder;
    }
    let out = value;
    for (const { pattern } of this.patterns) {
      pattern.lastIndex = 0;
      out = out.replace(pattern, this.placeholder);
    }
    return out;
  }

  /** A leaf path is redacted when a policy path equals it or is its prefix. */
  private matchesRedactPath(path: string): boolean {
    const paths = this.policy.redactPaths;
    if (!paths) {
      return false;
    }
    return paths.some(
      (entry) => path === entry || path.startsWith(`${entry}.`) || path.startsWith(`${entry}[`),
    );
  }

  /**
   * Combine any caller-supplied redaction metadata with the fields this pass
   * masked. `redacted_fields` is the sorted union (omitted when empty, per the
   * canonicalization contract); the caller's `policy_version` wins if present.
   */
  private mergeRedaction(
    base: IntentRedaction | undefined,
    redacted: Set<string>,
  ): IntentRedaction {
    const fields = [...new Set([...(base?.redacted_fields ?? []), ...redacted])].sort();
    const merged: IntentRedaction = {
      ...base,
      policy_version: base?.policy_version ?? this.policy.version,
    };
    if (fields.length > 0) {
      merged.redacted_fields = fields;
    }
    return merged;
  }
}
