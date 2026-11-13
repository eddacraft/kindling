# Query Result Caching Implementation Plan

> **Historical.** This plan targets the retired Node.js/TypeScript engine (pre-D-005).
> It is kept for reference and does not describe the current Rust engine. See
> [docs/architecture.md](../architecture.md) for the current design.

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Cache frequently repeated retrieval queries with a short TTL to avoid redundant DB round-trips during rapid sequential hook invocations (e.g., multiple tool calls in the same session triggering the same FTS search).

**Architecture:** Add an in-memory LRU cache in front of the retrieval orchestrator. Cache keys are derived from the query + scope + options hash. The cache lives in the KindlingService instance so it's scoped to the DB connection lifetime. TTL-based expiry ensures stale data doesn't persist.

**Tech Stack:** TypeScript, LRU cache (hand-rolled, no dependencies)

---

### Task 1: Create a lightweight LRU cache utility

**Files:**

- Create: `packages/kindling-core/src/util/lru-cache.ts`
- Test: `packages/kindling-core/test/lru-cache.spec.ts`

**Step 1: Write the failing test**

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { LruCache } from '../src/util/lru-cache.js';

describe('LruCache', () => {
  it('should store and retrieve values', () => {
    const cache = new LruCache<string>({ maxSize: 10, ttlMs: 60_000 });
    cache.set('key1', 'value1');
    expect(cache.get('key1')).toBe('value1');
  });

  it('should return undefined for missing keys', () => {
    const cache = new LruCache<string>({ maxSize: 10, ttlMs: 60_000 });
    expect(cache.get('missing')).toBeUndefined();
  });

  it('should evict least recently used when full', () => {
    const cache = new LruCache<string>({ maxSize: 2, ttlMs: 60_000 });
    cache.set('a', '1');
    cache.set('b', '2');
    cache.set('c', '3'); // evicts 'a'
    expect(cache.get('a')).toBeUndefined();
    expect(cache.get('b')).toBe('2');
    expect(cache.get('c')).toBe('3');
  });

  it('should expire entries after TTL', () => {
    vi.useFakeTimers();
    const cache = new LruCache<string>({ maxSize: 10, ttlMs: 1000 });
    cache.set('key', 'value');
    expect(cache.get('key')).toBe('value');

    vi.advanceTimersByTime(1001);
    expect(cache.get('key')).toBeUndefined();
    vi.useRealTimers();
  });

  it('should refresh access order on get', () => {
    const cache = new LruCache<string>({ maxSize: 2, ttlMs: 60_000 });
    cache.set('a', '1');
    cache.set('b', '2');
    cache.get('a'); // refresh 'a'
    cache.set('c', '3'); // evicts 'b', not 'a'
    expect(cache.get('a')).toBe('1');
    expect(cache.get('b')).toBeUndefined();
  });

  it('should clear all entries', () => {
    const cache = new LruCache<string>({ maxSize: 10, ttlMs: 60_000 });
    cache.set('a', '1');
    cache.set('b', '2');
    cache.clear();
    expect(cache.get('a')).toBeUndefined();
    expect(cache.get('b')).toBeUndefined();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `cd packages/kindling-core && pnpm test -- --grep "LruCache"`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

```typescript
interface CacheEntry<T> {
  value: T;
  expiresAt: number;
}

interface LruCacheOptions {
  maxSize: number;
  ttlMs: number;
}

export class LruCache<T> {
  private map = new Map<string, CacheEntry<T>>();
  private maxSize: number;
  private ttlMs: number;

  constructor(options: LruCacheOptions) {
    this.maxSize = options.maxSize;
    this.ttlMs = options.ttlMs;
  }

  get(key: string): T | undefined {
    const entry = this.map.get(key);
    if (!entry) return undefined;
    if (Date.now() > entry.expiresAt) {
      this.map.delete(key);
      return undefined;
    }
    // Move to end (most recently used)
    this.map.delete(key);
    this.map.set(key, entry);
    return entry.value;
  }

  set(key: string, value: T): void {
    this.map.delete(key); // Remove if exists to refresh position
    if (this.map.size >= this.maxSize) {
      // Evict oldest (first key in Map iteration order)
      const firstKey = this.map.keys().next().value;
      if (firstKey !== undefined) this.map.delete(firstKey);
    }
    this.map.set(key, { value, expiresAt: Date.now() + this.ttlMs });
  }

  clear(): void {
    this.map.clear();
  }
}
```

**Step 4: Run test to verify it passes**

Run: `cd packages/kindling-core && pnpm test -- --grep "LruCache"`
Expected: PASS

**Step 5: Commit**

```bash
git add packages/kindling-core/src/util/lru-cache.ts packages/kindling-core/test/lru-cache.spec.ts
git commit -m "feat(core): add LRU cache utility with TTL expiry"
```

---

### Task 2: Integrate cache into KindlingService.retrieve()

**Files:**

- Modify: `packages/kindling-core/src/service.ts`
- Test: `packages/kindling-core/test/service-cache.spec.ts`

**Step 1: Write the failing test**

```typescript
describe('KindlingService retrieval caching', () => {
  it('should return cached results for identical queries within TTL', () => {
    // Insert an observation, retrieve twice, verify provider is only called once
    const providerSpy = vi.spyOn(provider, 'search');

    const result1 = service.retrieve({ query: 'test', scopeIds: { sessionId: 's1' } });
    const result2 = service.retrieve({ query: 'test', scopeIds: { sessionId: 's1' } });

    expect(result1).toEqual(result2);
    expect(providerSpy).toHaveBeenCalledTimes(1);
  });

  it('should not cache with different query parameters', () => {
    const providerSpy = vi.spyOn(provider, 'search');

    service.retrieve({ query: 'test1', scopeIds: { sessionId: 's1' } });
    service.retrieve({ query: 'test2', scopeIds: { sessionId: 's1' } });

    expect(providerSpy).toHaveBeenCalledTimes(2);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `cd packages/kindling-core && pnpm test -- --grep "retrieval caching"`
Expected: FAIL — provider called twice

**Step 3: Add cache to KindlingService**

In `packages/kindling-core/src/service.ts`:

```typescript
import { LruCache } from './util/lru-cache.js';

// Inside class:
private retrievalCache = new LruCache<RetrieveResult>({
  maxSize: 50,
  ttlMs: 10_000, // 10 second TTL
});

retrieve(options: RetrieveOptions): RetrieveResult {
  const cacheKey = JSON.stringify({
    q: options.query,
    s: options.scopeIds,
    m: options.maxResults,
    p: options.pinnedOnly,
  });

  const cached = this.retrievalCache.get(cacheKey);
  if (cached) return cached;

  // ... existing retrieval logic ...

  this.retrievalCache.set(cacheKey, result);
  return result;
}
```

Also invalidate cache on writes:

```typescript
appendObservation(obs: Observation): void {
  // ... existing logic ...
  this.retrievalCache.clear();
}

pin(/* ... */): void {
  // ... existing logic ...
  this.retrievalCache.clear();
}
```

**Step 4: Run test to verify it passes**

Run: `cd packages/kindling-core && pnpm test`
Expected: PASS

**Step 5: Commit**

```bash
git add packages/kindling-core/src/service.ts packages/kindling-core/test/service-cache.spec.ts
git commit -m "perf(service): cache retrieval results with 10s TTL"
```

---

### Task 3: Rebuild bundle

**Step 1: Build and test**

```bash
pnpm run build
pnpm run test
node plugins/kindling-claude-code/scripts/build-bundle.js
```

**Step 2: Commit**

```bash
git add plugins/kindling-claude-code/dist/kindling-bundle.cjs
git commit -m "chore: rebuild plugin bundle with query result caching"
```
