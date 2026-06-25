# kindling read API + spool retention cap — design

| Field   | Value                                                        |
| ------- | ------------------------------------------------------------ |
| Status  | Accepted                                                     |
| Date    | 2026-06-26                                                   |
| Owner   | @aneki                                                       |
| Decides | D-009                                                        |
| Items   | KINTEG-003 (list/enumerate read API), KINTEG-009 (spool cap) |
| Source  | anvil-001 issues #2910 (KDS-004), #2916 (KDS-005 prereq)     |

Produced by a planning council (architect + delivery-lead + adversarial-reviewer).
This spec captures the resolved decisions and — importantly — the correctness
mitigations the adversarial pass surfaced, so the implementer does not rediscover
them.

## Problem

Downstream **anvil** is moving its `command.invoked` usage telemetry off a local
`usage.ndjson` sidecar onto the kindling daemon (anvil KDS module). Two kindling
gaps block that cutover:

1. **No exhaustive read path (KDS-004 / #2910).** anvil's `anvil kindling usage`
   views (`top` / `unused` / `flags` / `principals`) compute **exact counts** and
   a **set-difference** (`never_invoked = registered − seen`) over _all_
   `command.invoked` rows for a repo scope. The only observation-read endpoint is
   `POST /v1/retrieve` — ranked top-K (FTS5 BM25 + recency), capped by
   `max_candidates`. It returns the most-relevant sample, **not the full set**, so
   counts are wrong and rarely-used commands are falsely reported never-invoked.

2. **Uncapped spool (KDS-005 prereq / #2916).** Retiring anvil's sidecar makes
   `kindling-client`'s `SpooledClient` spool the only durable NDJSON fallback. The
   sidecar it replaces trims to a rolling **7-day / 64 MiB** window
   (`trim_usage_sidecar`); the spool has **no** retention bound and grows unbounded
   under a prolonged daemon outage. `SpoolConfig` reserves "rotation, size caps" as
   future knobs but ships neither.

## Guiding principle

kindling is **mechanism, not policy**. It exposes generic capabilities (enumerate
observations, bound the spool) and never encodes anvil's governance vocabulary
("flag", "principal", "invoked", retention numbers). Aggregation and retention
_values_ are anvil's; kindling provides the primitives.

---

## Feature 1 — KINTEG-003: structured list/enumerate read API

### Endpoint

`POST /v1/observations/list` with a JSON body. Matches every other parameterized
data route (POST + JSON + `X-Kindling-Project` per-project routing). A GET with
query params would have to URL-encode nested `scopeIds`, a multi-valued `kinds`,
and the cursor — rejected.

### Request

```jsonc
{
  "scopeIds": { "repoId": "…" }, // ScopeIds; task_id is NOT a field (non-filterable)
  "kinds": ["command"], // ObservationKind[]; omitted/empty = all kinds
  "since": 1750000000000, // optional, epoch ms, INCLUSIVE  (>=)
  "until": 1750086400000, // optional, epoch ms, EXCLUSIVE  (<)  — half-open [since, until)
  "limit": 500, // optional; server clamps to [1, 1000], default 100
  "cursor": null, // optional opaque token from a prior nextCursor
  "includeRedacted": false, // optional, default false
}
```

- **`kinds` is a list**, not a single value — anvil enumerates `command` today but
  a list avoids a two-request split (each request is a different point-in-time
  read) and stays mechanism-only. (adversarial F7)
- **Half-open time bounds** (`since` inclusive, `until` exclusive) so two adjacent
  range polls `[t0,t1)` + `[t1,t2)` never double-count the `ts == t1` boundary
  rows. This intentionally differs from the store's existing both-inclusive
  `query_observations`. (adversarial F6)
- **No `taskId`**: the request type has no such field, so it is structurally
  impossible to pass a non-filterable dimension. (adversarial F9)
- **`limit` is server-clamped** (max 1000) so one client cannot pull an unbounded
  page into memory while holding the per-project service mutex. (adversarial F11)

### Pagination — keyset cursor over `(ts ASC, id ASC)`

Use the total order `export_observations` already guarantees: `ORDER BY ts ASC, id
ASC`. The cursor is an **opaque** base64 token encoding `"<ts>:<id>"` (the last row
of the prior page). Next page predicate:

```sql
WHERE <scope filters>
  AND (kinds is empty OR kind IN (…))
  AND (since IS NULL OR ts >= since)
  AND (until IS NULL OR ts <  until)
  AND (cursor IS NULL OR (ts > cur_ts OR (ts = cur_ts AND id > cur_id)))
  AND (include_redacted OR redacted = 0)
ORDER BY ts ASC, id ASC
LIMIT limit + 1     -- the +1 sentinel determines nextCursor
```

Keyset (not offset) is the **only** scheme complete under concurrent appends: the
cursor is a value in the data, so a row inserted after the cursor passed its
position is picked up on a later page, never skipped or duplicated by an offset
shift. (adversarial F1)

### Snapshot / consistency contract

A multi-page enumeration is **not** a single SQLite snapshot (holding a read txn
across client round-trips is wrong). Instead the **client pins `until = now_ms()`
at enumeration start**, giving a fixed upper bound: rows written after start have
`ts >= until` and fall consistently out of range (counted on the _next_ poll, not
silently skipped mid-enumeration). The only residual ambiguity is rows whose `ts`
collides at the exact `until` millisecond boundary — negligible for usage
analytics and **documented** as the completeness boundary. anvil should call usage
views at quiescent moments and treat counts as "as-of the pinned `until`".
(adversarial F1)

### Response

```jsonc
{
  "observations": [
    /* Observation[] in (ts ASC, id ASC) order */
  ],
  "nextCursor": "…", // present iff the +1 sentinel existed; ABSENT = enumeration complete
}
```

- **No `totalCount`** — it is a second full scan and anvil obtains the exact count
  by accumulating pages. The absent `nextCursor` is the completeness signal the
  `never_invoked` set-difference relies on.
- **Redacted rows excluded by default**, but an **`includeRedacted` flag ships in
  v1**. This resolves the F2 CRITICAL: a `forget()` on a `command.invoked` row
  would otherwise vanish from the list and inflate `never_invoked` (the command
  _was_ invoked). The store's `export_observations` already takes
  `include_redacted`, so the flag is nearly free. Exposing the toggle is mechanism;
  anvil decides whether its counting base includes redacted rows. (adversarial F2)

### Store / service / wiring

- New store method `list_observations(scope, kinds, since, until, cursor, limit,
include_redacted)` — **not** an overload of `query_observations` (which is `ts
DESC`, no cursor, and has live retrieval callers). Mirrors `export_observations`'
  ASC ordering and `push_scope_filters`.
- Service method `list_observations(...)` → store. **No secret masking** (masking
  is a write-path boundary; reads return already-stored content).
- Server handler mirrors `retrieve`: `project_root(headers)` → `service_for(root)`
  → lock → call → unlock before response.
- Client method `Client::list_observations(req) -> ListObservationsResult`.
- New shared types in `kindling-types`: `ListObservationsRequest` /
  `ListObservationsResult` (camelCase; ts-rs bindings; `ts` fields tagged
  `ts(type = "number")`).

### Schema version — no bump

The endpoint is read-only over existing columns and the denormalized scope columns
(migration 004); **zero DDL**, so `schema/version.json` stays **5** and the
client's `EXPECTED_SCHEMA_VERSION` equality check stays green. No new index in v1:
anvil always queries repo-scoped, and `idx_obs_repo_ts(repo_id, ts DESC)` serves
the bounded backward range scan; `kind IN (…)` is a cheap residual filter. A
dedicated `(repo_id, ts ASC, id)` index + migration 006 (which _would_ bump the
schema) is **deferred until profiling proves a filesort**. (architect dec. 6/7;
adversarial F4)

Capability detection: an old daemon returns **404** for the new route (surfaced as
`ClientError::Api { status: 404 }`). anvil gates on daemon/client version; we do
not overload `schemaVersion` for endpoint presence. (adversarial F10)

### Consumer-contract notes (documented for anvil, not kindling bugs)

- **`repo_id` must match append-time values.** The scope filter is exact-match on
  the denormalized `repo_id` column; if anvil's producer writes a different
  `repo_id` string than its reader queries (canonicalization, casing, relative vs
  absolute path), rows silently fall out of scope. anvil owns string consistency.
  (adversarial F5)
- **The list is a daemon-store view only.** Entries still in anvil's _own_ spool
  (un-drained during an outage) are in neither the store nor the list; counts are
  accurate only when anvil's `pending_count == 0`. anvil should flush before
  listing. (adversarial F3)
- **Daemon-down reads have no fallback.** Unlike append, a read returns
  `ClientError::Unavailable`; anvil handles degraded state explicitly.
  (adversarial F21)

---

## Feature 2 — KINTEG-009: spool retention cap

### Config (breaking → client 0.3.0)

Add to `SpoolConfig`:

```rust
#[non_exhaustive]            // NEW — future knobs without a break
pub struct SpoolConfig {
    pub spool_path: PathBuf,
    pub max_bytes:  Option<u64>,   // None = unbounded
    pub max_age_ms: Option<i64>,   // None = unbounded
}

// builder so adding fields never breaks construction
SpoolConfig::new(path)
    .with_max_bytes(64 << 20)
    .with_max_age_ms(7 * 86_400_000);
```

`SpoolConfig` ships at 0.2.0 **without** `#[non_exhaustive]`, so naming new fields
is already a breaking change → bump `kindling-client` **0.2 → 0.3** and add
`#[non_exhaustive]` now. (adversarial F19)

### Defaults — unbounded (opt-in), decided

`max_bytes`/`max_age_ms` default **`None`**. Non-`None` defaults would silently
trim every existing `SpooledClient` caller on upgrade (e.g. a 10-day-offline anvil
draining its backlog). The 7d/64 MiB numbers are **anvil's** policy — anvil wires
them explicitly via the builder. (council split here; adversarial F20 decisive →
D-009 chooses unbounded.)

### Age stamping — add `spooled_at`

Add `spooled_at: Option<i64>` (epoch ms) to `SpoolEntry`, `#[serde(default)]`.
`input.ts` is the _observation_ time (wrong for retention — historical replays
carry old `ts`); the age cap needs the time the entry hit the spool file. Legacy
entries (no `spooled_at`) are **byte-trimmable only**, never age-trimmed.
(adversarial F16)

### Trim algorithm — oldest leading prefix only

Run **inside the existing `flush()` `file_lock`**, as a step of the atomic rewrite
(reuse `temp_sibling` + rename — never a system temp dir, which would `EXDEV` on a
cross-mount rename). (adversarial F18)

1. Read entries in order (append order = drain order).
2. **Age:** drop a leading prefix whose `spooled_at` is older than `now -
max_age_ms` (entries lacking `spooled_at` are skipped by the age rule).
3. **Bytes:** if the serialized remainder still exceeds `max_bytes`, keep dropping
   from the **front** until it fits.
4. A single entry larger than `max_bytes` is **kept** — the byte cap is a
   high-water target, not a hard ceiling; never split or drop a lone un-delivered
   record purely for size. It still ages out. (adversarial F15)
5. Write survivors via the existing `rewrite_spool`.

Dropping **only the contiguous oldest prefix** preserves drain order _by
construction_: drain replays front-to-back; trim removes only from that same
front; an un-drained entry is never dropped while a strictly newer one is kept
ahead of it. This is exactly what #2916's "never drop an entry that could still be
delivered ahead of a kept one" requires.

### When it runs

- **As part of `flush()`** — `flush` already rewrites the remainder under the lock;
  trim the remainder before `rewrite_spool`. Natural, race-free moment.
- **On the append→spool path under sustained outage** — only when the file crosses
  `max_bytes` (cheap `metadata().len()` precheck), do a lock-held read+trim+rewrite
  as a **separate** lock acquisition _outside_ the opportunistic-`flush` lock scope
  (so it cannot deadlock with the flush inside `append_observation`).

**Never** an independent concurrent trim and **never** a CLI `spool trim`
subcommand — an out-of-band trim racing a flush (or a second process) can drop
in-flight entries. Single-producer-per-path stays the v1 invariant (the in-process
`Mutex` does not protect two processes sharing a path; we do not add a cross-process
lock). (adversarial F12, F13)

### Honesty about the contract

Trim is **intentional, bounded retention loss** — distinct from the spool's
existing "never silently drop on flush" guarantee. Under an outage longer than the
window, the oldest un-drained entries _are_ discarded (exactly what the 7d/64 MiB
sidecar it replaces did). #2916's "respect at-least-once" means **don't reorder /
don't drop-newer-while-keeping-older**, which prefix-only trim guarantees — it does
**not** mean infinite retention. Document this loudly in the `flush`/trim rustdoc.
(adversarial F14, F17)

Add `dropped_count` (cumulative) to `SpoolRuntime`/`SpoolStatus`, bumped under the
lock and persisted via the existing best-effort status sidecar, so anvil can
observe that data was shed. The sidecar write stays best-effort and never gates the
trim.

---

## Sequencing

Two independent branches / two PRs (decided).

1. **KINTEG-009 first** — touches only `kindling-client`; unblocks anvil's sidecar
   retirement; ships in the **0.3.0** client bump.
2. **KINTEG-003 second** — types + store + service + server + client; ships after
   009 (can be developed in parallel, not gated on 009 merge).

Both are independent of the in-flight KINTEG-002 (dedup, PR #121) and KINTEG-008
(runtime facade, PR #122). KINTEG-002's id-dedup _complements_ the spool's
at-least-once replay but is not a prerequisite here.

## Test matrix (minimum, per PR)

**KINTEG-009:** trim-by-bytes preserves drain order (oldest dropped); trim-by-age
preserves order; trim under outage then flush still drains remainder; lone entry >
`max_bytes` retained; legacy entry (no `spooled_at`) byte-trimmable but not
age-trimmed; empty-spool trim is a no-op; `dropped_count` increments. Property
test: survivors are always a contiguous oldest-dropped suffix.

**KINTEG-003:** each filter dimension independently (kinds, repoId, since, until);
half-open boundary (row at `until` excluded, row at `since` included); cursor
advances, page 2 disjoint from page 1, empty trailing page terminates with no
`nextCursor`; determinism — an append mid-enumeration with `ts >= pinned until`
does not appear and does not skip a kept row; `includeRedacted` toggles a forgotten
row's presence; `limit` clamped to server max; unknown kind string → 400 not 500;
client round-trip returns the same rows the store holds.

## Out of scope / deferred

- Server-side aggregation rollups (anvil's `top`/`unused`/`flags`/`principals`
  semantics) — policy, stays in anvil.
- `totalCount`, `includeRedacted`-style count reconciliation beyond the v1 flag.
- `(repo_id, ts ASC, id)` index + migration 006 — until profiling proves a
  filesort.
- Cross-process spool lock, spool rotation/archival, CLI `spool trim`.
- A `/v1/observations/count` cheap-count endpoint — separate mechanism-only
  follow-up if anvil ever needs it.
