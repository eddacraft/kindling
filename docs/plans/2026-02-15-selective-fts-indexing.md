# Selective FTS Indexing Implementation Plan

> **Historical.** This plan targets the retired Node.js/TypeScript engine (pre-D-005).
> It is kept for reference and does not describe the current Rust engine. See
> [docs/architecture.md](../architecture.md) for the current design.

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Truncate large observation content before FTS indexing to reduce index size and improve search speed without losing searchability.

**Architecture:** Add a content-preparation step in the FTS trigger that caps indexed text at a configurable limit. The full content stays in the `observations` table untouched; only the FTS shadow table gets the truncated version.

**Tech Stack:** SQLite FTS5 triggers, TypeScript

---

### Task 1: Add content length limit to FTS trigger

**Files:**

- Create: `packages/kindling-store-sqlite/migrations/005_fts_content_limit.sql`
- Modify: `packages/kindling-store-sqlite/src/db/migrate.ts`
- Test: `packages/kindling-store-sqlite/test/migrations.spec.ts`

**Step 1: Write the migration SQL**

```sql
-- Drop existing FTS insert triggers
DROP TRIGGER IF EXISTS observations_fts_insert;
DROP TRIGGER IF EXISTS summaries_fts_insert;

-- Drop existing FTS update triggers (these also insert into FTS)
DROP TRIGGER IF EXISTS observations_fts_update;
DROP TRIGGER IF EXISTS summaries_fts_update;

-- Recreate INSERT triggers with SUBSTR to cap indexed content at 2000 chars
CREATE TRIGGER observations_fts_insert AFTER INSERT ON observations
WHEN NEW.redacted = 0
BEGIN
  INSERT INTO observations_fts(rowid, content)
  VALUES (NEW.rowid, SUBSTR(NEW.content, 1, 2000));
END;

CREATE TRIGGER summaries_fts_insert AFTER INSERT ON summaries
BEGIN
  INSERT INTO summaries_fts(rowid, content)
  VALUES (NEW.rowid, SUBSTR(NEW.content, 1, 2000));
END;

-- Recreate UPDATE triggers with SUBSTR for the re-insert step
CREATE TRIGGER observations_fts_update AFTER UPDATE ON observations
BEGIN
  INSERT INTO observations_fts(observations_fts, rowid, content)
  VALUES('delete', OLD.rowid, SUBSTR(OLD.content, 1, 2000));
  INSERT INTO observations_fts(rowid, content)
  SELECT NEW.rowid, SUBSTR(NEW.content, 1, 2000) WHERE NEW.redacted = 0;
END;

CREATE TRIGGER summaries_fts_update AFTER UPDATE ON summaries
BEGIN
  INSERT INTO summaries_fts(summaries_fts, rowid, content)
  VALUES('delete', OLD.rowid, SUBSTR(OLD.content, 1, 2000));
  INSERT INTO summaries_fts(rowid, content)
  VALUES (NEW.rowid, SUBSTR(NEW.content, 1, 2000));
END;
```

**Step 2: Register the migration**

In `packages/kindling-store-sqlite/src/db/migrate.ts`, add migration 005 to the `getMigrations()` array.

**Step 3: Update migration tests**

In `packages/kindling-store-sqlite/test/migrations.spec.ts`:

- Update migration count assertions from 4 → 5
- Add assertion for migration 005 name

**Step 4: Write a test for truncated indexing**

```typescript
describe('FTS content truncation', () => {
  it('should index only first 2000 chars of long content', () => {
    const longContent = 'searchterm ' + 'x'.repeat(3000);
    db.prepare(
      `
      INSERT INTO observations (id, kind, content, provenance, ts, scope_ids, redacted)
      VALUES ('obs-long', 'message', ?, '{}', 1000, '{}', 0)
    `,
    ).run(longContent);

    // Should still find it via FTS
    const result = db
      .prepare(
        `
      SELECT COUNT(*) as count FROM observations_fts WHERE content MATCH 'searchterm'
    `,
      )
      .get();
    expect(result.count).toBe(1);
  });

  it('should not index content beyond 2000 chars', () => {
    const longContent = 'x'.repeat(2100) + 'uniquetoken';
    db.prepare(
      `
      INSERT INTO observations (id, kind, content, provenance, ts, scope_ids, redacted)
      VALUES ('obs-long2', 'message', ?, '{}', 1000, '{}', 0)
    `,
    ).run(longContent);

    // Should NOT find uniquetoken since it's beyond 2000 chars
    const result = db
      .prepare(
        `
      SELECT COUNT(*) as count FROM observations_fts WHERE content MATCH 'uniquetoken'
    `,
      )
      .get();
    expect(result.count).toBe(0);
  });
});
```

**Step 5: Run tests**

Run: `cd packages/kindling-store-sqlite && pnpm test`
Expected: PASS

**Step 6: Commit**

```bash
git add packages/kindling-store-sqlite/migrations/005_fts_content_limit.sql \
       packages/kindling-store-sqlite/src/db/migrate.ts \
       packages/kindling-store-sqlite/test/migrations.spec.ts
git commit -m "perf(fts): truncate long content before FTS indexing"
```

---

### Task 2: Rebuild existing FTS index with truncated content

**Files:**

- Modify: `packages/kindling-store-sqlite/migrations/005_fts_content_limit.sql`

**Step 1: Add rebuild commands to migration**

Append to the migration SQL:

```sql
-- Rebuild observations FTS with truncated content
DELETE FROM observations_fts;
INSERT INTO observations_fts(rowid, content)
  SELECT rowid, SUBSTR(content, 1, 2000)
  FROM observations
  WHERE redacted = 0;

-- Rebuild summaries FTS with truncated content
DELETE FROM summaries_fts;
INSERT INTO summaries_fts(rowid, content)
  SELECT rowid, SUBSTR(content, 1, 2000)
  FROM summaries;
```

**Step 2: Run tests**

Run: `pnpm run test`
Expected: All pass

**Step 3: Commit**

```bash
git add packages/kindling-store-sqlite/migrations/005_fts_content_limit.sql
git commit -m "perf(fts): rebuild existing FTS index with truncated content"
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
git commit -m "chore: rebuild plugin bundle with selective FTS indexing"
```
