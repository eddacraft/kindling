/**
 * Tests for retrieval orchestration
 */

import { describe, it, expect, beforeEach } from 'vitest';
import type { Pin, Observation, Summary, ID } from '../src/types/index.js';
import type { RetrievalStore } from '../src/retrieval/orchestrator.js';
import type {
  RetrievalProvider,
  ProviderSearchOptions,
  ProviderSearchResult,
} from '../src/types/retrieval.js';
import { retrieve } from '../src/retrieval/orchestrator.js';
import {
  assignTiers,
  filterByTokenBudget,
  tier0ExceedsBudget,
  Tier,
} from '../src/retrieval/tiering.js';

/**
 * Mock store for testing
 */
class MockStore implements RetrievalStore {
  pins: Pin[] = [];
  observations: Map<ID, Observation> = new Map();
  summaries: Map<ID, Summary> = new Map();
  openCapsules: Map<string, { id: ID; summaryId?: ID }> = new Map();
  capsuleSummaries: Map<ID, Summary> = new Map();

  listActivePins(scopeIds?: Partial<Record<string, string>>, now?: number): Pin[] {
    return this.pins.filter((pin) => {
      // Check expiry
      if (pin.expiresAt && now && pin.expiresAt <= now) {
        return false;
      }

      // Check scope match
      if (scopeIds?.sessionId && pin.scopeIds.sessionId !== scopeIds.sessionId) {
        return false;
      }

      return true;
    });
  }

  getObservationById(observationId: ID): Observation | undefined {
    return this.observations.get(observationId);
  }

  getSummaryById(summaryId: ID): Summary | undefined {
    return this.summaries.get(summaryId);
  }

  getOpenCapsuleForSession(sessionId: string): { id: ID; summaryId?: ID } | undefined {
    return this.openCapsules.get(sessionId);
  }

  getLatestSummaryForCapsule(capsuleId: ID): Summary | undefined {
    return this.capsuleSummaries.get(capsuleId);
  }
}

/**
 * Mock provider for testing
 */
class MockProvider implements RetrievalProvider {
  name = 'mock-provider';
  results: ProviderSearchResult[] = [];

  async search(options: ProviderSearchOptions): Promise<ProviderSearchResult[]> {
    // Filter by excludeIds
    let filtered = this.results;
    if (options.excludeIds && options.excludeIds.length > 0) {
      const excludeSet = new Set(options.excludeIds);
      filtered = this.results.filter((r) => !excludeSet.has(r.entity.id));
    }

    // Limit results
    if (options.maxResults) {
      filtered = filtered.slice(0, options.maxResults);
    }

    return filtered;
  }
}

describe('Retrieval Orchestration', () => {
  let store: MockStore;
  let provider: MockProvider;

  beforeEach(() => {
    store = new MockStore();
    provider = new MockProvider();
  });

  describe('retrieve', () => {
    it('should return empty result when no data exists', async () => {
      const result = await retrieve(store, provider, {
        query: 'test query',
        scopeIds: { sessionId: 's1' },
      });

      expect(result.pins).toHaveLength(0);
      expect(result.currentSummary).toBeUndefined();
      expect(result.candidates).toHaveLength(0);
      expect(result.provenance.query).toBe('test query');
      expect(result.provenance.providerUsed).toBe('mock-provider');
    });

    it('should include active pins with resolved targets', async () => {
      const obs: Observation = {
        id: 'obs-1',
        kind: 'message',
        content: 'Pinned message',
        provenance: {},
        ts: 1000,
        scopeIds: { sessionId: 's1' },
        redacted: false,
      };

      store.observations.set('obs-1', obs);

      const pin: Pin = {
        id: 'pin-1',
        targetType: 'observation',
        targetId: 'obs-1',
        createdAt: 1000,
        scopeIds: { sessionId: 's1' },
      };

      store.pins.push(pin);

      const result = await retrieve(store, provider, {
        query: 'test',
        scopeIds: { sessionId: 's1' },
      });

      expect(result.pins).toHaveLength(1);
      expect(result.pins[0].pin.id).toBe('pin-1');
      expect(result.pins[0].target.id).toBe('obs-1');
    });

    it('should exclude expired pins', async () => {
      const obs: Observation = {
        id: 'obs-1',
        kind: 'message',
        content: 'Expired pin',
        provenance: {},
        ts: 1000,
        scopeIds: { sessionId: 's1' },
        redacted: false,
      };

      store.observations.set('obs-1', obs);

      const pin: Pin = {
        id: 'pin-1',
        targetType: 'observation',
        targetId: 'obs-1',
        createdAt: 1000,
        expiresAt: 2000,
        scopeIds: { sessionId: 's1' },
      };

      store.pins.push(pin);

      const result = await retrieve(store, provider, {
        query: 'test',
        scopeIds: { sessionId: 's1' },
      });

      expect(result.pins).toHaveLength(0);
    });

    it('should exclude redacted pins by default', async () => {
      const obs: Observation = {
        id: 'obs-1',
        kind: 'message',
        content: '[redacted]',
        provenance: {},
        ts: 1000,
        scopeIds: { sessionId: 's1' },
        redacted: true,
      };

      store.observations.set('obs-1', obs);

      const pin: Pin = {
        id: 'pin-1',
        targetType: 'observation',
        targetId: 'obs-1',
        createdAt: 1000,
        scopeIds: { sessionId: 's1' },
      };

      store.pins.push(pin);

      const result = await retrieve(store, provider, {
        query: 'test',
        scopeIds: { sessionId: 's1' },
      });

      expect(result.pins).toHaveLength(0);
    });

    it('should include redacted pins when requested', async () => {
      const obs: Observation = {
        id: 'obs-1',
        kind: 'message',
        content: '[redacted]',
        provenance: {},
        ts: 1000,
        scopeIds: { sessionId: 's1' },
        redacted: true,
      };

      store.observations.set('obs-1', obs);

      const pin: Pin = {
        id: 'pin-1',
        targetType: 'observation',
        targetId: 'obs-1',
        createdAt: 1000,
        scopeIds: { sessionId: 's1' },
      };

      store.pins.push(pin);

      const result = await retrieve(store, provider, {
        query: 'test',
        scopeIds: { sessionId: 's1' },
        includeRedacted: true,
      });

      expect(result.pins).toHaveLength(1);
    });

    it('should include current session summary', async () => {
      const summary: Summary = {
        id: 'sum-1',
        capsuleId: 'cap-1',
        content: 'Current session summary',
        confidence: 0.9,
        createdAt: 1000,
        evidenceRefs: [],
      };

      store.capsuleSummaries.set('cap-1', summary);
      store.openCapsules.set('s1', { id: 'cap-1' });

      const result = await retrieve(store, provider, {
        query: 'test',
        scopeIds: { sessionId: 's1' },
      });

      expect(result.currentSummary).toBeDefined();
      expect(result.currentSummary!.id).toBe('sum-1');
    });

    it('should include provider candidates', async () => {
      const obs: Observation = {
        id: 'obs-1',
        kind: 'message',
        content: 'Provider result',
        provenance: {},
        ts: 1000,
        scopeIds: { sessionId: 's1' },
        redacted: false,
      };

      provider.results = [
        {
          entity: obs,
          score: 0.95,
          matchContext: 'exact match',
        },
      ];

      const result = await retrieve(store, provider, {
        query: 'test',
        scopeIds: { sessionId: 's1' },
      });

      expect(result.candidates).toHaveLength(1);
      expect(result.candidates[0].entity.id).toBe('obs-1');
      expect(result.candidates[0].score).toBe(0.95);
    });

    it('should exclude pinned IDs from candidates', async () => {
      const obs1: Observation = {
        id: 'obs-1',
        kind: 'message',
        content: 'Pinned',
        provenance: {},
        ts: 1000,
        scopeIds: { sessionId: 's1' },
        redacted: false,
      };

      const obs2: Observation = {
        id: 'obs-2',
        kind: 'message',
        content: 'Not pinned',
        provenance: {},
        ts: 2000,
        scopeIds: { sessionId: 's1' },
        redacted: false,
      };

      store.observations.set('obs-1', obs1);

      const pin: Pin = {
        id: 'pin-1',
        targetType: 'observation',
        targetId: 'obs-1',
        createdAt: 1000,
        scopeIds: { sessionId: 's1' },
      };

      store.pins.push(pin);

      provider.results = [
        { entity: obs1, score: 0.95, matchContext: 'match 1' },
        { entity: obs2, score: 0.9, matchContext: 'match 2' },
      ];

      const result = await retrieve(store, provider, {
        query: 'test',
        scopeIds: { sessionId: 's1' },
      });

      // obs-1 should be in pins, not candidates
      expect(result.pins).toHaveLength(1);
      expect(result.candidates).toHaveLength(1);
      expect(result.candidates[0].entity.id).toBe('obs-2');
    });

    it('should exclude current summary from candidates', async () => {
      const summary: Summary = {
        id: 'sum-1',
        capsuleId: 'cap-1',
        content: 'Current summary',
        confidence: 0.9,
        createdAt: 1000,
        evidenceRefs: [],
      };

      store.capsuleSummaries.set('cap-1', summary);
      store.openCapsules.set('s1', { id: 'cap-1' });

      provider.results = [{ entity: summary, score: 0.95, matchContext: 'match' }];

      const result = await retrieve(store, provider, {
        query: 'test',
        scopeIds: { sessionId: 's1' },
      });

      // Summary should be in currentSummary, not candidates
      expect(result.currentSummary).toBeDefined();
      expect(result.candidates).toHaveLength(0);
    });

    it('should respect maxCandidates limit', async () => {
      const obs = (id: number): Observation => ({
        id: `obs-${id}`,
        kind: 'message',
        content: `Message ${id}`,
        provenance: {},
        ts: id,
        scopeIds: { sessionId: 's1' },
        redacted: false,
      });

      provider.results = [
        { entity: obs(1), score: 0.9 },
        { entity: obs(2), score: 0.8 },
        { entity: obs(3), score: 0.7 },
        { entity: obs(4), score: 0.6 },
        { entity: obs(5), score: 0.5 },
      ];

      const result = await retrieve(store, provider, {
        query: 'test',
        scopeIds: { sessionId: 's1' },
        maxCandidates: 3,
      });

      expect(result.candidates).toHaveLength(3);
      expect(result.provenance.totalCandidates).toBe(3);
    });
  });

  describe('assignTiers', () => {
    it('should assign tier 0 to pins', () => {
      const obs: Observation = {
        id: 'obs-1',
        kind: 'message',
        content: 'Pinned',
        provenance: {},
        ts: 1000,
        scopeIds: { sessionId: 's1' },
        redacted: false,
      };

      const pin: Pin = {
        id: 'pin-1',
        targetType: 'observation',
        targetId: 'obs-1',
        createdAt: 1000,
        scopeIds: { sessionId: 's1' },
      };

      const tiered = assignTiers([{ pin, target: obs }], undefined, []);

      expect(tiered).toHaveLength(1);
      expect(tiered[0].tier).toBe(Tier.PINNED);
      expect(tiered[0].id).toBe('obs-1');
    });

    it('should assign tier 0 to current summary', () => {
      const summary: Summary = {
        id: 'sum-1',
        capsuleId: 'cap-1',
        content: 'Current summary',
        confidence: 0.9,
        createdAt: 1000,
        evidenceRefs: [],
      };

      const tiered = assignTiers([], summary, []);

      expect(tiered).toHaveLength(1);
      expect(tiered[0].tier).toBe(Tier.PINNED);
      expect(tiered[0].id).toBe('sum-1');
    });

    it('should assign tier 1 to candidates', () => {
      const obs: Observation = {
        id: 'obs-1',
        kind: 'message',
        content: 'Candidate',
        provenance: {},
        ts: 1000,
        scopeIds: { sessionId: 's1' },
        redacted: false,
      };

      const tiered = assignTiers([], undefined, [{ entity: obs, score: 0.9 }]);

      expect(tiered).toHaveLength(1);
      expect(tiered[0].tier).toBe(Tier.CANDIDATE);
    });

    it('should order by tier: pins, summary, then candidates', () => {
      const obs1: Observation = {
        id: 'obs-1',
        kind: 'message',
        content: 'Pinned',
        provenance: {},
        ts: 1000,
        scopeIds: { sessionId: 's1' },
        redacted: false,
      };

      const obs2: Observation = {
        id: 'obs-2',
        kind: 'message',
        content: 'Candidate',
        provenance: {},
        ts: 2000,
        scopeIds: { sessionId: 's1' },
        redacted: false,
      };

      const summary: Summary = {
        id: 'sum-1',
        capsuleId: 'cap-1',
        content: 'Summary',
        confidence: 0.9,
        createdAt: 1500,
        evidenceRefs: [],
      };

      const pin: Pin = {
        id: 'pin-1',
        targetType: 'observation',
        targetId: 'obs-1',
        createdAt: 1000,
        scopeIds: { sessionId: 's1' },
      };

      const tiered = assignTiers([{ pin, target: obs1 }], summary, [{ entity: obs2, score: 0.8 }]);

      expect(tiered).toHaveLength(3);
      expect(tiered[0].id).toBe('obs-1'); // Pin
      expect(tiered[1].id).toBe('sum-1'); // Summary
      expect(tiered[2].id).toBe('obs-2'); // Candidate
    });
  });

  describe('filterByTokenBudget', () => {
    it('should always include tier 0 items', () => {
      const tiered = [
        { tier: Tier.PINNED, content: 'x'.repeat(100), id: '1' },
        { tier: Tier.CANDIDATE, content: 'x'.repeat(100), id: '2' },
      ];

      const filtered = filterByTokenBudget(tiered, 10); // Very small budget

      // Tier 0 always included
      expect(filtered.length).toBeGreaterThan(0);
      expect(filtered[0].tier).toBe(Tier.PINNED);
    });

    it('should exclude tier 1 items when budget exhausted', () => {
      const tiered = [
        { tier: Tier.PINNED, content: 'x'.repeat(40), id: '1' }, // ~10 tokens
        { tier: Tier.CANDIDATE, content: 'x'.repeat(400), id: '2' }, // ~100 tokens
      ];

      const filtered = filterByTokenBudget(tiered, 50);

      expect(filtered).toHaveLength(1);
      expect(filtered[0].tier).toBe(Tier.PINNED);
    });

    it('should include tier 1 items within budget', () => {
      const tiered = [
        { tier: Tier.PINNED, content: 'x'.repeat(40), id: '1' },
        { tier: Tier.CANDIDATE, content: 'x'.repeat(40), id: '2' },
      ];

      const filtered = filterByTokenBudget(tiered, 100);

      expect(filtered).toHaveLength(2);
    });
  });

  describe('tier0ExceedsBudget', () => {
    it('should return true when tier 0 exceeds budget', () => {
      const tiered = [
        { tier: Tier.PINNED, content: 'x'.repeat(400), id: '1' }, // ~100 tokens
      ];

      const exceeds = tier0ExceedsBudget(tiered, 50);
      expect(exceeds).toBe(true);
    });

    it('should return false when tier 0 within budget', () => {
      const tiered = [
        { tier: Tier.PINNED, content: 'x'.repeat(40), id: '1' }, // ~10 tokens
      ];

      const exceeds = tier0ExceedsBudget(tiered, 50);
      expect(exceeds).toBe(false);
    });
  });
});
