# Intent Capture Events (Kindling)

| ID      | Owner  | Status |
| ------- | ------ | ------ |
| KINTENT | @aneki | Ready  |

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
- **Status:** In Progress

### KINTENT-002: Implement hook emitters for high-signal moments

- **Intent:** Capture intent at points with highest signal-to-noise.
- **Expected Outcome:** Emitters wired for session start, prompt submit, constraint/task updates, checkpoint creation.
- **Validation:** `pnpm test -- --testNamePattern="intent emitter"`
- **Dependencies:** KINTENT-001
- **Status:** Ready

### KINTENT-003: Add append-only store with hash chaining

- **Intent:** Make local intent logs tamper-evident and replayable.
- **Expected Outcome:** JSONL-backed log with rolling integrity hash and monotonic sequencing.
- **Validation:** `pnpm test -- --testNamePattern="intent store integrity"`
- **Dependencies:** KINTENT-001
- **Status:** Ready

### KINTENT-004: Add redaction boundary and safe serialization

- **Intent:** Prevent secret leakage while preserving governance utility.
- **Expected Outcome:** Configurable redaction on known sensitive paths before persistence/export.
- **Validation:** `pnpm test -- --testNamePattern="intent redaction"`
- **Dependencies:** KINTENT-001
- **Status:** Ready

### KINTENT-005: Implement export command for Anvil ingestion

- **Intent:** Provide deterministic transfer of intent records downstream.
- **Expected Outcome:** `kindling intent export` outputs signed/hashed JSONL bundle with metadata manifest.
- **Validation:** `pnpm test -- --testNamePattern="intent export"`
- **Dependencies:** KINTENT-002, KINTENT-003, KINTENT-004
- **Status:** Ready

### KINTENT-006: Add observability + capture health command

- **Intent:** Make silent capture failures impossible to miss.
- **Expected Outcome:** `kindling intent status` shows emitter health, last event timestamp, backlog, and integrity state.
- **Validation:** `pnpm test -- --testNamePattern="intent status"`
- **Dependencies:** KINTENT-002, KINTENT-003
- **Status:** Draft
