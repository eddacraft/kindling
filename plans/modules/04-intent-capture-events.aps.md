# Intent Capture Events (Kindling)

| ID      | Owner  | Status      |
| ------- | ------ | ----------- |
| KINTENT | @aneki | In Progress |

## Purpose

Define and ship Kindling's first-class **intent capture primitive**: a low-friction,
append-only event stream that records what a developer/agent intended, under what
constraints, and in what execution context.

This turns intent into durable, queryable input for downstream governance and
verification systems (Anvil), while keeping capture close to the developer workflow.

## In Scope

- Canonical `IntentEvent` envelope schema (versioned)
- Event emission from high-signal hooks (session start, prompt submit, task revise,
  commit checkpoint)
- Local append-only storage format with integrity hash chain
- Correlation IDs linking events to repo, branch, commit, session, and agent/tool
- Redaction boundaries for sensitive fields
- Export sink for downstream ingestion (JSONL bundle)

## Out of Scope

- Central policy evaluation (Anvil-owned)
- Merge/deploy gate decisions
- Full semantic interpretation of intent quality
- Team/org multi-tenant data governance

## Interfaces

**Depends on:**

- Existing Kindling hook runtime and storage adapters
- `@eddacraft/kindling-core` event bus abstractions

**Exposes:**

- `IntentEvent` schema + TypeScript types
- `kindling intent status` (capture health)
- `kindling intent export --since <ref>` JSONL bundle
- Stable export contract for Anvil ingestion

## Ready Checklist

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one task defined

## Event Contract (v1)

```ts
interface IntentEvent {
  schema_version: '1.0';
  event_id: string; // uuid
  occurred_at: string; // ISO8601
  sequence: number; // monotonic per repo workspace
  event_type:
    | 'intent.session_started'
    | 'intent.prompt_submitted'
    | 'intent.constraints_updated'
    | 'intent.task_reframed'
    | 'intent.checkpoint_created';
  actor: {
    kind: 'human' | 'agent';
    id?: string;
    tool?: string; // codex/claude/pi/etc
    model?: string;
  };
  context: {
    workspace_id: string;
    repo: string;
    branch?: string;
    commit?: string;
    session_id?: string;
    thread_id?: string;
  };
  intent: {
    objective: string;
    constraints?: string[];
    success_criteria?: string[];
    scope_in?: string[];
    scope_out?: string[];
  };
  provenance: {
    parent_event_id?: string;
    source_refs?: string[];
    integrity_hash: string;
  };
  redaction: {
    redacted_fields?: string[];
    policy_version?: string;
  };
}
```

## Tasks

### KINTENT-001: Finalize canonical `IntentEvent` schema

- **Intent:** Establish a durable, versioned capture contract.
- **Expected Outcome:** Shared schema + type definitions in core package.
- **Validation:** `pnpm -r test --filter "*kindling-core*" -- --testNamePattern="IntentEvent schema"`
- **Status:** Merged (PR #59, merged 2026-06-11)

### KINTENT-002: Implement hook emitters for high-signal moments

- **Intent:** Capture intent at points with highest signal-to-noise.
- **Expected Outcome:** Emitters wired for session start, prompt submit, constraint/task updates, checkpoint creation.
- **Validation:** `pnpm test -- --testNamePattern="intent emitter"`
- **Dependencies:** KINTENT-001
- **Status:** Merged (PR #61, merged 2026-06-13)
- **Notes:** `IntentEmitter` in `kindling-core` (`src/intent/emitter.ts`) exposes typed
  methods for the five high-signal moments; each shapes the `event_type`, merges base
  context/actor with per-call overrides, and appends via `IntentStore`. Mechanism only —
  callers supply intent payload and context (no git/session sniffing in core).

### KINTENT-003: Add append-only store with hash chaining

- **Intent:** Make local intent logs tamper-evident and replayable.
- **Expected Outcome:** JSONL-backed log with rolling integrity hash and monotonic sequencing.
- **Validation:** `pnpm test -- --testNamePattern="intent store integrity"`
- **Dependencies:** KINTENT-001
- **Status:** Merged (PR #61, merged 2026-06-13)
- **Notes:** `IntentStore` in `kindling-core` (`src/intent/store.ts`) owns `sequence`
  (monotonic from 0) and `provenance.integrity_hash` (un-keyed SHA-256 chain over a
  key-sorted canonical JSON of each event). `verify()` detects payload tampering, broken
  links, and sequence gaps; constructor recovers torn trailing lines from a crash
  mid-append. Single-writer per workspace (documented invariant, no file lock).
  Canonicalization contract documented for Rust-port parity (omit absent optionals,
  never serialize as `null`).

### KINTENT-004: Add redaction boundary and safe serialization

- **Intent:** Prevent secret leakage while preserving governance utility.
- **Expected Outcome:** Configurable redaction on known sensitive paths before persistence/export.
- **Validation:** `pnpm test -- --testNamePattern="intent redaction"`
- **Dependencies:** KINTENT-001
- **Status:** Merged (PR #67, merged 2026-06-18)
- **Notes:** `IntentRedactor` in `kindling-core` (`src/intent/redaction.ts`) is a
  configurable transform applied before persistence/export. Two modes: **value
  patterns** mask only the matched substring (default set covers Anthropic/OpenAI
  keys, AWS access key ids, GitHub classic + fine-grained tokens, Slack tokens,
  Google API keys, bearer tokens, private-key blocks, and labeled
  `secret:`/`password=` pairs); **path redaction** (`policy.redactPaths`, prefix
  matched) replaces a whole field value regardless of content. Masked field paths
  are recorded in `redaction.redacted_fields` (sorted, dotted/`[index]` syntax)
  and `redaction.policy_version` is stamped. Wired into `IntentStore` via the
  optional `redactor` option so redaction runs _before_ hashing — the integrity
  chain covers the masked form and secrets never reach disk. Pure (no input
  mutation), deterministic, and idempotent. Default patterns deliberately avoid
  lookaround for Rust `regex`-crate portability; the pattern set, placeholder, and
  path syntax are the parity contract owed by the Rust port. 14 tests under
  `-t "intent redaction"`. Unblocks KINTENT-005 export.

### KINTENT-005: Implement export command for Anvil ingestion

- **Intent:** Provide deterministic transfer of intent records downstream.
- **Expected Outcome:** `kindling intent export` outputs signed/hashed JSONL bundle with metadata manifest.
- **Validation:** `pnpm test -- --testNamePattern="intent export"`
- **Dependencies:** KINTENT-002, KINTENT-003, KINTENT-004
- **Status:** Merged (PR #68, merged 2026-06-18)
- **Notes:** Export seals a sequence range of persisted (already-redacted) intent
  events into a portable, signed bundle for Anvil ingestion. The store's hash
  chain is un-keyed (tamper-_evident_); export adds the keyed authentication the
  store deferred — an **HMAC-SHA256** over a canonical manifest that binds the
  bundle body, the chain tip, the exported sequence range, and the event count.
  Bundle body is canonical JSONL (one sorted-key event per line, ascending by
  sequence) so `bundle_hash` is reproducible cross-implementation; the
  byte-for-byte canonicalization contract is shared with the store via
  `intent/canonical.ts` (extracted from `store.ts`). `createIntentExport` +
  `verifyIntentExport` (+ serialize/parse round-trip) live in
  `kindling-core/src/intent/export.ts`. `verify()` detects body tamper
  (`bundle_hash_mismatch`), forged/wrong-key signatures (`signature_mismatch`,
  timing-safe compare), and manifest/body disagreement (`manifest_mismatch`).
  Core exposes `fromSequence`/`toSequence`; the `--since <ref>` CLI flag maps
  onto these and is wired by the Rust CLI port (deferred). HMAC alg, manifest
  field set, and canonical JSONL are the parity contract owed by the Rust port.

### KINTENT-006: Add observability + capture health command

- **Intent:** Make silent capture failures impossible to miss.
- **Expected Outcome:** `kindling intent status` shows emitter health, last event timestamp, backlog, and integrity state.
- **Validation:** `pnpm test -- --testNamePattern="intent status"`
- **Dependencies:** KINTENT-002, KINTENT-003
- **Status:** In Progress
- **Notes:** `computeIntentStatus` + `formatIntentStatus` in `kindling-core`
  (`src/intent/status.ts`) derive a point-in-time capture health report. The
  report combines the four required signals into one `IntentStatusReport`:
  **emitter health** (`initialized`, `counts_by_type` per known event type,
  rolled-up `healthy`), **last event timestamp** (`last_event` + derived
  `last_event_age_ms`), **backlog** (events with `sequence > exportedThrough`;
  whole log when no watermark), and **integrity state** (`integrity` from
  `IntentStore.verify()`). `healthy = initialized && integrity.ok && !stale`;
  a non-zero backlog deliberately does not flip `healthy` (it reflects capture,
  not export keep-up — documented). Pure given two seams: `now` clock (for
  age/staleness) and `exportedThrough`. Count, last event, type tallies, and
  backlog all come from a single `readAll()` pass so they cannot disagree;
  `verify()` re-reads disk independently (integrity must be recomputed, not
  trusted). Staleness is strict `>` `staleAfterMs` (omit to disable). Age uses
  an RFC3339 guard before `Date.parse` so the `null` boundary matches a Rust
  `chrono` port rather than V8's permissive `Date.parse` grammar — the report
  is the parity contract owed by the Rust port (field set, `healthy` formula,
  RFC3339/`null` boundary, strict-`>` staleness, single-pass derivation,
  `formatIntentStatus` text). 22 tests `-t "intent status"`. The
  `kindling intent status` CLI wiring is deferred to the Rust CLI port (as the
  export `--since` flag was), which maps `exportedThrough`/`staleAfterMs` onto
  flags.
