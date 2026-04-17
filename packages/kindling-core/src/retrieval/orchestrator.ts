/**
 * Retrieval orchestrator
 *
 * Combines pins, summaries, and provider candidates into a unified retrieval response.
 */

import type {
  RetrieveOptions,
  RetrieveResult,
  PinResult,
  CandidateResult,
  RetrievalProvider,
} from '../types/retrieval.js';
import type { Pin, Observation, Summary, ID } from '../types/index.js';

/**
 * Store interface for retrieval operations
 */
export interface RetrievalStore {
  /**
   * List active pins for a scope
   */
  listActivePins(scopeIds?: Partial<Record<string, string>>, now?: number): Pin[];

  /**
   * Get observation by ID
   */
  getObservationById(observationId: ID): Observation | undefined;

  /**
   * Get summary by ID
   */
  getSummaryById(summaryId: ID): Summary | undefined;

  /**
   * Get open capsule for session
   */
  getOpenCapsuleForSession(sessionId: string): { id: ID; summaryId?: ID } | undefined;

  /**
   * Get latest summary for capsule
   */
  getLatestSummaryForCapsule(capsuleId: ID): Summary | undefined;
}

/**
 * Retrieve relevant context for a query
 *
 * Orchestrates retrieval by combining:
 * 1. Active pins (non-evictable, always included)
 * 2. Current session summary (non-evictable if exists)
 * 3. Provider candidates (ranked, budget-limited)
 *
 * @param store - Retrieval store
 * @param provider - Retrieval provider
 * @param options - Retrieval options
 * @returns Retrieval result with pins, summary, and candidates
 */
export async function retrieve(
  store: RetrievalStore,
  provider: RetrievalProvider,
  options: RetrieveOptions,
): Promise<RetrieveResult> {
  const { query, scopeIds, maxCandidates = 10, includeRedacted = false } = options;

  const now = Date.now();

  // Step 1: Fetch active pins for scope
  const pins = store.listActivePins(scopeIds as Partial<Record<string, string>>, now);

  // Step 2: Resolve pins to their targets
  const pinResults: PinResult[] = [];
  const pinnedIds = new Set<ID>();

  for (const pin of pins) {
    let target: Observation | Summary | undefined;

    if (pin.targetType === 'observation') {
      target = store.getObservationById(pin.targetId);
    } else if (pin.targetType === 'summary') {
      target = store.getSummaryById(pin.targetId);
    }

    if (target) {
      // Skip redacted unless explicitly requested
      if ('redacted' in target && target.redacted && !includeRedacted) {
        continue;
      }

      pinResults.push({ pin, target });
      pinnedIds.add(target.id);
    }
  }

  // Step 3: Get current session summary (non-evictable)
  let currentSummary: Summary | undefined;

  if (scopeIds.sessionId) {
    const capsule = store.getOpenCapsuleForSession(scopeIds.sessionId);
    if (capsule) {
      const summary = store.getLatestSummaryForCapsule(capsule.id);
      if (summary) {
        currentSummary = summary;
        pinnedIds.add(summary.id);
      }
    }
  }

  // Step 4: Get provider candidates (exclude pinned and current summary)
  const providerResults = await provider.search({
    query,
    scopeIds,
    maxResults: maxCandidates,
    excludeIds: Array.from(pinnedIds),
    includeRedacted,
  });

  // Step 5: Convert provider results to candidates
  const candidates: CandidateResult[] = providerResults.map((result) => ({
    entity: result.entity,
    score: result.score,
    matchContext: result.matchContext,
  }));

  // Step 6: Build provenance
  const provenance = {
    query,
    scopeIds,
    totalCandidates: providerResults.length,
    returnedCandidates: candidates.length,
    truncatedDueToTokenBudget: false, // Token budgeting not implemented in v0.1
    providerUsed: provider.name,
  };

  return {
    pins: pinResults,
    currentSummary,
    candidates,
    provenance,
  };
}
