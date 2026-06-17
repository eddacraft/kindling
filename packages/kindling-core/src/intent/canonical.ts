/**
 * Deterministic canonical serialization for intent records.
 *
 * This is the single source of truth for the byte-for-byte serialization used
 * by both the append-only store's integrity hash chain (KINTENT-003) and the
 * signed export bundle (KINTENT-005). Any other implementation that recomputes
 * these hashes (e.g. the Rust port) must match this contract exactly.
 *
 * Canonicalization contract:
 * - Object keys are sorted ascending (UTF-16 code-unit order, JS default).
 * - Keys whose value is `undefined` are omitted entirely. An optional field
 *   that is absent must serialize identically to one set to `undefined` — it
 *   must NOT be serialized as `null`. (Rust `Option::None` must serialize as an
 *   omitted key, i.e. `skip_serializing_if = "Option::is_none"`.)
 * - Arrays preserve element order (it is semantically meaningful), and an empty
 *   array `[]` is "present but empty" — it serializes differently from an
 *   absent/`undefined` field. Callers must not coerce `[]` to `undefined`.
 * - Strings/numbers/booleans/`null` use `JSON.stringify`.
 */
export function canonicalize(value: unknown): string {
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
