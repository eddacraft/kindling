/**
 * Append-only IntentStore with rolling integrity-hash chain (KINTENT-003)
 *
 * Intent events are persisted to a JSONL log, one event per line. The store
 * is the sole authority for the two fields emitters cannot compute on their
 * own:
 *
 * - `sequence`: monotonic, starting at 0, per repo workspace (one log file).
 * - `provenance.integrity_hash`: a rolling hash that chains each event to its
 *   predecessor, making the log tamper-evident and replayable.
 *
 * Emitters hand the store a {@link IntentEventDraft}; the store stamps the
 * sequence + integrity hash, fills `event_id`/`occurred_at`, validates the
 * completed envelope, then appends it.
 *
 * ## Integrity guarantee
 *
 * The chain is an *un-keyed* SHA-256 hash chain. It makes the log
 * tamper-**evident** against accidental corruption and naive edits: any change
 * to a persisted event (or a deleted/reordered event) breaks {@link
 * IntentStore.verify}. It is **not** an authentication mechanism — an actor
 * with write access to the file can rewrite events and recompute the whole
 * chain from {@link GENESIS_HASH}, since the algorithm is public. Adversarial
 * guarantees would require keying the hash (HMAC with a workspace-local secret)
 * or signing events; that is deliberately out of scope here and is the
 * province of the signed export bundle (KINTENT-005).
 *
 * ## Single-writer invariant
 *
 * A log file must have at most one live {@link IntentStore} writing to it at a
 * time. The store caches its tail state (`nextSequence`/`lastHash`) in memory
 * for O(1) appends; two instances or two processes appending concurrently will
 * assign duplicate sequence numbers and fork the chain (which {@link
 * IntentStore.verify} then reports as a `sequence_gap`). There is no file lock
 * — single-writer is the caller's responsibility, consistent with a
 * per-workspace local log.
 */

import { createHash, randomUUID } from 'crypto';
import { appendFileSync, mkdirSync, readFileSync, writeFileSync, existsSync } from 'fs';
import { dirname } from 'path';
import type {
  IntentEvent,
  IntentEventInput,
  IntentActor,
  IntentContext,
  IntentPayload,
  IntentProvenance,
  IntentRedaction,
  IntentEventType,
  Result,
  ValidationError,
} from '../types/index.js';
import { ok, err, INTENT_EVENT_SCHEMA_VERSION } from '../types/index.js';
import { validateIntentEvent } from '../validation/index.js';

/**
 * Predecessor hash used when computing the integrity hash of the first
 * (genesis) event. It is never stored as an event's own `integrity_hash` — the
 * first event's `integrity_hash` is always a full 64-char SHA-256 hex string.
 */
export const GENESIS_HASH = '';

/** A persisted `integrity_hash` is a lowercase SHA-256 hex digest. */
const HASH_PATTERN = /^[0-9a-f]{64}$/;

/**
 * A partial intent event handed to the store. The store assigns `sequence`,
 * `provenance.integrity_hash`, and (unless provided) `event_id`/`occurred_at`.
 */
export interface IntentEventDraft {
  event_type: IntentEventType;
  actor: IntentActor;
  context: IntentContext;
  intent: IntentPayload;
  /** Semantic lineage; `integrity_hash` is assigned by the store. */
  provenance?: Omit<IntentProvenance, 'integrity_hash'>;
  redaction?: IntentRedaction;
  /** Override the generated id (determinism/tests). */
  event_id?: string;
  /** Override the capture timestamp (determinism/tests). */
  occurred_at?: string;
}

/**
 * Options controlling store behaviour. Clock and id factory are injectable so
 * the hash chain is fully reproducible in tests and across replays.
 */
export interface IntentStoreOptions {
  now?: () => string;
  idFactory?: () => string;
}

/**
 * Why an integrity check failed.
 */
export type IntentIntegrityErrorKind = 'sequence_gap' | 'hash_mismatch' | 'parse_error';

export interface IntentIntegrityError {
  kind: IntentIntegrityErrorKind;
  /** Sequence number of the offending event, when known. */
  sequence?: number;
  message: string;
}

/**
 * Deterministically serialize a value for hashing.
 *
 * Canonicalization contract (must be matched byte-for-byte by any other
 * implementation that recomputes these hashes, e.g. the Rust port):
 * - Object keys are sorted ascending (UTF-16 code-unit order, JS default).
 * - Keys whose value is `undefined` are omitted entirely. An optional field
 *   that is absent must hash identically to one set to `undefined` — it must
 *   NOT be serialized as `null`. (Rust `Option::None` must serialize as an
 *   omitted key, i.e. `skip_serializing_if = "Option::is_none"`.)
 * - Arrays preserve element order (it is semantically meaningful), and an
 *   empty array `[]` is "present but empty" — it hashes differently from an
 *   absent/`undefined` field. Callers must not coerce `[]` to `undefined`.
 * - Strings/numbers/booleans/`null` use `JSON.stringify`.
 */
function canonicalize(value: unknown): string {
  if (typeof value === 'function' || typeof value === 'symbol' || typeof value === 'bigint') {
    throw new TypeError(`canonicalize: non-serializable value of type ${typeof value}`);
  }
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value) ?? 'null';
  }
  if (Array.isArray(value)) {
    return `[${value.map(canonicalize).join(',')}]`;
  }
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record)
    .filter((k) => record[k] !== undefined)
    .sort();
  const entries = keys.map((k) => `${JSON.stringify(k)}:${canonicalize(record[k])}`);
  return `{${entries.join(',')}}`;
}

/**
 * The hashable view of an event: everything except the integrity hash itself.
 */
function hashablePayload(event: IntentEvent): Record<string, unknown> {
  return {
    schema_version: event.schema_version,
    event_id: event.event_id,
    occurred_at: event.occurred_at,
    sequence: event.sequence,
    event_type: event.event_type,
    actor: event.actor,
    context: event.context,
    intent: event.intent,
    provenance: {
      parent_event_id: event.provenance.parent_event_id,
      source_refs: event.provenance.source_refs,
    },
    redaction: event.redaction,
  };
}

/**
 * Compute the rolling integrity hash for an event given its predecessor's hash.
 */
function computeIntegrityHash(prevHash: string, event: IntentEvent): string {
  return createHash('sha256')
    .update(prevHash)
    .update('\n')
    .update(canonicalize(hashablePayload(event)))
    .digest('hex');
}

export class IntentStore {
  private readonly filePath: string;
  private readonly now: () => string;
  private readonly idFactory: () => string;

  /** Cached tail state for O(1) appends (single-writer assumption). */
  private nextSequence: number;
  private lastHash: string;
  private lastEvent: IntentEvent | undefined;

  constructor(filePath: string, options: IntentStoreOptions = {}) {
    this.filePath = filePath;
    this.now = options.now ?? (() => new Date().toISOString());
    this.idFactory = options.idFactory ?? (() => randomUUID());

    // A complete append always ends in '\n'. A trailing line without one is a
    // torn write from a crash mid-append; truncate it so the valid prefix is
    // recoverable rather than wedging the store on a JSON.parse error.
    this.recoverTornTail();

    let existing: IntentEvent[];
    try {
      existing = this.readAll();
    } catch (cause) {
      throw new Error(
        `IntentStore: corrupt intent log at ${this.filePath}: ${(cause as Error).message}`,
      );
    }

    const tail = existing[existing.length - 1];
    const tailHash = tail?.provenance?.integrity_hash;
    if (tail && (typeof tailHash !== 'string' || !HASH_PATTERN.test(tailHash))) {
      throw new Error(
        `IntentStore: corrupt intent log at ${this.filePath}: tail event (sequence ${tail.sequence}) has a missing or malformed integrity_hash`,
      );
    }
    this.nextSequence = existing.length;
    this.lastHash = tailHash ?? GENESIS_HASH;
    this.lastEvent = tail;
  }

  /**
   * Append a draft to the log. Assigns sequence + integrity hash, validates the
   * completed envelope, and persists it. Returns the persisted event, or
   * validation errors (in which case nothing is written).
   */
  append(draft: IntentEventDraft): Result<IntentEvent, ValidationError[]> {
    const sequence = this.nextSequence;
    const candidate: IntentEvent = {
      schema_version: INTENT_EVENT_SCHEMA_VERSION,
      event_id: draft.event_id ?? this.idFactory(),
      occurred_at: draft.occurred_at ?? this.now(),
      sequence,
      event_type: draft.event_type,
      actor: draft.actor,
      context: draft.context,
      intent: draft.intent,
      provenance: {
        parent_event_id: draft.provenance?.parent_event_id,
        source_refs: draft.provenance?.source_refs,
        integrity_hash: GENESIS_HASH, // placeholder, replaced below
      },
      redaction: draft.redaction ?? {},
    };

    candidate.provenance.integrity_hash = computeIntegrityHash(this.lastHash, candidate);

    // Validate the completed envelope before it touches disk.
    const input: IntentEventInput = candidate;
    const validated = validateIntentEvent(input);
    if (!validated.ok) {
      return validated;
    }
    const event = validated.value;

    this.ensureDir();
    appendFileSync(this.filePath, JSON.stringify(event) + '\n');

    this.nextSequence += 1;
    this.lastHash = event.provenance.integrity_hash;
    this.lastEvent = event;

    return ok(event);
  }

  /**
   * Read every persisted event in log order. Reads from disk (authoritative).
   */
  readAll(): IntentEvent[] {
    if (!existsSync(this.filePath)) {
      return [];
    }
    const contents = readFileSync(this.filePath, 'utf8');
    const events: IntentEvent[] = [];
    for (const line of contents.split('\n')) {
      const trimmed = line.trim();
      if (trimmed.length === 0) {
        continue;
      }
      events.push(JSON.parse(trimmed) as IntentEvent);
    }
    return events;
  }

  /**
   * Number of events currently in the log. O(1) from cached tail state under
   * the single-writer invariant.
   */
  count(): number {
    return this.nextSequence;
  }

  /** The most recent event, or undefined for an empty log. */
  last(): IntentEvent | undefined {
    return this.lastEvent;
  }

  /**
   * Recompute the integrity chain from disk and confirm it is intact:
   * sequences are contiguous from 0 and every recorded hash matches a
   * recomputation over its predecessor. Tamper-evidence lives here.
   */
  verify(): Result<true, IntentIntegrityError> {
    let events: IntentEvent[];
    try {
      events = this.readAll();
    } catch (cause) {
      return err({
        kind: 'parse_error',
        message: `Failed to parse intent log: ${(cause as Error).message}`,
      });
    }

    let prevHash = GENESIS_HASH;
    for (let i = 0; i < events.length; i += 1) {
      const event = events[i];

      // A persisted line can be syntactically valid JSON yet structurally
      // malformed (hand-edited, truncated field). Guard before touching
      // provenance so verify() returns a Result rather than throwing.
      const recordedHash = event.provenance?.integrity_hash;
      if (typeof recordedHash !== 'string') {
        return err({
          kind: 'parse_error',
          sequence: typeof event.sequence === 'number' ? event.sequence : undefined,
          message: `Event at index ${i} is missing provenance.integrity_hash`,
        });
      }

      if (event.sequence !== i) {
        return err({
          kind: 'sequence_gap',
          sequence: event.sequence,
          message: `Expected sequence ${i} but found ${event.sequence}`,
        });
      }

      const expected = computeIntegrityHash(prevHash, event);
      if (recordedHash !== expected) {
        return err({
          kind: 'hash_mismatch',
          sequence: event.sequence,
          message: `Integrity hash mismatch at sequence ${event.sequence}`,
        });
      }

      prevHash = recordedHash;
    }

    return ok(true);
  }

  private ensureDir(): void {
    const dir = dirname(this.filePath);
    if (!existsSync(dir)) {
      mkdirSync(dir, { recursive: true });
    }
  }

  /**
   * Repair a torn trailing line left by a crash mid-append. Every complete
   * append ends in '\n', so a non-empty file not ending in '\n' has a partial
   * final line; truncate to the last newline (or empty), preserving the valid
   * prefix. A torn write that happened to land on a '\n' boundary is instead
   * caught later by {@link verify} as a hash mismatch.
   */
  private recoverTornTail(): void {
    if (!existsSync(this.filePath)) {
      return;
    }
    const contents = readFileSync(this.filePath, 'utf8');
    if (contents.length === 0 || contents.endsWith('\n')) {
      return;
    }
    const lastNewline = contents.lastIndexOf('\n');
    const recovered = lastNewline === -1 ? '' : contents.slice(0, lastNewline + 1);
    writeFileSync(this.filePath, recovered);
  }
}
