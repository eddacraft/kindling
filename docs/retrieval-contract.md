# Retrieval Contract

## Overview

kindling retrieval is **deterministic**, **scoped**, and **explainable**. The same query with the same context always produces the same results. All results include provenance, explaining why they were returned.

Rust is canonical: retrieval is implemented in the `kindling-provider` crate (default provider id `"local-fts"`), exposed through `kindling-service` (embedded) and `kindling-client` (daemon). Domain types come from `kindling-types` (camelCase JSON; ts-rs bindings checked for drift in CI).

Retrieval combines three sources in a tiered structure:

1. **Pins** — User-marked important items (non-evictable)
2. **Current Summary** — Latest closed-capsule summary (non-evictable)
3. **Provider Candidates** — FTS-ranked observations and summaries (evictable)

## Core Interface

### Retrieve Function

```rust
pub struct RetrieveOptions {
    pub query: String,                  // Search query
    pub scope_ids: ScopeIds,            // Isolation dimensions (camelCase: scopeIds)
    pub token_budget: Option<usize>,    // DEPRECATED — see "Token Budget and Truncation"
    pub max_candidates: Option<usize>,  // Max candidates from provider (the bounded-result knob)
    pub include_redacted: Option<bool>, // Include redacted observations (default: false)
}

pub struct RetrieveResult {
    pub pins: Vec<PinResult>,            // Active pins
    pub current_summary: Option<Summary>, // Latest closed-capsule summary (if any)
    pub candidates: Vec<CandidateResult>, // Provider results
    pub provenance: RetrieveProvenance,   // Explains how results were generated
}

pub struct PinResult {
    pub pin: Pin,                 // The pin itself
    pub target: RetrievalEntity,  // The pinned Observation or Summary
}

pub struct CandidateResult {
    pub entity: RetrievalEntity,      // The matched Observation or Summary
    pub score: f64,                   // Relevance score (0.0..=1.0)
    pub match_context: Option<String>, // Snippet showing the match
}

pub struct RetrieveProvenance {
    pub query: String,                    // Original query
    pub scope_ids: ScopeIds,              // Applied scope filters
    pub total_candidates: usize,          // Total candidates before bounding
    pub returned_candidates: usize,       // Actual candidates returned
    pub truncated_due_to_token_budget: bool, // Legacy flag (see note below)
    pub provider_used: String,            // Provider id (e.g., "local-fts")
}
```

All fields serialize as camelCase (`scopeIds`, `currentSummary`, `matchContext`, `totalCandidates`, `truncatedDueToTokenBudget`, `providerUsed`, …).

### Example Usage

Embedded (`kindling-service`):

```rust
use kindling_service::KindlingService;

let svc = KindlingService::open("./.kindling/kindling.db")?;
let result = svc.retrieve(retrieve_options)?; // query, scopeIds { repoId, sessionId }, max_candidates
```

Daemon (`kindling-client`):

```rust
use kindling_client::Client;

let client = Client::new()?;
let result = client.retrieve(retrieve_options).await?; // client methods are async
```

The `RetrieveResult` (serialized JSON):

```jsonc
{
  "pins": [
    {
      "pin": { "id": "pin1", "targetType": "observation", "targetId": "obs1" /* … */ },
      "target": { "id": "obs1", "kind": "file_diff", "content": "…" },
    },
  ],
  "currentSummary": {
    "id": "sum1",
    "capsuleId": "cap1",
    "content": "Worked on fixing the authentication bug…",
    "confidence": 0.85,
  },
  "candidates": [
    {
      "entity": { "id": "obs2", "kind": "tool_call", "content": "…" },
      "score": 0.92,
      "matchContext": "…authentication bug…",
    },
    { "entity": { "id": "sum2", "content": "…auth flow…" }, "score": 0.78 },
  ],
  "provenance": {
    "query": "authentication bug",
    "scopeIds": { "repoId": "/repo", "sessionId": "s1" },
    "totalCandidates": 25,
    "returnedCandidates": 15,
    "truncatedDueToTokenBudget": false,
    "providerUsed": "local-fts",
  },
}
```

## Retrieval Tiering

Retrieval results are organized in a **three-tier structure**:

### Tier 1: Pins (Non-evictable)

**Source:** Store (`pins` table)

**Filtering:**

- Active pins only (`expiresAt` is null or `> now`)
- Scoped by `scopeIds` (filterable dimensions only)
- Excludes redacted targets (unless `include_redacted = true`)

**Ordering:** By `createdAt` (most recent first)

**Characteristics:**

- **Always included** (not bounded by `max_candidates`)
- **User-curated** (explicitly marked important)
- **Provenance:** Pin reason + creation timestamp

### Tier 2: Current Summary (Non-evictable)

**Source:** Store (`summaries` table)

**Filtering:**

- The latest closed-capsule summary in scope (if any)
- Scoped by `scopeIds`

**Characteristics:**

- **At most one**
- **Always included** (not bounded by `max_candidates`)
- **Context-aware** (reflects recent work in the session)
- **Provenance:** Capsule id + evidence refs + confidence

### Tier 3: Provider Candidates (Evictable)

**Source:** Provider (`local-fts` — FTS5 + recency scoring)

**Filtering:**

- FTS query match (over `observations_fts` and `summaries_fts`)
- Scoped by `scopeIds`
- Excludes redacted observations (unless `include_redacted = true`)
- Excludes entities already in pins or current summary (deduplication)

**Ordering:** By relevance score (highest first)

**Characteristics:**

- **Ranked by provider** (FTS relevance blended with recency)
- **Bounded** by `max_candidates`
- **Explainable scoring** (score + match context)
- **Provenance:** Query + provider id + score

## Provider Contract

The retrieval search logic is implemented by a provider. The default — and only stable — provider is `local-fts` in the `kindling-provider` crate (FTS5 + recency).

Conceptually a provider is a search function: given a query, scope filters, a
result bound, a set of ids to exclude (for deduplication), a redaction flag, and
an explicit `now` clock, it returns scored entities. In Rust this is expressed as
a trait implemented by the local provider.

```rust
// Conceptual — the provider trait as implemented by the local FTS provider.
pub trait RetrievalProvider {
    fn name(&self) -> &str; // e.g., "local-fts"

    fn search(&self, options: ProviderSearchOptions) -> Result<Vec<ProviderSearchResult>>;
}

pub struct ProviderSearchOptions {
    pub query: String,
    pub scope_ids: ScopeIds,        // filterable dimensions only
    pub max_results: Option<usize>,
    pub exclude_ids: Vec<String>,   // for deduplication
    pub include_redacted: bool,
    pub retrieve_at: i64,           // explicit "now" (epoch ms) — recency + determinism
}

pub struct ProviderSearchResult {
    pub entity: RetrievalEntity,      // matched Observation or Summary
    pub score: f64,                   // 0.0..=1.0
    pub match_context: Option<String>,
}
```

> The provider trait is an internal extension point, not a published plugin API. Treat custom providers as a conceptual/future capability — see [Extension Points](#extension-points).

### Local FTS Provider

The default provider uses SQLite FTS5 (tokenizer `porter unicode61`) blended with recency.

**Scoring formula:**

```
score = (fts_relevance * 0.7) + (recency * 0.3)

where:
  fts_relevance = BM25 score from FTS5, normalized to [0, 1]
                  across observations + summaries
  recency       = MAX(0, 1 - (age_ms / max_age_ms))
  max_age_ms    = 30-day window
  age_ms        = retrieve_at - entity.ts
```

**Characteristics:**

- **FTS-based:** Uses `observations_fts` and `summaries_fts`
- **Recency-weighted:** Recent entities score higher (30-day window)
- **Deterministic:** Same query + scope + data + `retrieve_at` → same results
- **Fast:** Leverages SQLite FTS5 indexes

The provider takes an explicit `retrieve_at` (the `now` clock) so recency scoring is testable and reproducible; nothing reads the wall clock implicitly.

## Scoping

All retrieval queries are **scoped** to prevent cross-contamination.

### Scope Dimensions

```rust
pub struct ScopeIds {
    pub session_id: Option<String>, // FILTERABLE
    pub repo_id: Option<String>,    // FILTERABLE
    pub agent_id: Option<String>,   // FILTERABLE
    pub user_id: Option<String>,    // FILTERABLE
    pub task_id: Option<String>,    // NOT filterable — provenance/grouping only
}
```

`sessionId`, `repoId`, `agentId`, and `userId` are denormalized into real columns and are matched directly. **`taskId` has no column and is never used as a filter** — supplying it in `scopeIds` does not affect the result set (it is carried for provenance/grouping only).

### Scope Filtering

**Behavior:**

- **AND semantics:** All specified filterable dimensions must match
- **Partial matching:** Unspecified dimensions are ignored
- **Exact match:** No wildcards or prefixes
- **No `taskId` filtering:** Ignored even if present

**Examples (JSON):**

```jsonc
// Session-only scope → entities WHERE session_id = 's1'
{ "sessionId": "s1" }

// Repo-only scope → entities WHERE repo_id = '/repo'
{ "repoId": "/repo" }

// Session + repo → entities WHERE BOTH columns match
{ "sessionId": "s1", "repoId": "/repo" }

// taskId is ignored for filtering (carried for provenance only)
{ "repoId": "/repo", "taskId": "t1" }  // same result set as { "repoId": "/repo" }

// Global (unscoped) → all entities, no filtering
{ }
```

### Default Scoping

Adapters typically scope retrieval to:

```jsonc
{ "sessionId": "<currentSessionId>", "repoId": "<currentRepoPath>" }
```

This ensures session isolation (no cross-session leakage) and repository isolation (no cross-repo leakage).

## Token Budget and Truncation

> **`token_budget` is DEPRECATED.** Token-budget assembly is a **downstream-system responsibility**, not the retrieval engine's. The engine does not pack results to a token target. Prefer **`max_candidates`** to bound result size.

### Bounding behavior (current)

1. **Pins and Current Summary:** Always returned (non-evictable, not bounded)
2. **Candidates:** Bounded to `max_candidates` from the provider, ordered by score

The engine returns the top-scoring candidates up to `max_candidates`. Deciding how
many of those to actually feed into a prompt — and any token accounting — is left
to the consumer (an adapter or a downstream context-assembly system), which can
apply whatever tokenizer it prefers (e.g., `tiktoken`).

### `truncatedDueToTokenBudget` in provenance

The field **still exists** in `RetrieveProvenance` for wire/back-compat stability, but it reflects the legacy token-budget path. With `max_candidates`-based bounding it is typically `false`. Consumers should rely on `totalCandidates` vs. `returnedCandidates` to detect that more candidates were available than were returned, rather than on this flag.

## Deduplication

Retrieval automatically deduplicates across tiers:

- **Pins vs. Candidates:** A pinned entity will not appear in candidates
- **Current Summary vs. Candidates:** The current summary will not appear in candidates

The engine collects the ids already present in pins and the current summary and passes them as `exclude_ids` to the provider:

```rust
// Conceptual
let mut exclude_ids: Vec<String> =
    result.pins.iter().map(|p| p.target.id().to_string()).collect();
if let Some(summary) = &result.current_summary {
    exclude_ids.push(summary.id.clone());
}

let candidates = provider.search(ProviderSearchOptions {
    query,
    scope_ids,
    max_results: max_candidates,
    exclude_ids,            // provider omits these
    include_redacted,
    retrieve_at,
})?;
```

## Redaction Handling

By default, **redacted observations are excluded** from retrieval.

**Behavior:**

- `redacted = true` observations are filtered out of all tiers
- Redaction is performed via `forget`: content is moved to a redacted state, the entity is removed from FTS, and provenance/references are preserved
- Set `include_redacted = true` to include them (content appears as `[redacted]`)

**Override (JSON options):**

```jsonc
{
  "query": "auth",
  "scopeIds": { "repoId": "/repo" },
  "includeRedacted": true,
}
```

**Use case:** Debugging or auditing (to confirm redacted content existed).

## Provenance

All retrieval results include **provenance** explaining how they were generated.

### Provenance Structure

```rust
pub struct RetrieveProvenance {
    pub query: String,                       // Original query
    pub scope_ids: ScopeIds,                 // Applied scope filters
    pub total_candidates: usize,             // Total candidates before bounding
    pub returned_candidates: usize,          // Actual candidates returned
    pub truncated_due_to_token_budget: bool, // Legacy flag (see Token Budget section)
    pub provider_used: String,               // Provider id (e.g., "local-fts")
}
```

### Entity-Level Provenance

Each result type carries its own provenance:

- **Pins:** `pin.reason` + `pin.createdAt`
- **Summary:** `summary.evidenceRefs` + `summary.confidence` + `summary.capsuleId`
- **Observation:** `observation.provenance` + `observation.ts`

### Example Provenance

```jsonc
{
  "query": "authentication",
  "scopeIds": { "sessionId": "s1", "repoId": "/repo" },
  "totalCandidates": 25,
  "returnedCandidates": 10,
  "truncatedDueToTokenBudget": false,
  "providerUsed": "local-fts",
}
```

**Interpretation:**

- Query was "authentication"
- Scoped to session `s1` in repo `/repo`
- Provider found 25 candidates, returned 10 (bounded by `max_candidates`)
- Used the local FTS provider

## Determinism Guarantees

kindling retrieval is **deterministic** under these conditions:

1. **Same query** — Identical query string
2. **Same scope** — Identical `scopeIds` (filterable dimensions)
3. **Same data** — No new observations, capsules, summaries, or pins
4. **Same clock** — Same `retrieve_at` (the explicit `now`; pin expiry and recency depend on it)
5. **Same provider** — Same provider with the same configuration

**Violation examples:**

- Time-based pins expire → results change
- New observations added → FTS index updated → results change
- Provider scoring updated → results change

Because the provider takes an explicit `retrieve_at`, tests can pin the clock and assert byte-for-byte stable results.

## Performance Considerations

### Query Optimization

- **FTS indexes:** `observations_fts` and `summaries_fts` (tokenizer `porter unicode61`)
- **Scope indexes:** Indexes over the denormalized `session_id` / `repo_id` columns (e.g., `idx_obs_session_ts`, `idx_obs_repo_ts`)
- **Pin filtering:** `expires_at` is checked against `retrieve_at`

### Caching

kindling does **not cache retrieval results** by default (to preserve determinism). A consumer may cache only if the cache key includes all determinism inputs (query, scope, `retrieve_at`) and is invalidated on data changes.

### Latency Targets

- **FTS query:** < 50ms (typical dataset < 100k observations)
- **Pin lookup:** < 10ms
- **End-to-end retrieval:** < 100ms

## Extension Points

### Custom Providers (conceptual / future)

The `RetrievalProvider` trait is an internal seam, not a stable, published plugin API. A future custom provider (e.g., embeddings + vector search) would implement the same trait, returning scored entities for a query+scope:

```rust
// Conceptual sketch — NOT a stable API.
struct SemanticProvider;

impl RetrievalProvider for SemanticProvider {
    fn name(&self) -> &str { "custom-semantic" }

    fn search(&self, options: ProviderSearchOptions) -> Result<Vec<ProviderSearchResult>> {
        // embeddings + vector search, then return scored results
        todo!()
    }
}
```

**Requirements for any provider:**

- Must return scores in `[0.0, 1.0]`
- Must respect scope filters (filterable dimensions only)
- Should be deterministic given the same inputs + `retrieve_at` (or document non-determinism)

### Future Enhancements

- **Hybrid search** — FTS + semantic embeddings
- **Re-ranking** — LLM-based relevance scoring
- **Faceted retrieval** — Filter by observation kind, time range
- **Personalization** — User-specific ranking heuristics

## Testing Retrieval

Examples use `kindling-service`; the same assertions hold via `kindling-client`. A pinned `retrieve_at` makes results reproducible.

### Determinism Test

```rust
#[test]
fn retrieval_is_deterministic() -> anyhow::Result<()> {
    let svc = KindlingService::open(":memory:")?;
    // … seed identical data …

    let r1 = svc.retrieve(opts.clone())?; // same query + scope + retrieve_at
    let r2 = svc.retrieve(opts.clone())?;

    assert_eq!(r1, r2); // exact match
    Ok(())
}
```

### Scoping Test

```rust
#[test]
fn retrieval_respects_scope_isolation() -> anyhow::Result<()> {
    let svc = KindlingService::open(":memory:")?;
    // … seed s1 and s2 with distinct data …

    let r1 = svc.retrieve(opts_for_session("s1"))?;
    let r2 = svc.retrieve(opts_for_session("s2"))?;

    let ids1: Vec<_> = r1.candidates.iter().map(|c| c.entity.id().to_string()).collect();
    let ids2: Vec<_> = r2.candidates.iter().map(|c| c.entity.id().to_string()).collect();
    assert!(ids1.iter().all(|id| !ids2.contains(id))); // no overlap
    Ok(())
}
```

### Bounding Test

```rust
#[test]
fn retrieval_respects_max_candidates() -> anyhow::Result<()> {
    let svc = KindlingService::open(":memory:")?;
    // … seed > 5 matching observations …

    let result = svc.retrieve(opts_with_max_candidates(5))?;

    assert!(result.candidates.len() <= 5);
    assert!(result.provenance.total_candidates >= result.provenance.returned_candidates);
    Ok(())
}
```

## Contract Summary

| Property          | Requirement                                                        |
| ----------------- | ------------------------------------------------------------------ |
| **Determinism**   | Same query + scope + data + `retrieve_at` → same results           |
| **Scoping**       | All results respect filterable `scopeIds` (`taskId` ignored)       |
| **Tiering**       | Pins → Current Summary → Candidates (in that order)                |
| **Non-eviction**  | Pins and current summary always returned                           |
| **Bounding**      | Candidates bounded by `max_candidates` (`token_budget` deprecated) |
| **Provenance**    | All results include explanation                                    |
| **Redaction**     | Redacted observations excluded by default                          |
| **Deduplication** | No entity appears in multiple tiers                                |
| **Performance**   | < 100ms for typical queries                                        |
