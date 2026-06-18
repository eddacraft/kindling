# Retrieval Contract

## Overview

kindling retrieval is **deterministic**, **scoped**, and **explainable**. The same query with the same context always produces the same results. All results include provenance, explaining why they were returned.

Retrieval combines three sources in a tiered structure:

1. **Pins** — User-marked important items (non-evictable)
2. **Current Summary** — Summary of the open capsule (non-evictable)
3. **Provider Candidates** — FTS-ranked observations and summaries (evictable)

## Core Interface

### Retrieve Function

```typescript
interface RetrieveOptions {
  query: string; // Search query
  scopeIds: ScopeIds; // Isolation dimensions
  tokenBudget?: number; // Max tokens in response (for truncation)
  maxCandidates?: number; // Max candidates from provider (default: 50)
  includeRedacted?: boolean; // Include redacted observations (default: false)
}

interface RetrieveResult {
  pins: PinResult[]; // Active pins
  currentSummary?: Summary; // Summary for open capsule (if any)
  candidates: CandidateResult[]; // Provider results
  provenance: RetrieveProvenance; // Explain how results were generated
}

interface PinResult {
  pin: Pin; // The pin itself
  target: Observation | Summary; // The pinned entity
}

interface CandidateResult {
  entity: Observation | Summary; // The matched entity
  score: number; // Relevance score (0.0-1.0)
  matchContext?: string; // Snippet showing match
}

interface RetrieveProvenance {
  query: string; // Original query
  scopeIds: ScopeIds; // Applied scope filters
  totalCandidates: number; // Total candidates before truncation
  returnedCandidates: number; // Actual candidates returned
  truncatedDueToTokenBudget: boolean; // Whether truncation occurred
  providerUsed: string; // Provider name (e.g., "local-fts")
}
```

### Example Usage

```typescript
const result = await kindling.retrieve({
  query: 'authentication bug',
  scopeIds: { repoId: '/repo', sessionId: 's1' },
  tokenBudget: 8000
});

// Result structure:
{
  pins: [
    {
      pin: { id: 'pin1', targetType: 'observation', targetId: 'obs1', ... },
      target: { id: 'obs1', kind: 'file_diff', content: '...', ... }
    }
  ],
  currentSummary: {
    id: 'sum1',
    capsuleId: 'cap1',
    content: 'Working on fixing authentication bug...',
    confidence: 0.85,
    ...
  },
  candidates: [
    {
      entity: { id: 'obs2', kind: 'tool_call', content: '...', ... },
      score: 0.92,
      matchContext: '...authentication bug...'
    },
    {
      entity: { id: 'sum2', content: '...auth flow...', ... },
      score: 0.78
    }
  ],
  provenance: {
    query: 'authentication bug',
    scopeIds: { repoId: '/repo', sessionId: 's1' },
    totalCandidates: 25,
    returnedCandidates: 15,
    truncatedDueToTokenBudget: true,
    providerUsed: 'local-fts'
  }
}
```

## Retrieval Tiering

Retrieval results are organized in a **three-tier structure**:

### Tier 1: Pins (Non-evictable)

**Source:** Store (pins table)

**Filtering:**

- Active pins only (`expiresAt` is null or `> now`)
- Scoped by `scopeIds`
- Excludes redacted targets (unless `includeRedacted=true`)

**Ordering:** By `createdAt` (most recent first)

**Characteristics:**

- **Always included** (not subject to token budget truncation)
- **User-curated** (explicitly marked important)
- **Provenance:** Pin reason + creation timestamp

### Tier 2: Current Summary (Non-evictable)

**Source:** Store (summaries table)

**Filtering:**

- Summary for the currently open capsule (if any)
- Scoped by `scopeIds`

**Characteristics:**

- **At most one** (one summary per capsule)
- **Always included** (not subject to token budget truncation)
- **Context-aware** (reflects ongoing work in the session)
- **Provenance:** Capsule ID + evidence refs + confidence

### Tier 3: Provider Candidates (Evictable)

**Source:** Provider (e.g., local FTS + recency scoring)

**Filtering:**

- FTS query match
- Scoped by `scopeIds`
- Excludes redacted observations (unless `includeRedacted=true`)
- Excludes observations already in pins or current summary (deduplication)

**Ordering:** By relevance score (highest first)

**Characteristics:**

- **Ranked by provider** (FTS relevance + recency)
- **Subject to truncation** (token budget applies)
- **Explainable scoring** (score + match context)
- **Provenance:** Query + provider name + score

## Provider Contract

Providers implement the retrieval search logic. The default provider is `LocalFtsProvider` (FTS + recency-based).

### Provider Interface

```typescript
interface RetrievalProvider {
  name: string; // Provider identifier (e.g., "local-fts")

  search(options: ProviderSearchOptions): Promise<ProviderSearchResult[]>;
}

interface ProviderSearchOptions {
  query: string; // Search query
  scopeIds: ScopeIds; // Scope filters
  maxResults?: number; // Max results to return
  excludeIds?: string[]; // IDs to exclude (for deduplication)
  includeRedacted?: boolean; // Include redacted observations
}

interface ProviderSearchResult {
  entity: Observation | Summary; // The matched entity
  score: number; // Relevance score (0.0-1.0)
  matchContext?: string; // Snippet showing match
}
```

### Local FTS Provider

The default provider uses SQLite FTS5 + recency scoring.

**Scoring formula:**

```
score = (fts_relevance * 0.7) + (recency_score * 0.3)

where:
  fts_relevance = BM25 score from FTS5 (normalized to 0.0-1.0)
  recency_score = 1.0 - (age_days / max_age_days)
```

**Characteristics:**

- **FTS-based:** Uses `observations_fts` and `summaries_fts` tables
- **Recency-weighted:** Recent observations score higher
- **Deterministic:** Same query + context → same results
- **Fast:** Leverages SQLite FTS5 indexes

**Example:**

```typescript
// FTS query: "authentication"
// Scope: { repoId: '/repo' }
// Max results: 50

// Provider returns:
[
  {
    entity: { id: 'obs1', content: 'Fixed authentication bug', ... },
    score: 0.95,
    matchContext: '...authentication bug...'
  },
  {
    entity: { id: 'sum1', content: 'Updated auth flow', ... },
    score: 0.82
  },
  // ... up to 50 results
]
```

## Scoping

All retrieval queries are **scoped** to prevent cross-contamination.

### Scope Dimensions

```typescript
interface ScopeIds {
  sessionId?: string; // Session isolation
  repoId?: string; // Repository isolation
  agentId?: string; // Agent isolation (future)
  userId?: string; // User isolation (future)
}
```

### Scope Filtering

**Behavior:**

- **AND semantics:** All specified dimensions must match
- **Partial matching:** Unspecified dimensions are ignored
- **Exact match:** No wildcards or prefixes

**Examples:**

```typescript
// Example 1: Session-only scope
scopeIds: { sessionId: 's1' }
// Returns: All entities where scope_ids->>'sessionId' = 's1'

// Example 2: Repo-only scope
scopeIds: { repoId: '/repo' }
// Returns: All entities where scope_ids->>'repoId' = '/repo'

// Example 3: Session + Repo scope
scopeIds: { sessionId: 's1', repoId: '/repo' }
// Returns: All entities where BOTH conditions match

// Example 4: Global (unscoped)
scopeIds: {}
// Returns: All entities (no filtering)
```

### Default Scoping

Adapters typically scope retrieval to:

```typescript
{
  sessionId: currentSessionId,
  repoId: currentRepoPath
}
```

This ensures:

- Session isolation (no cross-session leakage)
- Repository isolation (no cross-repo leakage)

## Token Budget and Truncation

Retrieval supports **token budget truncation** to limit result size.

### Truncation Behavior

1. **Pins and Current Summary:** Never truncated (non-evictable)
2. **Candidates:** Truncated to fit within budget

**Algorithm:**

```typescript
function applyTokenBudget(result: RetrieveResult, budget: number): RetrieveResult {
  let usedTokens = 0;

  // 1. Count tokens for pins (non-evictable)
  for (const pin of result.pins) {
    usedTokens += estimateTokens(pin.target);
  }

  // 2. Count tokens for current summary (non-evictable)
  if (result.currentSummary) {
    usedTokens += estimateTokens(result.currentSummary);
  }

  // 3. Add candidates until budget exhausted
  const truncatedCandidates = [];
  for (const candidate of result.candidates) {
    const candidateTokens = estimateTokens(candidate.entity);
    if (usedTokens + candidateTokens <= budget) {
      truncatedCandidates.push(candidate);
      usedTokens += candidateTokens;
    } else {
      break; // Budget exhausted
    }
  }

  return {
    ...result,
    candidates: truncatedCandidates,
    provenance: {
      ...result.provenance,
      truncatedDueToTokenBudget: truncatedCandidates.length < result.candidates.length,
    },
  };
}
```

### Token Estimation

Token count is estimated using a simple heuristic:

```typescript
function estimateTokens(entity: Observation | Summary): number {
  // Rough estimate: 1 token H 4 characters
  return Math.ceil(entity.content.length / 4);
}
```

**Note:** Adapters can use more sophisticated token counting (e.g., `tiktoken`) if needed.

## Deduplication

Retrieval automatically deduplicates results:

- **Pins vs. Candidates:** If an observation is pinned, it won't appear in candidates
- **Current Summary vs. Candidates:** If a summary is the current summary, it won't appear in candidates

**Implementation:**

```typescript
const excludeIds = [...result.pins.map((p) => p.target.id), result.currentSummary?.id].filter(
  Boolean,
);

const candidates = await provider.search({
  query,
  scopeIds,
  maxResults: maxCandidates,
  excludeIds, // Provider excludes these IDs
});
```

## Redaction Handling

By default, **redacted observations are excluded** from retrieval.

**Behavior:**

- `redacted=true` observations are filtered out
- Redacted observations in capsules still preserve provenance (IDs retained)
- Content shown as `[redacted]` if explicitly requested

**Override:**

```typescript
retrieve({
  query: 'auth',
  scopeIds: { repoId: '/repo' },
  includeRedacted: true, // Include redacted observations
});
```

**Use case:** Debugging or auditing (to see that redacted content existed)

## Provenance

All retrieval results include **provenance** explaining how they were generated.

### Provenance Structure

```typescript
interface RetrieveProvenance {
  query: string; // Original query
  scopeIds: ScopeIds; // Applied scope filters
  totalCandidates: number; // Total candidates before truncation
  returnedCandidates: number; // Actual candidates returned
  truncatedDueToTokenBudget: boolean; // Whether truncation occurred
  providerUsed: string; // Provider name (e.g., "local-fts")
}
```

### Entity-Level Provenance

Each result type has built-in provenance:

- **Pins:** `pin.reason` + `pin.createdAt`
- **Summary:** `summary.evidenceRefs` + `summary.confidence` + `summary.capsuleId`
- **Observation:** `observation.provenance` + `observation.ts`

### Example Provenance

```typescript
{
  query: 'authentication',
  scopeIds: { sessionId: 's1', repoId: '/repo' },
  totalCandidates: 25,
  returnedCandidates: 10,
  truncatedDueToTokenBudget: true,
  providerUsed: 'local-fts'
}
```

**Interpretation:**

- Query was "authentication"
- Scoped to session 's1' in repo '/repo'
- Provider found 25 candidates
- Token budget limited results to 10 candidates
- Used local FTS provider

## Determinism Guarantees

kindling retrieval is **deterministic** under these conditions:

1. **Same query** — Identical query string
2. **Same scope** — Identical `scopeIds`
3. **Same data** — No new observations, capsules, or pins created
4. **Same time** — TTL-based pins use `now` parameter (must be same)
5. **Same provider** — Same provider with same configuration

**Violation examples:**

- Time-based pins expired → results change
- New observations added → FTS index updated → results change
- Provider algorithm updated → scoring changes → results change

## Performance Considerations

### Query Optimization

- **FTS indexes:** Maintained on `observations.content` and `summaries.content`
- **Scope indexes:** JSON path indexes on `scope_ids->>'sessionId'`, etc.
- **Pin TTL index:** Index on `pins.expires_at` for fast filtering

### Caching

kindling does **not cache retrieval results** by default (to preserve determinism).

Adapters may implement caching if:

- Cache key includes all determinism inputs (query, scope, time)
- Cache invalidation on data changes

### Latency Targets

- **FTS query:** < 50ms (for typical dataset < 100k observations)
- **Pin lookup:** < 10ms
- **End-to-end retrieval:** < 100ms

## Extension Points

### Custom Providers

Providers can be swapped or extended:

```typescript
interface CustomProvider extends RetrievalProvider {
  name: 'custom-semantic';

  async search(options: ProviderSearchOptions): Promise<ProviderSearchResult[]> {
    // Use embeddings + vector search
    // Return scored results
  }
}
```

**Requirements:**

- Must return scored results (0.0-1.0)
- Must respect scope filters
- Should be deterministic (or document non-determinism)

### Future Enhancements (Not v0.1)

- **Hybrid search** — FTS + semantic embeddings
- **Re-ranking** — LLM-based relevance scoring
- **Faceted retrieval** — Filter by observation kind, time range
- **Personalization** — User-specific ranking heuristics

## Testing Retrieval

### Determinism Tests

```typescript
test('retrieval is deterministic', async () => {
  const result1 = await kindling.retrieve({ query: 'auth', scopeIds: { repoId: '/repo' } });
  const result2 = await kindling.retrieve({ query: 'auth', scopeIds: { repoId: '/repo' } });

  expect(result1).toEqual(result2); // Exact match
});
```

### Scoping Tests

```typescript
test('retrieval respects scope isolation', async () => {
  const result1 = await kindling.retrieve({ query: 'auth', scopeIds: { sessionId: 's1' } });
  const result2 = await kindling.retrieve({ query: 'auth', scopeIds: { sessionId: 's2' } });

  // No overlap (assuming sessions are distinct)
  const ids1 = result1.candidates.map((c) => c.entity.id);
  const ids2 = result2.candidates.map((c) => c.entity.id);
  expect(intersection(ids1, ids2)).toEqual([]);
});
```

### Truncation Tests

```typescript
test('retrieval respects token budget', async () => {
  const result = await kindling.retrieve({
    query: 'auth',
    scopeIds: { repoId: '/repo' },
    tokenBudget: 1000,
  });

  const totalTokens = estimateTokens(result);
  expect(totalTokens).toBeLessThanOrEqual(1000);
  expect(result.provenance.truncatedDueToTokenBudget).toBe(true);
});
```

## Contract Summary

| Property          | Requirement                                 |
| ----------------- | ------------------------------------------- |
| **Determinism**   | Same query + scope + data → same results    |
| **Scoping**       | All results respect `scopeIds` filters      |
| **Tiering**       | Pins → Summary → Candidates (in that order) |
| **Non-eviction**  | Pins and current summary never truncated    |
| **Provenance**    | All results include explanation             |
| **Redaction**     | Redacted observations excluded by default   |
| **Deduplication** | No entity appears in multiple tiers         |
| **Performance**   | < 100ms for typical queries                 |
