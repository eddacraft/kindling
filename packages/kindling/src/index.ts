/**
 * @eddacraft/kindling
 *
 * Thin TypeScript client for the kindling daemon — the Rust-canonical local
 * memory and continuity engine for AI-assisted development.
 *
 * This package no longer bundles an implementation. It speaks the daemon's v1
 * HTTP API over a Unix domain socket (`~/.kindling/kindling.sock`), auto-spawning
 * `kindling serve` on first use. It has NO native dependencies.
 *
 * @example
 * ```typescript
 * import { Kindling } from '@eddacraft/kindling';
 *
 * const kindling = new Kindling();
 * const capsule = await kindling.openCapsule({
 *   kind: 'session',
 *   intent: 'investigate bug',
 *   scopeIds: { sessionId: 's1' },
 * });
 * await kindling.appendObservation(
 *   { kind: 'message', content: 'hello', scopeIds: { sessionId: 's1' } },
 *   { capsuleId: capsule.id },
 * );
 * const result = await kindling.retrieve({ query: 'hello', scopeIds: { sessionId: 's1' } });
 * await kindling.closeCapsule(capsule.id);
 * ```
 */

// The client and its argument/result types.
export {
  Kindling,
  type Health,
  type OpenCapsuleArgs,
  type CloseCapsuleArgs,
  type AppendObservationArgs,
  type PinArgs,
} from './client.js';

// Configuration + resolution helpers.
export {
  EXPECTED_SCHEMA_VERSION,
  defaultSocketPath,
  resolveProjectRoot,
  resolveConfig,
  type KindlingOptions,
  type ResolvedConfig,
} from './config.js';

// Typed errors.
export { KindlingError, DaemonUnavailableError, ApiError, SchemaMismatchError } from './errors.js';

// Transport constant (project header name) for advanced consumers.
export { PROJECT_HEADER } from './transport.js';

// Generated domain types (Capsule, Observation, Pin, RetrieveResult, …),
// sourced from crates/kindling-types/bindings via scripts/sync-types.mjs.
export type * from './generated/index.js';
