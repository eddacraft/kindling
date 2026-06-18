/**
 * /memory forget command
 *
 * MIGRATION NOTE (PORT-019): this command redacted observations through the
 * in-process store's `redactObservation`. The thin
 * {@link import('@eddacraft/kindling').kindling} client / daemon exposes NO
 * redaction endpoint, so the command cannot be wired to the daemon. It is kept
 * as a clearly-marked stub (so the command surface and its formatter stay
 * stable) that always reports the operation as unsupported. Re-enable once the
 * daemon grows an observation-redaction endpoint.
 */

/**
 * Forget options
 */
export interface ForgetOptions {
  /** ID of observation to forget */
  observationId: string;
}

/**
 * Forget result
 */
export interface ForgetResult {
  /** Observation ID */
  observationId: string;
  /** Whether observation was redacted */
  redacted: boolean;
  /** Error if any */
  error?: string;
}

/**
 * Execute /memory forget command.
 *
 * STUB: the daemon does not expose a redaction endpoint, so this always returns
 * an unsupported result. See the module note.
 *
 * @param options - Forget options
 * @returns Forget result (always unsupported)
 */
export function memoryForget(options: ForgetOptions): ForgetResult {
  return {
    observationId: options.observationId,
    redacted: false,
    error: 'forget is not supported by the kindling daemon (no redaction endpoint)',
  };
}

/**
 * Format forget result as human-readable text
 *
 * @param result - Forget result
 * @returns Formatted forget result
 */
export function formatForgetResult(result: ForgetResult): string {
  if (result.error && !result.redacted) {
    return `❌ Failed to redact: ${result.error}`;
  }

  if (result.error && result.redacted) {
    return `⚠️  ${result.error}`;
  }

  return `🗑️  Redacted observation ${result.observationId}`;
}
