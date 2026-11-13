# Batch Pin Resolution Implementation Plan

> **Historical.** This plan targets the retired Node.js/TypeScript engine (pre-D-005).
> It is kept for reference and does not describe the current Rust engine. See
> [docs/architecture.md](../architecture.md) for the current design.

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace N individual `getObservationById`/`getSummaryById` calls during pin resolution with a single batched `WHERE id IN (...)` query per entity type.

**Architecture:** Add batch getter methods to the store, then update the retrieval orchestrator to call them instead of looping with single-ID lookups.

**Tech Stack:** SQLite, TypeScript

---

### Task 1: Add batch getters to SqliteKindlingStore

**Files:**

- Modify: `packages/kindling-store-sqlite/src/store/sqlite.ts`
- Test: `packages/kindling-store-sqlite/test/store.read.spec.ts`

**Step 1: Write the failing test**

```typescript
describe('getObservationsByIds', () => {
  it('should return multiple observations in a single query', () => {
    const obs1 = validateObservation({
      kind: 'message',
      content: 'first',
      scopeIds: { sessionId: 's1' },
    });
    const obs2 = validateObservation({
      kind: 'message',
      content: 'second',
      scopeIds: { sessionId: 's1' },
    });
    if (!obs1.ok || !obs2.ok) return;

    store.insertObservation(obs1.value);
    store.insertObservation(obs2.value);

    const results = store.getObservationsByIds([obs1.value.id, obs2.value.id]);
    expect(results).toHaveLength(2);
    expect(results.map((o) => o.id).sort()).toEqual([obs1.value.id, obs2.value.id].sort());
  });

  it('should return empty array for empty input', () => {
    expect(store.getObservationsByIds([])).toEqual([]);
  });

  it('should skip nonexistent IDs without error', () => {
    const obs1 = validateObservation({
      kind: 'message',
      content: 'test',
      scopeIds: { sessionId: 's1' },
    });
    if (!obs1.ok) return;
    store.insertObservation(obs1.value);

    const results = store.getObservationsByIds([obs1.value.id, 'nonexistent']);
    expect(results).toHaveLength(1);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `cd packages/kindling-store-sqlite && pnpm test -- --grep "getObservationsByIds"`
Expected: FAIL with "store.getObservationsByIds is not a function"

**Step 3: Write minimal implementation**

```typescript
getObservationsByIds(ids: string[]): Observation[] {
  if (ids.length === 0) return [];
  const placeholders = ids.map(() => '?').join(',');
  const rows = this.db.prepare(`
    SELECT id, kind, content, provenance, ts, scope_ids, redacted
    FROM observations
    WHERE id IN (${placeholders})
  `).all(...ids) as Array<{
    id: string; kind: string; content: string; provenance: string;
    ts: number; scope_ids: string; redacted: number;
  }>;
  return rows.map(row => ({
    id: row.id,
    kind: row.kind as Observation['kind'],
    content: row.content,
    provenance: JSON.parse(row.provenance),
    ts: row.ts,
    scopeIds: JSON.parse(row.scope_ids),
    redacted: row.redacted === 1,
  }));
}
```

Do the same for `getSummariesByIds`.

**Step 4: Run test to verify it passes**

Run: `cd packages/kindling-store-sqlite && pnpm test`
Expected: PASS

**Step 5: Commit**

```bash
git add packages/kindling-store-sqlite/src/store/sqlite.ts packages/kindling-store-sqlite/test/store.read.spec.ts
git commit -m "feat(store): add batch getters for observations and summaries"
```

---

### Task 2: Update RetrievalStore interface and orchestrator

**Files:**

- Modify: `packages/kindling-core/src/retrieval/orchestrator.ts`
- Test: `packages/kindling-core/test/retrieval.spec.ts`

**Step 1: Add batch methods to RetrievalStore interface**

```typescript
export interface RetrievalStore {
  // ... existing methods ...
  getObservationsByIds?(ids: ID[]): Observation[];
  getSummariesByIds?(ids: ID[]): Summary[];
}
```

Make them optional for backward compatibility.

**Step 2: Update pin resolution loop to use batch getters**

Replace the `for (const pin of pins)` loop with:

```typescript
const obsPins = pins.filter((p) => p.targetType === 'observation');
const sumPins = pins.filter((p) => p.targetType === 'summary');

const obsMap = new Map<string, Observation>();
if (obsPins.length > 0 && store.getObservationsByIds) {
  for (const obs of store.getObservationsByIds(obsPins.map((p) => p.targetId))) {
    obsMap.set(obs.id, obs);
  }
} else {
  for (const pin of obsPins) {
    const obs = store.getObservationById(pin.targetId);
    if (obs) obsMap.set(obs.id, obs);
  }
}
// Same pattern for summaries
```

**Step 3: Run tests**

Run: `pnpm run test`
Expected: All pass

**Step 4: Commit**

```bash
git add packages/kindling-core/
git commit -m "perf(retrieval): batch pin resolution to reduce DB round-trips"
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
git commit -m "chore: rebuild plugin bundle with batch pin resolution"
```
