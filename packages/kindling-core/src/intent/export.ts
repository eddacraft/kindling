/**
 * Signed intent export bundle (KINTENT-005)
 *
 * Seals a sequence range of persisted intent events into a portable, signed
 * bundle for downstream ingestion (Anvil). Events handed here are already in
 * their persisted form — the redaction boundary (KINTENT-004) runs at the store
 * write path, so secrets never reach disk and therefore never reach an export.
 *
 * ## Why this layer signs
 *
 * The append-only store's integrity chain is an *un-keyed* SHA-256 hash chain:
 * tamper-**evident** but not authenticated, since anyone with the public
 * algorithm can rewrite events and recompute the chain. The store deliberately
 * defers the keyed/authenticated layer to export. This module provides it: an
 * **HMAC-SHA256** over a canonical manifest, using a workspace-local key. A
 * consumer that shares the key can confirm both that the bundle is intact and
 * that it was produced by a holder of the key.
 *
 * ## What is signed
 *
 * The signature covers the canonical manifest *core* (every manifest field
 * except the signature itself). The core includes `bundle_hash` — the SHA-256
 * of the canonical JSONL body — so signing the core transitively binds the
 * bundle body, the exported sequence range, the event count, and the chain tip.
 * Tampering with any of them invalidates the signature.
 *
 * ## Bundle body
 *
 * The body is canonical JSONL: one event per line, ascending by sequence, each
 * line serialized with the shared {@link canonicalize} contract (sorted keys,
 * omitted `undefined`). This makes `bundle_hash` reproducible across
 * implementations regardless of in-memory key order — the Rust port must emit
 * byte-identical lines.
 */

import { createHash, createHmac, timingSafeEqual } from 'crypto';
import type { IntentEvent, Result } from '../types/index.js';
import { ok, err } from '../types/index.js';
import { canonicalize } from './canonical.js';

/** Bundle format version, stamped into the manifest. */
export const INTENT_EXPORT_BUNDLE_VERSION = '1.0' as const;

/** The only signature algorithm this version emits and accepts. */
export const INTENT_EXPORT_SIGNATURE_ALG = 'HMAC-SHA256' as const;

export type IntentExportBundleVersion = typeof INTENT_EXPORT_BUNDLE_VERSION;
export type IntentExportSignatureAlg = typeof INTENT_EXPORT_SIGNATURE_ALG;

/** Authentication tag over the canonical manifest core. */
export interface IntentExportSignature {
  alg: IntentExportSignatureAlg;
  /** Identifies which workspace key signed the bundle. */
  key_id?: string;
  /** Lowercase hex HMAC-SHA256 digest. */
  value: string;
}

/** Inclusive `[lo, hi]` sequence range covered by a bundle. */
export type IntentExportSequenceRange = [number, number];

export interface IntentExportManifest {
  bundle_version: IntentExportBundleVersion;
  /** ISO8601 timestamp the bundle was created. */
  created_at: string;
  /** Number of events in the body. */
  event_count: number;
  /** Inclusive sequence range, or `null` for an empty bundle. */
  sequence_range: IntentExportSequenceRange | null;
  /**
   * `integrity_hash` of the highest-sequence event in the bundle (the chain
   * tip at export time), or `null` for an empty bundle. Lets a consumer anchor
   * the bundle against the source chain.
   */
  tip_integrity_hash: string | null;
  /** SHA-256 hex of the canonical JSONL body. */
  bundle_hash: string;
  signature: IntentExportSignature;
}

export interface IntentExportBundle {
  manifest: IntentExportManifest;
  /**
   * Canonical JSONL body: one event per line, ascending by sequence. Empty
   * string for an empty bundle. Lines are joined (not trailing-terminated) so
   * `bundle_hash` is the SHA-256 of exactly these bytes.
   */
  jsonl: string;
}

export interface CreateIntentExportOptions {
  /** Workspace HMAC key. Required — every bundle is signed. */
  key: string | Buffer;
  /** Recorded in `signature.key_id` to identify the signing key. */
  keyId?: string;
  /** Inclusive lower bound on `sequence` (events below are excluded). */
  fromSequence?: number;
  /** Inclusive upper bound on `sequence` (events above are excluded). */
  toSequence?: number;
  /** Clock seam for `created_at` (determinism/tests). */
  now?: () => string;
}

export type IntentExportErrorKind =
  | 'bundle_hash_mismatch'
  | 'signature_mismatch'
  | 'manifest_mismatch'
  | 'unsupported_alg'
  | 'parse_error';

export interface IntentExportError {
  kind: IntentExportErrorKind;
  message: string;
}

/** Manifest minus the signature — the exact value the HMAC is computed over. */
type ManifestCore = Omit<IntentExportManifest, 'signature'>;

function sha256Hex(input: string): string {
  return createHash('sha256').update(input).digest('hex');
}

function hmacHex(key: string | Buffer, input: string): string {
  return createHmac('sha256', key).update(input).digest('hex');
}

/** Constant-time hex digest comparison (guards against length leaks too). */
function digestsEqual(a: string, b: string): boolean {
  if (a.length !== b.length) {
    return false;
  }
  const bufA = Buffer.from(a, 'utf8');
  const bufB = Buffer.from(b, 'utf8');
  // Lengths are equal here, so timingSafeEqual is safe to call.
  return timingSafeEqual(bufA, bufB);
}

function bodyFor(events: IntentEvent[]): string {
  return events.map((event) => canonicalize(event)).join('\n');
}

/**
 * Build a signed export bundle from `events`. Events are filtered to the
 * inclusive `[fromSequence, toSequence]` range and sorted ascending by
 * sequence; the resulting body, range, tip, and count are sealed under an
 * HMAC-SHA256 signature over the canonical manifest core.
 */
export function createIntentExport(
  events: IntentEvent[],
  options: CreateIntentExportOptions,
): IntentExportBundle {
  const { key, keyId, fromSequence, toSequence } = options;
  if (key === undefined || key === null || (typeof key === 'string' && key.length === 0)) {
    throw new Error('createIntentExport: a non-empty signing key is required');
  }
  const now = options.now ?? (() => new Date().toISOString());

  const selected = events
    .filter((event) => {
      if (fromSequence !== undefined && event.sequence < fromSequence) {
        return false;
      }
      if (toSequence !== undefined && event.sequence > toSequence) {
        return false;
      }
      return true;
    })
    .sort((a, b) => a.sequence - b.sequence);

  const jsonl = bodyFor(selected);
  const bundleHash = sha256Hex(jsonl);

  const first = selected[0];
  const last = selected[selected.length - 1];
  const sequenceRange: IntentExportSequenceRange | null = last
    ? [first.sequence, last.sequence]
    : null;
  const tipIntegrityHash = last ? last.provenance.integrity_hash : null;

  const core: ManifestCore = {
    bundle_version: INTENT_EXPORT_BUNDLE_VERSION,
    created_at: now(),
    event_count: selected.length,
    sequence_range: sequenceRange,
    tip_integrity_hash: tipIntegrityHash,
    bundle_hash: bundleHash,
  };

  const signature: IntentExportSignature = {
    alg: INTENT_EXPORT_SIGNATURE_ALG,
    key_id: keyId,
    value: hmacHex(key, canonicalize(core)),
  };

  return { manifest: { ...core, signature }, jsonl };
}

/**
 * Verify a bundle against `key`. Confirms, in order: the signature algorithm is
 * supported, the body hashes to the recorded `bundle_hash`, the manifest's
 * count/range/tip agree with the body, and the HMAC over the manifest core
 * matches under `key`. Returns `ok(true)` only when all hold.
 */
export function verifyIntentExport(
  bundle: IntentExportBundle,
  key: string | Buffer,
): Result<true, IntentExportError> {
  const { manifest, jsonl } = bundle;

  if (manifest.signature.alg !== INTENT_EXPORT_SIGNATURE_ALG) {
    return err({
      kind: 'unsupported_alg',
      message: `Unsupported signature algorithm: ${String(manifest.signature.alg)}`,
    });
  }

  const recomputedHash = sha256Hex(jsonl);
  if (!digestsEqual(recomputedHash, manifest.bundle_hash)) {
    return err({
      kind: 'bundle_hash_mismatch',
      message: 'Bundle body does not match the recorded bundle_hash',
    });
  }

  // The manifest metadata is signed, but cross-check it against the body so a
  // mismatch surfaces as a precise manifest error rather than only as a
  // signature failure.
  let parsed: IntentEvent[];
  try {
    parsed =
      jsonl.length === 0 ? [] : jsonl.split('\n').map((line) => JSON.parse(line) as IntentEvent);
  } catch (cause) {
    return err({
      kind: 'parse_error',
      message: `Failed to parse bundle body: ${(cause as Error).message}`,
    });
  }

  const manifestError = checkManifestAgainstBody(manifest, parsed);
  if (manifestError) {
    return err(manifestError);
  }

  const core: ManifestCore = {
    bundle_version: manifest.bundle_version,
    created_at: manifest.created_at,
    event_count: manifest.event_count,
    sequence_range: manifest.sequence_range,
    tip_integrity_hash: manifest.tip_integrity_hash,
    bundle_hash: manifest.bundle_hash,
  };
  const expected = hmacHex(key, canonicalize(core));
  if (!digestsEqual(expected, manifest.signature.value)) {
    return err({
      kind: 'signature_mismatch',
      message: 'Signature does not match — wrong key or tampered manifest',
    });
  }

  return ok(true);
}

function checkManifestAgainstBody(
  manifest: IntentExportManifest,
  events: IntentEvent[],
): IntentExportError | undefined {
  if (manifest.event_count !== events.length) {
    return {
      kind: 'manifest_mismatch',
      message: `event_count ${manifest.event_count} != body length ${events.length}`,
    };
  }
  if (events.length === 0) {
    if (manifest.sequence_range !== null || manifest.tip_integrity_hash !== null) {
      return {
        kind: 'manifest_mismatch',
        message: 'Empty bundle must have null sequence_range and tip_integrity_hash',
      };
    }
    return undefined;
  }
  const last = events[events.length - 1];
  const expectedRange: IntentExportSequenceRange = [events[0].sequence, last.sequence];
  if (
    manifest.sequence_range === null ||
    manifest.sequence_range[0] !== expectedRange[0] ||
    manifest.sequence_range[1] !== expectedRange[1]
  ) {
    return {
      kind: 'manifest_mismatch',
      message: 'sequence_range does not match the bundle body',
    };
  }
  if (manifest.tip_integrity_hash !== last.provenance.integrity_hash) {
    return {
      kind: 'manifest_mismatch',
      message: 'tip_integrity_hash does not match the last event in the body',
    };
  }
  return undefined;
}

/** Serialize a bundle to a single self-contained JSON string for transfer. */
export function serializeIntentExport(bundle: IntentExportBundle): string {
  return JSON.stringify(bundle);
}

/** Parse a bundle produced by {@link serializeIntentExport}. */
export function parseIntentExport(text: string): IntentExportBundle {
  return JSON.parse(text) as IntentExportBundle;
}
