/**
 * Retrieval tiering utilities
 *
 * Implements tiering rules for retrieval results:
 * - Tier 0 (non-evictable): Pins and current session summary
 * - Tier 1 (evictable): Provider candidates
 */

import type { PinResult, CandidateResult } from '../types/retrieval.js';
import type { Summary } from '../types/summary.js';

/**
 * Tiering levels
 */
export enum Tier {
  /** Non-evictable: pins and current summary */
  PINNED = 0,
  /** Evictable: provider candidates */
  CANDIDATE = 1,
}

/**
 * Result with tier assignment
 */
export interface TieredResult {
  tier: Tier;
  content: string;
  id: string;
}

/**
 * Assign tiers to retrieval results
 *
 * Tier 0 (non-evictable):
 * - Active pins
 * - Current session summary
 *
 * Tier 1 (evictable):
 * - Provider candidates
 *
 * @param pins - Pin results
 * @param currentSummary - Current session summary
 * @param candidates - Provider candidates
 * @returns Tiered results in priority order
 */
export function assignTiers(
  pins: PinResult[],
  currentSummary: Summary | undefined,
  candidates: CandidateResult[],
): TieredResult[] {
  const tiered: TieredResult[] = [];

  // Tier 0: Pins (non-evictable)
  for (const pin of pins) {
    tiered.push({
      tier: Tier.PINNED,
      content: pin.target.content,
      id: pin.target.id,
    });
  }

  // Tier 0: Current summary (non-evictable)
  if (currentSummary) {
    tiered.push({
      tier: Tier.PINNED,
      content: currentSummary.content,
      id: currentSummary.id,
    });
  }

  // Tier 1: Candidates (evictable, ordered by score)
  for (const candidate of candidates) {
    tiered.push({
      tier: Tier.CANDIDATE,
      content: candidate.entity.content,
      id: candidate.entity.id,
    });
  }

  return tiered;
}

/**
 * Filter tiered results by token budget
 *
 * Ensures tier 0 (non-evictable) items are always included.
 * Tier 1 items are included until budget exhausted.
 *
 * @param tiered - Tiered results
 * @param tokenBudget - Maximum tokens to include
 * @param estimateTokens - Function to estimate tokens for content
 * @returns Filtered results that fit within budget
 */
export function filterByTokenBudget(
  tiered: TieredResult[],
  tokenBudget: number,
  estimateTokens: (content: string) => number = estimateTokensSimple,
): TieredResult[] {
  const included: TieredResult[] = [];
  let tokensUsed = 0;

  for (const result of tiered) {
    const tokens = estimateTokens(result.content);

    // Always include tier 0 (pins and current summary)
    if (result.tier === Tier.PINNED) {
      included.push(result);
      tokensUsed += tokens;
      continue;
    }

    // Include tier 1 only if within budget
    if (tokensUsed + tokens <= tokenBudget) {
      included.push(result);
      tokensUsed += tokens;
    }
  }

  return included;
}

/**
 * Simple token estimation (characters / 4)
 *
 * Rough approximation: ~4 characters per token on average
 *
 * @param content - Content to estimate
 * @returns Estimated token count
 */
export function estimateTokensSimple(content: string): number {
  return Math.ceil(content.length / 4);
}

/**
 * Check if tier 0 results exceed token budget
 *
 * Returns true if non-evictable items (pins + current summary) exceed budget.
 * This is a warning condition - tier 0 items are always included.
 *
 * @param tiered - Tiered results
 * @param tokenBudget - Token budget
 * @param estimateTokens - Token estimation function
 * @returns True if tier 0 exceeds budget
 */
export function tier0ExceedsBudget(
  tiered: TieredResult[],
  tokenBudget: number,
  estimateTokens: (content: string) => number = estimateTokensSimple,
): boolean {
  let tier0Tokens = 0;

  for (const result of tiered) {
    if (result.tier === Tier.PINNED) {
      tier0Tokens += estimateTokens(result.content);
    }
  }

  return tier0Tokens > tokenBudget;
}
