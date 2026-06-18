/**
 * /memory pin command
 *
 * Pins observations or summaries for persistent retrieval via the daemon-backed
 * {@link PinService} (the {@link import('@eddacraft/kindling').Kindling} thin
 * client). The daemon owns pin id minting and target validation, so this command
 * no longer constructs a `Pin` locally or checks target existence up front — a
 * missing target surfaces as the daemon's error.
 */

import type { PinTargetType, ScopeIds, Pin } from '@eddacraft/kindling';

/**
 * Pin service interface.
 *
 * Satisfied by the {@link import('@eddacraft/kindling').Kindling} thin client —
 * `memoryPin` only needs its `pin` method.
 */
export interface PinService {
  pin(args: {
    targetType: PinTargetType;
    targetId: string;
    note?: string;
    ttlMs?: number;
    scopeIds?: ScopeIds;
  }): Promise<Pin>;
}

/**
 * Pin options
 */
export interface PinOptions {
  /** Type of target to pin */
  targetType: PinTargetType;
  /** ID of target to pin */
  targetId: string;
  /** Optional reason for pinning (sent to the daemon as the pin note) */
  reason?: string;
  /** Scope for the pin */
  scopeIds?: ScopeIds;
  /** Optional time-to-live in milliseconds (relative) */
  ttlMs?: number;
}

/**
 * Pin result
 */
export interface PinResult {
  /** Pin ID */
  pinId: string;
  /** Target ID that was pinned */
  targetId: string;
  /** Target type */
  targetType: PinTargetType;
  /** Whether pin was created */
  created: boolean;
  /** Error if any */
  error?: string;
}

/**
 * Execute /memory pin command.
 *
 * @param service - Pin service (the daemon client)
 * @param options - Pin options
 * @returns Pin result
 */
export async function memoryPin(service: PinService, options: PinOptions): Promise<PinResult> {
  const { targetType, targetId, reason, scopeIds, ttlMs } = options;

  try {
    const pin = await service.pin({
      targetType,
      targetId,
      ...(reason !== undefined ? { note: reason } : {}),
      ...(ttlMs !== undefined ? { ttlMs } : {}),
      ...(scopeIds !== undefined ? { scopeIds } : {}),
    });

    return {
      pinId: pin.id,
      targetId,
      targetType,
      created: true,
    };
  } catch (err) {
    return {
      pinId: '',
      targetId,
      targetType,
      created: false,
      error: err instanceof Error ? err.message : 'Unknown error',
    };
  }
}

/**
 * Format pin result as human-readable text
 *
 * @param result - Pin result
 * @returns Formatted pin result
 */
export function formatPinResult(result: PinResult): string {
  if (!result.created) {
    return `❌ Failed to pin: ${result.error}`;
  }

  return `📌 Pinned ${result.targetType} ${result.targetId}`;
}
