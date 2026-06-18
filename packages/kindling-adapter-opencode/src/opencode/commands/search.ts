/**
 * /memory search command
 *
 * Searches memory and returns relevant context
 */

import type { RetrieveResult, ScopeIds } from '@eddacraft/kindling';

/**
 * Retrieval service interface.
 *
 * Satisfied by the {@link import('@eddacraft/kindling').Kindling} thin client —
 * `memorySearch` only needs its `retrieve` method.
 */
export interface RetrievalService {
  retrieve(options: {
    query: string;
    scopeIds: ScopeIds;
    maxCandidates?: number;
  }): Promise<RetrieveResult>;
}

/**
 * Search options
 */
export interface SearchOptions {
  /** Search query */
  query: string;
  /** Scope for search */
  scopeIds: ScopeIds;
  /** Maximum results to return */
  maxResults?: number;
}

/**
 * Execute /memory search command
 *
 * @param service - Retrieval service
 * @param options - Search options
 * @returns Retrieval result
 */
export async function memorySearch(
  service: RetrievalService,
  options: SearchOptions,
): Promise<RetrieveResult> {
  const { query, scopeIds, maxResults = 10 } = options;

  return service.retrieve({
    query,
    scopeIds,
    maxCandidates: maxResults,
  });
}

/**
 * Format search results as human-readable text
 *
 * @param result - Retrieval result
 * @returns Formatted search results
 */
export function formatSearchResults(result: RetrieveResult): string {
  const lines: string[] = [];

  lines.push('Search Results');
  lines.push('==============');
  lines.push('');
  lines.push(`Query: "${result.provenance.query}"`);
  lines.push(
    `Found: ${result.provenance.totalCandidates} results (showing ${result.provenance.returnedCandidates})`,
  );
  lines.push('');

  // Pinned items
  if (result.pins.length > 0) {
    lines.push('📌 Pinned:');
    lines.push('');

    for (const pinResult of result.pins) {
      const preview = truncateContent(pinResult.target.content, 200);
      lines.push(`  [${pinResult.target.id}]`);
      lines.push(`  ${preview}`);
      if (pinResult.pin.reason) {
        lines.push(`  Reason: ${pinResult.pin.reason}`);
      }
      lines.push('');
    }
  }

  // Current summary
  if (result.currentSummary) {
    lines.push('📝 Current Session Summary:');
    lines.push('');
    lines.push(`  ${result.currentSummary.content}`);
    lines.push(`  Confidence: ${(result.currentSummary.confidence * 100).toFixed(0)}%`);
    lines.push('');
  }

  // Candidates
  if (result.candidates.length > 0) {
    lines.push('🔍 Search Results:');
    lines.push('');

    for (const candidate of result.candidates) {
      const preview = truncateContent(candidate.entity.content, 200);
      const score = (candidate.score * 100).toFixed(0);

      lines.push(`  [${candidate.entity.id}] (score: ${score}%)`);
      lines.push(`  ${preview}`);

      if (candidate.matchContext) {
        lines.push(`  Match: ${candidate.matchContext}`);
      }

      lines.push('');
    }
  }

  if (result.pins.length === 0 && !result.currentSummary && result.candidates.length === 0) {
    lines.push('No results found.');
  }

  return lines.join('\n');
}

/**
 * Truncate content to max length
 *
 * @param content - Content to truncate
 * @param maxLength - Maximum length
 * @returns Truncated content
 */
function truncateContent(content: string, maxLength: number): string {
  if (content.length <= maxLength) {
    return content;
  }

  return content.substring(0, maxLength) + '...';
}
