/**
 * Intent capture health report (KINTENT-006)
 *
 * Silent capture failures are the failure mode that matters most for an
 * append-only intent log: if emitters stop firing, or the log is corrupted, or
 * a backlog of un-exported events piles up, nothing breaks loudly — the data
 * just quietly goes missing. This module turns that silence into a single,
 * inspectable report so an operator (or the `kindling intent status` command)
 * can see at a glance whether capture is working.
 *
 * The report is a pure derivation over an {@link IntentStore}: it reads the
 * live log (one ordered pass for counts/backlog/last-event, plus an
 * independent integrity recompute) and folds in two caller-supplied seams —
 * the current clock (for staleness) and the highest already-exported sequence
 * (for backlog). It performs no I/O of its own beyond what the store exposes
 * and is fully deterministic given those seams, so the Rust port can mirror it
 * field-for-field.
 */

import type { IntentEventType } from '../types/index.js';
import { INTENT_EVENT_TYPES, isIntentEventType } from '../types/index.js';
import type { IntentStore, IntentIntegrityError } from './store.js';

/** Summary of the most recent persisted event. */
export interface IntentStatusLastEvent {
  sequence: number;
  occurred_at: string;
  event_type: IntentEventType;
  event_id: string;
}

/** Outcome of recomputing the store's integrity chain. */
export interface IntentStatusIntegrity {
  ok: boolean;
  /** Present only when `ok` is false. */
  error?: IntentIntegrityError;
}

/**
 * A point-in-time health report for an intent capture log. Combines the four
 * signals KINTENT-006 requires — emitter health, last event timestamp,
 * backlog, and integrity state — plus a single rolled-up `healthy` verdict.
 */
export interface IntentStatusReport {
  /** Whether the log holds at least one event. A never-written log is the
   *  canonical silent-failure case, so it is reported as not healthy. */
  initialized: boolean;
  /** Total events in the log. */
  event_count: number;
  /** The most recent event, or `null` for an empty log. */
  last_event: IntentStatusLastEvent | null;
  /** `now - last_event.occurred_at` in milliseconds; `null` when there is no
   *  event or its `occurred_at` is not a valid RFC3339 timestamp. May be
   *  negative if the last event's clock is ahead of `now`. */
  last_event_age_ms: number | null;
  /** True when `last_event_age_ms` is strictly greater than `staleAfterMs`.
   *  Always false when no threshold is supplied or the log is empty. */
  stale: boolean;
  /** Integrity-chain verification result. */
  integrity: IntentStatusIntegrity;
  /** Events not yet exported: those with `sequence > exportedThrough`. With no
   *  watermark supplied, the whole log is considered backlog. */
  backlog: number;
  /** Event count keyed by type. Every known {@link IntentEventType} is present,
   *  defaulting to 0, so a type that has never fired reads as an explicit 0
   *  rather than a missing key. Events with an unrecognized type (only
   *  reachable via a hand-edited log) are excluded here but still counted in
   *  `event_count`/`backlog`, so these totals may sum to less than
   *  `event_count`. */
  counts_by_type: Record<IntentEventType, number>;
  /** Rolled-up verdict: initialized, integrity intact, and not stale. A
   *  non-zero `backlog` deliberately does NOT make a report unhealthy —
   *  `healthy` reflects whether capture is working, not whether downstream
   *  export is keeping up. */
  healthy: boolean;
}

export interface IntentStatusOptions {
  /** Clock seam for staleness/age (determinism/tests). Defaults to wall clock. */
  now?: () => string;
  /** If set, the log is `stale` once the last event is strictly older than this
   *  many milliseconds. Omit to disable the staleness check. */
  staleAfterMs?: number;
  /** Highest sequence already exported downstream. Events with a greater
   *  sequence count toward `backlog`. Omit to treat the whole log as backlog. */
  exportedThrough?: number;
}

/** The subset of {@link IntentStore} the report reads — kept minimal so the
 *  report can be exercised against a fake. `readAll` supplies count, last
 *  event, type tallies, and backlog from a single ordered pass; `verify`
 *  re-reads independently because integrity must be recomputed from disk. */
export type IntentStatusSource = Pick<IntentStore, 'verify' | 'readAll'>;

/** RFC3339 / ISO8601 instant, matching what the store writes via
 *  `Date.toISOString()` and what a Rust `chrono` port accepts. Anything else
 *  is treated as unparseable so both implementations agree on the `null`
 *  boundary rather than relying on `Date.parse`'s permissive grammar. */
const RFC3339_PATTERN = /^\d{4}-\d{2}-\d{2}[Tt]\d{2}:\d{2}:\d{2}(\.\d+)?([Zz]|[+-]\d{2}:\d{2})$/;

function zeroedCounts(): Record<IntentEventType, number> {
  const counts = {} as Record<IntentEventType, number>;
  for (const type of INTENT_EVENT_TYPES) {
    counts[type] = 0;
  }
  return counts;
}

/**
 * Derive a {@link IntentStatusReport} from a store. Pure given the `now` and
 * `exportedThrough` seams: it reads the store but introduces no other I/O or
 * clock dependency. Count, last event, type counts, and backlog all come from
 * a single `readAll()` pass so they cannot disagree with one another.
 */
export function computeIntentStatus(
  store: IntentStatusSource,
  options: IntentStatusOptions = {},
): IntentStatusReport {
  const now = options.now ?? (() => new Date().toISOString());
  const nowIso = now();

  const events = store.readAll();
  const eventCount = events.length;
  const initialized = eventCount > 0;
  const tail = events[eventCount - 1];

  const lastEvent: IntentStatusLastEvent | null = tail
    ? {
        sequence: tail.sequence,
        occurred_at: tail.occurred_at,
        event_type: tail.event_type,
        event_id: tail.event_id,
      }
    : null;

  const lastEventAgeMs = computeAgeMs(nowIso, lastEvent?.occurred_at);
  const stale =
    options.staleAfterMs !== undefined &&
    lastEventAgeMs !== null &&
    lastEventAgeMs > options.staleAfterMs;

  const verifyResult = store.verify();
  const integrity: IntentStatusIntegrity = verifyResult.ok
    ? { ok: true }
    : { ok: false, error: verifyResult.error };

  const counts = zeroedCounts();
  let backlog = 0;
  for (const event of events) {
    if (isIntentEventType(event.event_type)) {
      counts[event.event_type] += 1;
    }
    if (options.exportedThrough === undefined || event.sequence > options.exportedThrough) {
      backlog += 1;
    }
  }

  return {
    initialized,
    event_count: eventCount,
    last_event: lastEvent,
    last_event_age_ms: lastEventAgeMs,
    stale,
    integrity,
    backlog,
    counts_by_type: counts,
    healthy: initialized && integrity.ok && !stale,
  };
}

function computeAgeMs(nowIso: string, occurredAt: string | undefined): number | null {
  if (occurredAt === undefined || !RFC3339_PATTERN.test(occurredAt)) {
    return null;
  }
  const nowMs = Date.parse(nowIso);
  const thenMs = Date.parse(occurredAt);
  if (Number.isNaN(nowMs) || Number.isNaN(thenMs)) {
    return null;
  }
  return nowMs - thenMs;
}

/**
 * Render a report as a compact, human-readable block for the
 * `kindling intent status` command. Deterministic given its input.
 */
export function formatIntentStatus(report: IntentStatusReport): string {
  const lines: string[] = [];

  lines.push(`Capture: ${report.healthy ? 'healthy' : 'unhealthy'}`);
  lines.push(`Events: ${report.event_count}`);

  if (report.last_event === null) {
    lines.push('Last event: none (uninitialized — no events captured yet)');
  } else {
    const age = report.last_event_age_ms;
    const ageLabel = age === null ? 'unknown age' : `${formatDuration(age)} ago`;
    const staleLabel = report.stale ? ' [STALE]' : '';
    lines.push(
      `Last event: #${report.last_event.sequence} ${report.last_event.event_type} ` +
        `at ${report.last_event.occurred_at} (${ageLabel})${staleLabel}`,
    );
  }

  lines.push(`Backlog: ${report.backlog} un-exported`);

  if (report.integrity.ok) {
    lines.push('Integrity: ok');
  } else {
    const detail = report.integrity.error
      ? `${report.integrity.error.kind} — ${report.integrity.error.message}`
      : 'unknown error';
    lines.push(`Integrity: BROKEN (${detail})`);
  }

  lines.push('By type:');
  for (const type of INTENT_EVENT_TYPES) {
    lines.push(`  ${type}: ${report.counts_by_type[type]}`);
  }

  return lines.join('\n');
}

function formatDuration(ms: number): string {
  if (ms < 0) {
    return `-${formatDuration(-ms)}`;
  }
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return `${minutes}m`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours}h`;
  }
  const days = Math.floor(hours / 24);
  return `${days}d`;
}
