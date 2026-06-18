/**
 * Typed errors thrown by the {@link Kindling} client.
 *
 * Mirrors the Rust `kindling_client::ClientError` variants so TS and Rust
 * consumers reason about the same failure modes:
 *   - {@link DaemonUnavailableError} ⇔ `ClientError::Unavailable`
 *   - {@link ApiError}               ⇔ `ClientError::Api`
 *   - {@link SchemaMismatchError}    ⇔ `ClientError::SchemaMismatch`
 */

/** Base class for all kindling client errors. */
export class KindlingError extends Error {
  constructor(message: string) {
    super(message);
    this.name = new.target.name;
  }
}

/**
 * The daemon could not be reached or spawned within the connect budget.
 *
 * Carries a human-readable explanation (e.g. the socket never appeared after a
 * spawn, or the binary could not be resolved). Points at the install docs.
 */
export class DaemonUnavailableError extends KindlingError {
  constructor(message: string) {
    super(
      `kindling daemon unavailable: ${message}. ` +
        'Install the kindling binary or set KINDLING_BIN. ' +
        'See https://github.com/eddacraft/kindling#installation',
    );
  }
}

/**
 * The daemon returned a non-2xx response. `message` is the daemon's
 * `{ "error": "<msg>" }` body when present, else the raw body or status phrase.
 */
export class ApiError extends KindlingError {
  /** HTTP status code. */
  readonly status: number;

  constructor(status: number, message: string) {
    super(`daemon returned ${status}: ${message}`);
    this.status = status;
  }
}

/**
 * The daemon's reported `schemaVersion` does not match the version this client
 * was built/configured to expect. Fail loud rather than risk silent contract
 * drift (mirrors the Rust client).
 */
export class SchemaMismatchError extends KindlingError {
  /** Schema version the client expects. */
  readonly expected: number;
  /** Schema version the daemon reports. */
  readonly actual: number;

  constructor(expected: number, actual: number) {
    super(`schema version mismatch: client expected ${expected}, daemon reports ${actual}`);
    this.expected = expected;
    this.actual = actual;
  }
}
