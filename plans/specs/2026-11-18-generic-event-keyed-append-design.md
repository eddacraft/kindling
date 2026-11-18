# Generic Events and Keyed Durable Append (`generic-event@1`)

| Field      | Value                                                    |
| ---------- | -------------------------------------------------------- |
| Status     | Proposed                                                 |
| Owner      | @aneki                                                   |
| Created    | 2026-11-18                                               |
| APS Module | `plans/modules/06-downstream-integration-surface.aps.md` |
| Work Item  | KINTEG-015 (015a–015d)                                   |
| Council    | plan-9ad9b830                                            |

A planning council (architect, delivery lead, adversarial reviewer, security
analyst, operations reviewer) produced this spec. It records the user's
decisions (Q-00 to Q-13, gates G-1 to G-3) and the four negotiated outcomes
(T1 schema compatibility, T2 wire contract, T3 durability, T4 masking and
export). Release 0.5.0 ships without a release candidate (Q-12), so the wire
contract has to be right first time. Where this spec and an older KINTEG-015
wording disagree, this spec wins (see [Amendments](#amendments-to-the-original-kinteg-015-wording)).

## Context

### Who needs it

anvil's KFIT-005 (`eddacraft/anvil-001`,
`plans/modules/kindling-product-fit.aps.md`) needs governance and audit evidence
events in its `anvil.governance.v1` payload. They must be stored durably and be
retrievable by an exact key after a restart. anvil Track B (KFIT-007, -009,
-010, -011, -013) waits on this release. anvil validates it with a
cross-repository fixture that commits one keyed opaque event, restarts the
runtime and retrieves the exact row.

kindling has to supply, through `kindling-runtime`:

1. a versioned outer envelope around an opaque, size-bounded payload with a
   consumer-declared schema name and version, where kindling validates only the
   envelope;
2. a keyed append that reserves the key, canonical scope and payload digest
   atomically, returns a receipt only after a durable commit, is idempotent for
   a same-key retry and reports a conflict on a mismatch;
3. exact lookup by key after a restart;
4. a closed failure set that never falls back to the spool;
5. capability negotiation, in which 0.4.x answers "unsupported".

### What exists today (v0.4.1)

| Area             | Today                                                                                                                                                                                | Code reference                                                                              |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------- |
| Storage layout   | One SQLite DB per project at `<home>/projects/<sha256(root)[..12]>/kindling.db`. The root string is hashed raw, without canonicalisation.                                            | `crates/kindling-store/src/paths.rs:15-37`                                                  |
| Daemon routing   | `X-Kindling-Project` routes requests and is not an authentication mechanism. One `Arc<Mutex<KindlingService>>` per project serialises writes.                                        | `crates/kindling-server/src/state.rs:18-20,60-90`                                           |
| Durability       | Every connection runs WAL with `synchronous=NORMAL` and a 5 s `busy_timeout`. There is no checkpoint tuning.                                                                         | `crates/kindling-store/src/db.rs:158-166`                                                   |
| Schema gate      | `user_version` above the supported version gives `SchemaTooNew`. Migrations run in place with no down path. `version.json` = `{version: 6, minCompatible: 1}`.                       | `db.rs:172-207`, `schema/version.json`                                                      |
| Client gate      | `Client::health` fails on any `schemaVersion != 6`. `EXPECTED_SCHEMA_VERSION` is parsed from the first `"version"` key in `version.json`. The TS client has the same equality check. | `crates/kindling-client/src/config.rs:20-40`, `packages/kindling/src/client.ts:105`         |
| Masking          | Every append masks content, provenance and `scope_ids`, even with `validate: false`. Dedup is id-based `INSERT OR IGNORE` with no content comparison.                                | `crates/kindling-service/src/service.rs:293-347`, `kindling-store/src/store.rs:71-95`       |
| Detectors        | Service has 7 regexes, 6 with known prefixes plus a substring `credentialAssignment` rule. The hook filter is a second, drifted detector with a 32+ alnum heuristic.                 | `crates/kindling-service/src/filter/secrets.rs`, `crates/kindling/src/hook/filter.rs:60-90` |
| Forget           | Soft redaction: `content='[redacted]', redacted=1`.                                                                                                                                  | `store.rs:303-307`                                                                          |
| Export/import    | Bundle `"1.0"`. Import is id-based, with no key or digest checks, and can partially apply.                                                                                           | `crates/kindling-service/src/export.rs`                                                     |
| Kinds and FTS    | `observations.kind` is a hard-coded CHECK of 9 kinds. FTS triggers index every observation.                                                                                          | `schema/schema.sql:37-60,145-162`                                                           |
| Capability       | `{version, schemaVersion, supportedKinds, storagePath, kindRegistry}` with no feature flags. Served on unauthenticated `GET /v1/health`.                                             | `crates/kindling-types/src/capability.rs:28-38`                                             |
| Errors           | `ApiError` has 400/404/409/500 only, and store errors become a free-text 500. `ClientError` = Unavailable/Http/InvalidRequest/Api/SchemaMismatch/Decode/Io.                          | `crates/kindling-server/src/error.rs:33-55`, `crates/kindling-client/src/error.rs`          |
| Client deadlines | Only the connect phase is bounded. There is no per-request deadline.                                                                                                                 | `crates/kindling-client/src/transport.rs:181-260`                                           |
| Spool            | `SpooledClient` returns `AppendOutcome::Spooled` on an outage. Spool writes call `flush()` only, with no fsync.                                                                      | `crates/kindling-client/src/spool.rs:216-227,576-582`                                       |
| Runtime          | Attaches to any daemon already on the socket. `SpawnStrategy` only governs the no-daemon case, and the embedded daemon is an in-process tokio task.                                  | `crates/kindling-runtime/src/lib.rs:57-65,99-113,241-347`                                   |
| Daemon runtime   | `kindling serve` runs on the shared `current_thread` runtime. Handlers do SQLite inline under a std `Mutex`.                                                                         | `crates/kindling/src/lib.rs:221-225`, `commands/serve.rs:33`                                |
| Disk/quota       | No quota, statvfs, `SQLITE_FULL` or `ENOSPC` handling anywhere.                                                                                                                      | —                                                                                           |
| Socket           | UDS socket 0600 in a 0700 directory. No bearer auth on UDS.                                                                                                                          | `crates/kindling-server/src/lib.rs:255-266`                                                 |
| Dependencies     | `sha2` is already used. `base64 0.22.1` is already in `Cargo.lock`.                                                                                                                  | `paths.rs:12`, `Cargo.lock`                                                                 |

## Goals

- Ship `generic-event@1` and keyed durable append in **final 0.5.0**, through
  `kindling-runtime`, `kindling-client` and three new v1 endpoints.
- A receipt means the row is durable. Same-key replay is idempotent and
  byte-identical, and every failure falls in a closed, exhaustively matchable
  set that never touches the spool.
- 0.4.x binaries keep opening 0.5 databases and keep working with a 0.5 daemon.
- Ordinary append keeps its KINTEG-013/014 budgets.
- Define the contract precisely enough (golden digest vectors, detector
  fixtures, a code table) that anvil can test against it in CI without reading
  kindling source.

## Non-goals

- Interpreting the payload, policy or anvil-specific kinds. anvil owns
  `anvil.governance.v1`.
- Unkeyed generic events, listing, pagination or batch append. Each can be
  added later as a separate feature entry, such as `generic-event-list@1`.
- A TypeScript client method, a CLI verb or an embedded-service-only API (Q-03).
  Only the ts-rs bindings are regenerated.
- FTS indexing, or appearance in retrieve, inject or list.
- Tamper-proof attestation. The digest is an unkeyed SHA-256 integrity check,
  and anvil signs if it needs proof.
- Physical erasure on forget, a consumer allow-list for the detector, retention
  or prune changes, cloud or sync.

## Decisions

| #   | Decision                  | Choice                                                                                                                                                     | Rationale                                                                                                       |
| --- | ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| 1   | Key domain (Q-01)         | `UNIQUE(schema_name, key)` per project DB. `projectId` is part of the canonical scope.                                                                     | One DB per project already. A profile-wide index would need a cross-DB registry.                                |
| 2   | Storage (Q-02)            | Separate `generic_events` table, outside FTS, retrieve, inject and list, but included in export/import.                                                    | Avoids touching the `kind` CHECK and FTS triggers, and keeps opaque bytes out of retrieval.                     |
| 3   | Surfaces (Q-03)           | v1 endpoints plus `kindling-client` methods plus a `kindling-runtime` facade. ts-rs regenerates.                                                           | anvil is a Rust consumer. Every extra surface is contract that ships without an RC.                             |
| 4   | Capability (Q-00)         | Additive `features` list on `/v1/health`. Absence means unsupported, and the server re-checks on write.                                                    | 0.4 health bodies parse unchanged, and 0.4 daemons are detected as unsupported.                                 |
| 5   | Secrets (Q-04)            | Read-only detector. A match returns `RejectedPayload`, and stored bytes are never mutated.                                                                 | Masking would break the digest and idempotency.                                                                 |
| 6   | Forget (Q-05)             | Tombstone: the payload is cleared and key, scope, digest and id are kept. Lookup returns `redacted`, and a later same-key append is a `Redacted` conflict. | The key stays reserved, so a forgotten event cannot be silently re-created.                                     |
| 7   | Digest (Q-06)             | SHA-256 over a fixed framing of schema, scope and raw payload, tagged `sha256:`. The key is excluded.                                                      | Gives a deterministic, language-neutral idempotency check. It is not attestation.                               |
| 8   | Import (Q-13)             | A full match is skipped. A mismatch fails the whole import and names the key. No partial write.                                                            | Q-13 forbids partial writes.                                                                                    |
| 9   | Durability (Q-07)         | `synchronous=FULL` around keyed transactions only. Ordinary append stays NORMAL.                                                                           | Keeps the ordinary p95 budget. A mixed bench workload proves it.                                                |
| 10  | Ambiguity (Q-08, **G-1**) | Per-attempt deadline. After an ambiguous outcome, **one** same-key re-append, then `Indeterminate`. This replaces the Q-08 lookup.                         | The re-append queues behind the original and resolves strictly more cases than a lookup.                        |
| 11  | Quota/disk (Q-09)         | Fixed max payload, an optional per-project byte cap and a low-disk floor. `SQLITE_FULL`/`ENOSPC` map to `LowDisk`. Errors use typed codes.                 | A free-text 500 cannot be matched exhaustively.                                                                 |
| 12  | Schema (Q-10)             | Additive revision 7 with a `minReader` floor. The client check is relaxed to a range, and schema/README.md changes.                                        | 0.4.x must keep opening 0.5 databases.                                                                          |
| 13  | Phasing (Q-11)            | KINTEG-015a–d as sequential PRs, released together in 0.5.0 behind one capability.                                                                         | Small reviewable PRs, one contract.                                                                             |
| 14  | Release (Q-12)            | Final 0.5.0 via `scripts/publish.sh`. No RC, and not coupled to C14.                                                                                       | C14 is blocked, and anvil Track B is waiting.                                                                   |
| 15  | Bench gate (**G-2**)      | Release-prep gate: closed-loop ordinary p95 < 1 ms with keyed at 5/s, plus replay p95 < 1 ms. The sweep is reported only.                                  | 50/s fails by arithmetic (lock occupancy), and 5/s covers anvil's evidence rate.                                |
| 16  | Consequences (**G-3**)    | Forget endpoint ships in 015d. The macOS durability caveat is documented.                                                                                  | The `redacted` state and `key_redacted` are in `@1`, so they need a live write path.                            |
| 17  | Version stamps (T1)       | `user_version` = `minReader` = 6. Revision = `MAX(schema_migrations.version)` = 7. New `wireVersion` = 6.                                                  | 0.4 refuses `user_version > 6`. Three numbers separate DB-open, revision and wire compatibility.                |
| 18  | Client range (T1)         | Rust and TS clients accept `6 <= schemaVersion <= MAX_WIRE_VERSION` (6). The `"version"` parse trap is fixed in both.                                      | Keeps an upper safety bound while tolerating additive releases.                                                 |
| 19  | Bundle version (T1/T4)    | `"1.0"` (byte-identical to 0.4) without generic events, `"2.0"` with them, and tombstones count.                                                           | 0.4 would silently half-import a `"1.0"` bundle that carries `genericEvents`.                                   |
| 20  | Envelope (T2)             | Strict grammars. Payload is padded base64 (RFC 4648 §4).                                                                                                   | A JSON-value payload would change bytes on re-serialisation.                                                    |
| 21  | Limits (T2)               | `maxPayloadBytes` 262144 in 0.5.0, with a 65536 floor for `@1`. Raising the max is additive.                                                               | Leaves room for future tuning without needing `@2`.                                                             |
| 22  | Key transport (T2)        | Keys go in POST bodies, including lookup and forget.                                                                                                       | Avoids path encoding and keeps keys out of URL logs. Mirrors `/v1/observations/list`.                           |
| 23  | Receipt (T2)              | Byte-identical on every replay and lookup. No `replayed`, `payloadBytes` or durability fields. 201 vs 200 signals replay.                                  | Smaller contract, and fields can be added later.                                                                |
| 24  | `projectId` (T2)          | The id of the DB where the row was committed (`sha256(root)[..12]`). Imported rows keep theirs.                                                            | Keeps digests verifiable across export and import.                                                              |
| 25  | Enums (T2)                | Top-level outcome enums are exhaustive. Detail enums are `#[non_exhaustive]`.                                                                              | anvil matches every outcome, and detail can grow.                                                               |
| 26  | Busy and commit (T2/T3)   | `StoreBusy` folds into `FailedCommit{Busy}`. A COMMIT-step I/O error where the re-check finds nothing gives `commit_indeterminate`.                        | Keeps one closed variant per meaning. Only a true unknown is ambiguous.                                         |
| 27  | Error body (T2)           | `{error, code, details}` on generic-event routes only.                                                                                                     | No retrofit of existing endpoints, and the TS client reads only `error`.                                        |
| 28  | Durable write (T3)        | Pragma toggle on the shared connection before `BEGIN IMMEDIATE`. `checkpoint_fullfsync=ON` on all writable connections.                                    | A second connection would spin on the WAL lock, and a macOS checkpoint could otherwise drop a receipted row.    |
| 29  | Concurrency (T3)          | `kindling serve` gets a multi-thread runtime. The keyed handler runs in `spawn_blocking` behind a per-project `Semaphore(1)`.                              | An inline fsync would stall every project and `/v1/health`. Ordinary writes wait for at most one fsync.         |
| 30  | Usage counter (T3)        | Trigger-maintained `generic_event_usage` row ships in revision 7, with the cap off by default.                                                             | O(1), correct across processes, and avoids a revision 8 later.                                                  |
| 31  | Crash tests (T3)          | `test-failpoints` cargo feature with `KINDLING_FAILPOINT` aborts. Power-loss durability holds by construction and is checked by a pragma-state test.       | Hosted CI cannot simulate power loss honestly.                                                                  |
| 32  | Observability (T3)        | `AtomicU64` counters in `AppState` and stderr logs. Not on Capability.                                                                                     | Exposing them is a follow-up.                                                                                   |
| 33  | Detector (T4)             | One rule table with two profiles. Observation masking is unchanged. New `GenericEventDetector` v1.                                                         | Removes false positives on digests and SHAs without changing observation behaviour.                             |
| 34  | Detection order (T4)      | Detection counts only on a key miss. `detector_version` is stored per row, outside the digest.                                                             | A rule change in 0.5.x can never break idempotent replay.                                                       |
| 35  | No allow-list (T4)        | No consumer exemptions in 0.5.0. Recovery is to rename the field and retry with the same key.                                                              | A rejection reserves nothing, and fixtures make the detector testable.                                          |
| 36  | Export/import (T4)        | Tombstones are always exported. Import is all-or-nothing, recomputes digests and does not re-scan.                                                         | Old backups stay restorable after a detector bump.                                                              |
| 37  | Erasure (T4)              | Forget is logical, and docs never claim erasure. `secure_delete` plus checkpoint is deferred.                                                              | TRUNCATE inside the mutex blocks writers, and connection-wide `secure_delete` taxes ordinary appends.           |
| 38  | Recovery mapping (synth.) | During recovery, only a receipt, `Conflict`, a 4xx rejection or `Unsupported` is terminal. Everything else gives `Indeterminate`.                          | The original may still be in flight, so a 503, 507 or 500 on the recovery attempt cannot prove "not committed". |
| 39  | Reopen failure (synth.)   | If the service cannot be reopened after a COMMIT-step I/O error, the result is `commit_indeterminate`, not `commit_failed`.                                | The re-check cannot run, so "definitely not committed" cannot be claimed.                                       |

Rows 38 and 39 resolve gaps between T2 and T3. They tighten the T2 not-committed
invariant and do not loosen it.

## Storage and schema compatibility (T1)

### Tables

`generic_events` (new, revision 7):

| Column                                                    | Type         | Notes                                                                     |
| --------------------------------------------------------- | ------------ | ------------------------------------------------------------------------- |
| `event_id`                                                | TEXT PK      | Server UUIDv4                                                             |
| `envelope_version`                                        | INTEGER      | `1`                                                                       |
| `schema_name`, `event_key`                                | TEXT         | `UNIQUE(schema_name, event_key)`. The key is compared byte-for-byte.      |
| `schema_version`                                          | INTEGER      | u32 ≥ 1                                                                   |
| `project_id`                                              | TEXT         | 12 lowercase hex                                                          |
| `session_id`, `repo_id`, `agent_id`, `user_id`, `task_id` | TEXT NULL    | Canonical scope. NULL means absent.                                       |
| `digest`                                                  | TEXT         | `sha256:<64 lowercase hex>`                                               |
| `payload`                                                 | BLOB NULL    | Raw bytes. NULL once tombstoned.                                          |
| `committed_at`                                            | INTEGER      | ms since epoch                                                            |
| `redacted_at`                                             | INTEGER NULL | Set by forget. One-way.                                                   |
| `detector_version`                                        | INTEGER      | Detector version that screened the row. Not in the digest or the receipt. |

`generic_event_usage` (new): a single row (`id = 1`, checked) with
`payload_bytes` and `event_count`. It is seeded by migration 007 and maintained
by `AFTER INSERT`, `AFTER UPDATE` (tombstone subtracts the cleared bytes,
measured as `length(CAST(payload AS BLOB))`) and `AFTER DELETE` triggers on
`generic_events`. Imports are covered by the same triggers.

`generic_events` has **no FK, trigger or CHECK coupling to any existing table**,
and no FTS. Every INSERT, UPDATE and DELETE that a 0.4 binary issues stays valid
on a revision-7 DB. A golden test enforces this.

### Version stamps

| Stamp                                                    | Value     | Meaning                                                                                                           |
| -------------------------------------------------------- | --------- | ----------------------------------------------------------------------------------------------------------------- |
| `PRAGMA user_version`                                    | 6         | **minReader**: the lowest schema revision a binary must know to open the DB. Writers stamp it and never lower it. |
| `MAX(schema_migrations.version)`                         | 7         | **Schema revision**                                                                                               |
| `version.json` `wireVersion`                             | 6         | Wire compatibility level reported as Capability `schemaVersion`. Rises only on a breaking wire change.            |
| `version.json` `version` / `minReader` / `minCompatible` | 7 / 6 / 1 | Revision this source knows, floor it stamps, and oldest DB it can migrate. `ftsTokenizer` is unchanged.           |

### Migration and open rules (0.5)

| DB state on open                  | Writable open            | Read-only open                                       |
| --------------------------------- | ------------------------ | ---------------------------------------------------- |
| `user_version = 0`                | Refuse (`SchemaTooOld`)  | Refuse                                               |
| `user_version = 5`                | 006 (sets 6), then 007   | Unchanged from 0.4                                   |
| `user_version = 6`, row 7 missing | 007 in `BEGIN IMMEDIATE` | Open, no migration. Generic reads return `notFound`. |
| `user_version = 6`, row 7 present | Open                     | Open                                                 |
| `user_version = 7`                | Open                     | Open                                                 |
| `user_version > 7`                | Refuse (`SchemaTooNew`)  | Refuse                                               |
| Unknown extra tables              | Ignored                  | Ignored                                              |

Migration 007 runs `CREATE TABLE/INDEX/TRIGGER IF NOT EXISTS`, seeds the usage
row and `INSERT OR IGNORE`s `schema_migrations` row 7. It **never writes
`user_version`**. It is idempotent when 0.5 processes race, and a concurrent
0.4 daemon re-prepares on `SQLITE_SCHEMA`. `schema.sql` (fresh DBs) seeds row 7
and `user_version = 6`.

**Additive rule** (new in `schema/README.md`): a revision is additive only if
every older writer's INSERT, UPDATE and DELETE stays valid on every table,
including new ones. Otherwise the revision must raise `minReader`
(`user_version`).

### Wire version helpers

`schemaVersion` on `/v1/health` (`crates/kindling-server/src/handlers.rs:60`)
and in `kindling status --json` (`crates/kindling/src/commands/status.rs:62`)
both come from one helper, such as `kindling_store::wire_version()`, which reads
`wireVersion`. The `const` parser in `crates/kindling-client/src/config.rs:31`
and the TS reader in `packages/kindling/src/config.ts:47` read `wireVersion`
explicitly, not the first `"version"` key. `EXPECTED_SCHEMA_VERSION` and
`ClientConfig::expected_schema_version` keep their names for anvil compile
compatibility, but now mean the **lower bound**. Const asserts pin
`EXPECTED_SCHEMA_VERSION == 6` and `KNOWN_SCHEMA_REVISION == 7`. Additive
features never raise `wireVersion`. `scripts/sync-vendored-schema.sh` updates
every vendored copy of `version.json`.

## Wire contract (T2)

### Envelope (`generic-event@1`)

```jsonc
{
  "envelopeVersion": 1, // must equal 1; else 400 unsupported_envelope_version
  "schemaName": "example.audit", // ^[a-z0-9][a-z0-9._-]{0,127}$
  "schemaVersion": 3, // u32 >= 1
  "key": "kfit/run-42/evt-7", // ^[\x21-\x7E]{1,256}$, compared byte-for-byte
  "scopeIds": { "sessionId": "…", "repoId": "…" }, // ScopeIds; each 1..=256 bytes, no control chars, "" rejected
  "payload": "eyJrIjoidiJ9", // padded base64, RFC 4648 §4, strict; 1..=maxPayloadBytes decoded
}
```

- `projectId` is not in the envelope. The daemon derives it from the routing
  root with `paths::project_id`, so it is the directory name of the DB it writes.
- Scope is never masked on this path. The detector screens it read-only (see
  [Secret detection](#secret-detection-forget-and-exportimport-t4)).
- The route body limit is `4 * ceil(maxPayloadBytes / 3) + 16 KiB`. A request
  over that limit gets a 413 from axum with no body.

### Endpoints

All three are POST routes that take a JSON body, require `X-Kindling-Project`,
and are unauthenticated on UDS (same-UID trust boundary).

| Route                            | Body                | Success                                                       | Ships in |
| -------------------------------- | ------------------- | ------------------------------------------------------------- | -------- |
| `POST /v1/generic-events`        | envelope            | **201** receipt (new row) / **200** original receipt (replay) | 015c     |
| `POST /v1/generic-events/lookup` | `{schemaName, key}` | **200** `found` \| `redacted` \| `notFound`                   | 015c     |
| `POST /v1/generic-events/forget` | `{schemaName, key}` | **200** `redacted` \| `notFound`. Idempotent.                 | 015d     |

`notFound` is a normal 200 result. A bare 404 always means the route does not
exist, which a 0.4 daemon returns and the client reads as unsupported.

### Receipt and lookup/forget states

```jsonc
// GenericEventReceipt: byte-identical on every replay, lookup and forget
{
  "eventId": "9b2f…", // server UUIDv4
  "envelopeVersion": 1,
  "schemaName": "example.audit",
  "schemaVersion": 3,
  "key": "kfit/run-42/evt-7",
  "scope": { "projectId": "0123456789ab", "sessionId": "…", "repoId": "…" }, // absent fields omitted
  "digest": "sha256:<64 lowercase hex>",
  "committedAt": 1763452800000, // ms
}

// GenericEventLookup (tagged on "state")
{ "state": "found", "receipt": { … }, "payload": "<base64>" }
{ "state": "redacted", "receipt": { … }, "redactedAt": 1763456400000 }
{ "state": "notFound" }

// GenericEventForget (tagged on "state"); redactedAt is the ORIGINAL time on repeats
{ "state": "redacted", "receipt": { … }, "redactedAt": 1763456400000 }
{ "state": "notFound" }
```

### Digest framing

`digest = "sha256:" + lowercase_hex(SHA-256(M))`, where `M` is:

| Order | Field                                                                          | Encoding                                                                              |
| ----- | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| 1     | Domain tag                                                                     | 26 ASCII bytes `kindling.generic-event.v1` followed by `0x00`                         |
| 2     | `schemaName`                                                                   | `u32be len` ‖ UTF-8 bytes                                                             |
| 3     | `schemaVersion`                                                                | `u32be` value                                                                         |
| 4–9   | `projectId`, `sessionId`, `repoId`, `agentId`, `userId`, `taskId` (this order) | `u32be len` ‖ UTF-8 bytes. `len = 0` means absent (`""` is rejected at the envelope). |
| 10    | payload                                                                        | `u32be len` ‖ raw decoded bytes                                                       |

- **The key is excluded**, because it is identity, not content.
  `envelopeVersion` is implied by the domain tag. `detector_version` is
  excluded.
- The digest is computed **server-side only**. There is no public helper.
- Golden vectors live in `schema/fixtures/generic-event-digest-v1.json` (015a)
  and are asserted in CI. The reference values below were computed from this
  layout. Once merged, the fixture file is authoritative.

| Vector      | Inputs                                                                                                                   | `M` bytes | Digest                                                                    |
| ----------- | ------------------------------------------------------------------------------------------------------------------------ | --------- | ------------------------------------------------------------------------- |
| minimal     | `schemaName "a"`, `schemaVersion 1`, `projectId "000000000000"`, other scope absent, payload `00`                        | 76        | `sha256:43852a2e7ef09166831bb33fce56d5ad008c97be6e8d3770326316b1d6b48c6f` |
| all scope   | `"example.audit"`, `3`, `projectId "0123456789ab"`, `sess-1`/`repo-1`/`agent-1`/`user-1`/`task-1`, payload `{"k":"v"}`   | 127       | `sha256:d4391e955ce4cf16ae13fccb7bacb97f4263415f956f599c4e4beef35de9e57d` |
| max payload | `"x"×128`, `4294967295`, `projectId "ffffffffffff"`, other scope absent, payload 262144 bytes where byte `i = i mod 251` | 262346    | `sha256:cc16e8457b2d01706b41c43ac52af81f45132fe3ea7e9c8cca2124154b794457` |

`M` for the minimal vector is
`6b696e646c696e672e67656e657269632d6576656e742e763100 00000001 61 00000001 0000000c 303030303030303030303030 00000000×5 00000001 00`.

Import into a different project keeps the original `projectId`, so the digest
still verifies. A later same-key append in the destination carries the
destination's `projectId` and gets `Conflict(Mismatch)`. `/repo` and `/repo/`
route to different DBs. That routing issue already exists and is documented
here, not fixed.

### Rust types

Wire shapes live in `kindling-types` (serde camelCase, ts-rs). The closed
outcome enums live in `kindling-client` and are re-exported by
`kindling-runtime`, because they encode transport outcomes.

```rust
// kindling-types
pub struct GenericEvent { envelope_version: u32, schema_name: String, schema_version: u32,
                          key: String, scope_ids: ScopeIds, payload: Vec<u8> /* base64 on wire */ }
pub struct GenericEventScope { project_id: String, session_id: Option<String>, repo_id: Option<String>,
                               agent_id: Option<String>, user_id: Option<String>, task_id: Option<String> }
pub struct GenericEventReceipt { event_id: String, envelope_version: u32, schema_name: String,
                                 schema_version: u32, key: String, scope: GenericEventScope,
                                 digest: String, committed_at: i64 }
pub enum GenericEventLookup { Found { receipt, payload: Vec<u8> }, Redacted { receipt, redacted_at: i64 }, NotFound }
pub enum GenericEventForget { Redacted { receipt, redacted_at: i64 }, NotFound }
pub struct GenericEventLimits { max_payload_bytes: u64, max_key_bytes: u32, max_schema_name_bytes: u32,
                                project_quota_bytes: Option<u64>, secret_detector_version: u32 }
#[non_exhaustive] pub enum RejectReason { InvalidEnvelope { field: String, message: String },
    TooLarge { limit: u64, actual: u64 },
    SecretDetected { field: String, classes: Vec<String>, count: u32, detector_version: u32 },
    Unrecognized { code: String, message: String } }
#[non_exhaustive] pub enum ConflictKind { Mismatch { existing: GenericEventReceipt },
    Redacted { existing: GenericEventReceipt, redacted_at: i64 } }
#[non_exhaustive] pub enum CommitFailure { Busy, Storage }

// kindling-client (re-exported by kindling-runtime): exhaustive, NOT non_exhaustive
pub enum AppendEventError { Unsupported { detail: String }, Unavailable { message: String } /* pre-send */,
    RejectedPayload(RejectReason), Conflict(ConflictKind), Quota { limit_bytes: u64, used_bytes: u64 },
    LowDisk { free_bytes: u64, floor_bytes: u64 }, FailedCommit { kind: CommitFailure, message: String },
    Indeterminate { message: String } /* retry-safe with the same key */ }
pub enum LookupEventError { Unsupported { detail: String }, Unavailable { message: String },
    InvalidRequest { code: String, message: String }, FailedLookup { message: String } }
pub enum ForgetEventError { Unsupported { detail: String }, Unavailable { message: String },
    InvalidRequest { code: String, message: String }, FailedCommit { kind: CommitFailure, message: String } }
```

For `TooLarge`, `actual` is always the caller's decoded payload length, which
the client knows locally. `limit` comes from the response details, or from the
advertised limits when the 413 has no body.

### Error codes

The error body is `{ "error": "<text>", "code": "<code>", "details": { … } }` on
generic-event routes only. Details are camelCase, and an error never echoes
payload text.

| Code                                        | HTTP | Routes         | `details`                                  | Append maps to                                   | Lookup / forget map to                                                               |
| ------------------------------------------- | ---- | -------------- | ------------------------------------------ | ------------------------------------------------ | ------------------------------------------------------------------------------------ |
| `invalid_envelope`                          | 400  | all            | `{field}`                                  | `RejectedPayload(InvalidEnvelope)`               | `InvalidRequest`                                                                     |
| `missing_project`                           | 400  | all            | —                                          | `RejectedPayload(InvalidEnvelope)`               | `InvalidRequest`                                                                     |
| `unsupported_envelope_version`              | 400  | append         | `{supported:[1]}`                          | `Unsupported`                                    | —                                                                                    |
| `payload_too_large` / bare 413              | 413  | append         | `{limit, actual}` (absent on bare 413)     | `RejectedPayload(TooLarge)`                      | —                                                                                    |
| `secret_detected`                           | 422  | append         | `{field, classes, count, detectorVersion}` | `RejectedPayload(SecretDetected)`                | —                                                                                    |
| `key_conflict`                              | 409  | append         | `{existing}`                               | `Conflict(Mismatch)`                             | —                                                                                    |
| `key_redacted`                              | 409  | append         | `{existing, redactedAt}`                   | `Conflict(Redacted)`, even if the digest matches | —                                                                                    |
| `quota_exceeded`                            | 507  | append         | `{limitBytes, usedBytes}`                  | `Quota`                                          | —                                                                                    |
| `low_disk`                                  | 507  | append         | `{freeBytes, floorBytes}`                  | `LowDisk`                                        | —                                                                                    |
| `store_busy`                                | 503  | append, forget | —                                          | `FailedCommit{Busy}`                             | forget: `FailedCommit{Busy}`                                                         |
| `commit_failed`                             | 500  | append, forget | —                                          | `FailedCommit{Storage}`                          | forget: `FailedCommit{Storage}`                                                      |
| `commit_indeterminate`                      | 500  | append         | —                                          | → recovery                                       | —                                                                                    |
| `lookup_failed`                             | 500  | lookup         | —                                          | —                                                | `FailedLookup`                                                                       |
| bare 404 / 405                              | —    | all            | —                                          | `Unsupported`                                    | `Unsupported`                                                                        |
| unknown 4xx                                 | —    | all            | —                                          | `RejectedPayload(Unrecognized)`                  | `InvalidRequest`                                                                     |
| unknown 5xx, transport after send, deadline | —    | all            | —                                          | → recovery                                       | lookup: `FailedLookup`; forget: one idempotent re-send, then `FailedCommit{Storage}` |
| connect failure before send                 | —    | all            | —                                          | `Unavailable`                                    | `Unavailable`                                                                        |

**Not-committed invariant (append):** every 4xx, `503 store_busy`, any 507 and
`500 commit_failed` that the server issues to a first attempt means **this
request committed nothing**. `commit_failed` is issued only after the server
re-checks the key under the lock and finds it absent. No append failure ever
returns a receipt, writes to the spool or reports a spooled outcome as
retained. For forget, server-issued codes carry the same guarantee. A
`FailedCommit` that the client synthesises after a transport failure means the
outcome is unknown. Because forget is idempotent, the caller can retry or
confirm with lookup.

### Capability

```jsonc
// GET /v1/health (additions only; every new field has serde(default))
{
  "schemaVersion": 6, // = wireVersion
  "schemaRevision": 7,
  "features": [
    {
      "name": "generic-event",
      "version": 1,
      "limits": {
        "maxPayloadBytes": 262144,
        "maxKeyBytes": 256,
        "maxSchemaNameBytes": 128,
        "projectQuotaBytes": 1073741824, // omitted when no cap is configured
        "secretDetectorVersion": 1,
      },
    },
  ],
}
```

- No `deny_unknown_fields`. An unknown feature entry, or an unknown `limits`
  shape on any entry, must never fail health parsing. The client types `limits`
  as `GenericEventLimits` only for `name = "generic-event", version = 1`, and
  otherwise keeps the entry with no limits. A golden test pins the shape.
- One feature entry covers append, lookup and forget. A 0.5.0 daemon advertises
  it only once 015d is merged, and nothing is advertised before then.
- The runtime checks the feature before the first write and after any
  reconnect. It pre-validates limits locally, and the server re-checks
  everything. A missing feature or a 404/405 gives `Unsupported`.

### Versioning: what needs `@2`

| Needs `generic-event@2` (and a new 0.x minor)                  | Additive within `@1`                                   |
| -------------------------------------------------------------- | ------------------------------------------------------ |
| Any change to the digest framing or payload encoding           | New optional response fields                           |
| Narrowing a charset or range, or `maxPayloadBytes` below 65536 | New `limits` keys. Raising `maxPayloadBytes`.          |
| Widening the key or `schemaName` charset                       | New 4xx codes (they fall into `Unrecognized`)          |
| Changing idempotency, uniqueness or conflict semantics         | New detail-enum variants (`#[non_exhaustive]`)         |
| A new lookup/forget state or a new top-level error variant     | New feature entries (`generic-event-list@1`, batch, …) |
| Changing the status↔code mapping                               | A detector rule change (bumps `secretDetectorVersion`) |

## Durability, concurrency and failure handling (T3)

### `durable_write`

`SqliteKindlingStore::durable_write(|tx| …)` runs these steps on the project's
existing connection:

1. Set `synchronous=FULL`, plus `fullfsync=ON` on macOS. Read
   `PRAGMA synchronous` back and abort before BEGIN unless it is `2`.
2. Set `busy_timeout` to 1 s, then `BEGIN IMMEDIATE`.
3. Run the closure, then COMMIT. In WAL mode, FULL fsyncs the WAL on commit,
   which also makes any earlier NORMAL frames durable.
4. A guard restores `NORMAL`, `fullfsync=OFF` and the 5 s busy timeout on every
   path: success, error and panic. If the restore fails, the guard logs it and
   increments a counter only. FULL is merely slower, so no eviction is needed.

Every transaction that writes `generic_events` (keyed append, import with
generic events, forget-tombstone) uses `durable_write`. `configure()`
(`db.rs:158`) sets `checkpoint_fullfsync=ON` on every writable connection.
Otherwise a macOS auto-checkpoint triggered by an ordinary commit could copy a
keyed row with a plain fsync and reset the WAL. On Linux both fullfsync pragmas
are no-ops.

### Keyed append sequence

| Step | Where            | Action                                                                                                                                                                                             |
| ---- | ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | Handler, no lock | Validate the envelope and sizes, run the detector, compute the canonical scope and digest, and refresh the free-space cache if it is more than 5 s old.                                            |
| 2    | Handler          | Acquire the per-project keyed `Semaphore(1)` asynchronously.                                                                                                                                       |
| 3    | `spawn_blocking` | Lock the project's std `Mutex`, then `durable_write { BEGIN IMMEDIATE`.                                                                                                                            |
| 4    | In txn           | `SELECT` by `(schema_name, key)`. Same scope and digest returns the stored receipt (200). A mismatch returns `key_conflict`. A tombstone returns `key_redacted`. All three ROLLBACK with no fsync. |
| 5    | In txn, key miss | If the detector hit, return `secret_detected` and ROLLBACK. Nothing is reserved.                                                                                                                   |
| 6    | In txn           | Quota check (usage row + payload > cap gives `quota_exceeded`), then the cached free-space floor (`low_disk`).                                                                                     |
| 7    | In txn           | INSERT (the trigger updates usage), then COMMIT (WAL frames, fsync, publish), then restore the pragmas and return 201.                                                                             |

Quota and disk checks come **after** the key lookup, so an exact replay still
gets its receipt when the project is over quota or low on disk. A row becomes
visible only after its sync, so the match path never needs an fsync.

### Server error handling

| Failure                                                 | Server action                                                                                                                     | Code                                                        |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `SQLITE_BUSY` before any write (1 s keyed busy timeout) | ROLLBACK                                                                                                                          | `store_busy` 503                                            |
| `SQLITE_FULL` / `ENOSPC`                                | ROLLBACK, then re-check the key under the lock                                                                                    | `low_disk` 507, or the original receipt if found            |
| Other error before COMMIT                               | ROLLBACK, then re-check under the lock                                                                                            | `commit_failed` 500, or the original receipt if found       |
| I/O error (incl. fsync EIO) at the COMMIT step          | Reopen the service in place under the lock, then re-check the key                                                                 | Original receipt if found, otherwise `commit_indeterminate` |
| Reopen fails                                            | Leave the slot for the next request to reopen                                                                                     | `commit_indeterminate` (decision 39)                        |
| Panic in the keyed closure                              | Caught with `catch_unwind`. A poisoned lock is recovered with `into_inner()`, the service is reopened, and the key is re-checked. | Original receipt if found, otherwise `commit_indeterminate` |

### Crash windows

| Window | Crash point                      | On disk after restart                                 | Client saw                 | Resolution                                                                      |
| ------ | -------------------------------- | ----------------------------------------------------- | -------------------------- | ------------------------------------------------------------------------------- |
| W0     | Before COMMIT                    | Nothing                                               | Transport error            | Recovery re-append inserts the row (201).                                       |
| W1     | WAL frames written, before fsync | Kept after a process crash, may be lost on power loss | Transport error            | Recovery returns the replay receipt (200) or inserts fresh (201).               |
| W2     | After fsync, before reply        | Durable                                               | Transport error / deadline | Recovery returns the original receipt (200).                                    |
| W3     | fsync EIO at COMMIT              | Unknown, because frames may reappear on recovery      | `commit_indeterminate`     | Recovery re-append, or `Indeterminate` that is safe to retry with the same key. |

A client that hits its deadline drops the connection, but the blocking task
keeps running. The original can therefore commit **after** the recovery
attempt. This is harmless: whichever request reaches the lock first inserts,
and the other replays.

### Recovery flow (G-1, replaces the Q-08 lookup)

Each attempt has a 3 s deadline, set by a new `ClientConfig` field that
defaults to 3 s (1 s busy + about 250 ms fsync + margin). The worst case is
about 6 s. Lookup and forget also use 3 s.

| First attempt result                                                        | Action                                                       |
| --------------------------------------------------------------------------- | ------------------------------------------------------------ |
| 201/200 receipt, any mapped 4xx, 409, 503, 507, `commit_failed`             | Terminal. Return the mapped result.                          |
| Transport failure after send, deadline, `commit_indeterminate`, unknown 5xx | **Exactly one** same-key re-append of the identical envelope |

| Recovery attempt result                                                      | Returned                                                                   |
| ---------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| 200 replay or 201 fresh                                                      | The receipt                                                                |
| `key_conflict` / `key_redacted`                                              | `Conflict(Mismatch / Redacted)`, which reflects real key state             |
| 4xx envelope or detector rejection, bare 404/405                             | Mapped as usual. These are deterministic on identical bytes.               |
| Connect failure, transport failure, deadline, 503, 507, any 500, unknown 5xx | `Indeterminate { message }`, which names the recovery result (decision 38) |

There is never a second recovery attempt, and nothing is spooled.
`Indeterminate` is always safe to retry with the same key.

### Quota and low disk

- `maxPayloadBytes` is fixed at 262144 and advertised.
- `projectQuotaBytes` is optional and off by default. It is a daemon config
  field, `ServerConfig`, which embedded-daemon hosts set through
  `RuntimeConfig`. It caps the live payload bytes in `generic_event_usage`, and
  tombstones free their bytes.
- The low-disk floor is a cached free-space probe of the DB volume (statvfs on
  Unix, the platform equivalent on Windows, under `unsafe_code = "forbid"`). The
  cache refreshes outside the lock when it is more than 5 s old. The proposed
  default floor is 64 MiB, configurable in `ServerConfig` (see
  [Open questions](#open-questions)). `SQLITE_FULL` and `ENOSPC` also map to
  `low_disk`.
- Forget skips the quota and floor checks, because it only frees space. A
  `SQLITE_FULL` during forget gives `commit_failed`.

### Runtime change for `serve`

`commands/serve.rs:33` gets its own `serve_runtime()`, a multi-thread runtime
with 2–4 workers. The shared `runtime()` (`crates/kindling/src/lib.rs:221`)
stays `current_thread` for client verbs. Only the keyed handler moves into
`spawn_blocking`. The std `Mutex` is kept, and since the lock is only taken on a
blocking-pool thread, this is deadlock-free even inside a `current_thread` host
running the embedded daemon. The per-project keyed `Semaphore(1)`, acquired
before `spawn_blocking`, means an ordinary append waits for at most one fsync.
Other projects and `/v1/health` never stall.

### Durability guarantee and the macOS caveat

- **Process crash anywhere:** a receipt implies the row is present.
- **Power loss** on local disks that honour cache flush: a receipt implies the
  row is present. On macOS this holds only while **every** process writing the
  project DB is 0.5 or later, because a 0.4 process can checkpoint with a plain
  fsync. Linux is unaffected. Upgrade note: stop 0.4 hooks and daemons.
- Network filesystems are excluded. Foreign SQLite tools that checkpoint
  without `checkpoint_fullfsync` are documented as a residual risk.

### Observability

`AppState` gets `AtomicU64` counters: inserted, matched, conflict,
redacted_conflict, rejected, busy, low_disk, commit_failed, and max/last commit
µs. Errors and commits over 100 ms are logged to stderr, and payload bytes are
never logged. The counters are not on Capability; exposing them is follow-up
work.

## Secret detection, forget and export/import (T4)

### Detector

All regex bodies move into one rule table, `crates/kindling-service/src/filter/rules.rs`,
with two profiles:

| Profile                   | Used by                  | Rules                                                                                |
| ------------------------- | ------------------------ | ------------------------------------------------------------------------------------ |
| Observation masking       | Every observation append | Unchanged (Node parity). A parity test pins the shared bodies.                       |
| `GenericEventDetector` v1 | Generic-event append     | Known-prefix `sk-ant-`, `sk-`, `gh[pousr]_`, `AKIA`, each with a left boundary `(?:^ | [^A-Za-z0-9_-])`. `Bearer`(case-sensitive).`Basic`(case-sensitive, value ends`={0,2}`then a non-base64 char or end of input, no`//`in the value).`privateKeyBlock` `-----BEGIN [A-Z ]_PRIVATE KEY-----`. `credentialField`: exact names `password\|passwd\|secret\|token\|api[-*]?key\|access[-*]?token\|refresh[-*]?token\|client[-*]?secret\|secret[-*]?key\|private[-*]?key`, not preceded by `[A-Za-z0-9\_-]`, followed by `["']?\s_[:=]` and a value of 8 or more chars. |

The v1 profile excludes the substring `credentialAssignment` rule, the
shell-assignment and URL transforms, and the hook-filter heuristic in
`crates/kindling/src/hook/filter.rs`. That heuristic flags any 32+ alnum run
containing a digit and a letter, so it would reject every SHA-256 hex. A test
asserts that the generic path never calls it.

| Surface      | Screening                                                                                                                                                                                   |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `payload`    | Full v1 profile over the raw bytes with `regex::bytes`. Nothing is decoded, and non-UTF-8 is allowed.                                                                                       |
| `scopeIds.*` | Full v1 profile                                                                                                                                                                             |
| `key`        | Grammar plus the anchored prefix subset (`sk-ant-`, `sk-`, `gh[pousr]_`, `AKIA`). The key appears in clear in receipts, lookups, exports, tombstones, errors and logs, and survives forget. |
| `schemaName` | Grammar only                                                                                                                                                                                |

Verified behaviour: `kfit/run-42/evt-7`, `task-<sha>`, `desk-<sha>` and
`risk_sk-<sha>` are clean. `approval:sk-<40hex>`, `run/sk-<40hex>`,
`k.sk-<40hex>`, `AKIA…` and `ghp_…` are rejected. One false positive is known
and accepted: `"token": "<uuid>"`. anvil renames such a field (for example,
`idempotencyToken` is not matched).

A rejection has the shape `{field: payload|key|scope.<name>, classes, count,
detectorVersion}` and never includes matched text, an offset or a length. It is
terminal, is never spooled, and reserves nothing. The detector runs outside the
lock but counts only on a key miss (step 5 above). A rule change bumps
`secretDetectorVersion`, gets a CHANGELOG entry, and can never break replay.
Each row stores `detector_version`. The fixture vectors in
`schema/fixtures/generic-event-detector-v1.json`, including the key cases above,
are published for anvil CI. There is no allow-list in 0.5.0.

### Forget (tombstone)

`POST /v1/generic-events/forget` runs under `durable_write`. It sets `payload`
to NULL and records `redacted_at`. The usage trigger subtracts the bytes. Key,
scope, digest, event id, `committed_at` and `detector_version` are kept. A
repeat forget returns the same `redactedAt`. Afterwards a lookup returns
`redacted` with the original receipt, and a same-key append returns
`key_redacted` even when the digest is identical. Forget is **logical**:
residual bytes can remain in the WAL, free pages, backups and old bundles. Docs
must say so and must never claim erasure.

### Export bundles

| Bundle content                             | `bundleVersion` / `dataset.version` | Readers                                          |
| ------------------------------------------ | ----------------------------------- | ------------------------------------------------ |
| No generic events, including no tombstones | `"1.0"`, byte-identical to 0.4      | 0.4 and 0.5                                      |
| Any generic events or tombstones           | `"2.0"`                             | 0.5 only. 0.4 fails loudly on the version check. |
| `"1.0"` that carries `genericEvents`       | Invalid                             | 0.5 rejects it                                   |

`dataset.genericEvents` is skipped when empty. Tombstones are always exported
whatever `include_redacted` says. `limit` never truncates generic events. A
scope-filtered bundle is documented as not a complete key reservation.

```jsonc
// live row
{ "eventId": "…", "envelopeVersion": 1, "schemaName": "…", "schemaVersion": 3, "key": "…",
  "scope": { "projectId": "…" }, "digest": "sha256:…", "committedAt": 0, "detectorVersion": 1,
  "payload": "<base64>" }
// tombstone: no payload field
{ "eventId": "…", "schemaName": "…", "schemaVersion": 3, "key": "…", "scope": { … },
  "digest": "sha256:…", "committedAt": 0, "redacted": true, "redactedAt": 0, "detectorVersion": 1 }
```

### Import

- All generic-event checks run **before any entity write**, in the same
  `durable_write` transaction. Today's import can partially apply, but generic
  events make it all-or-nothing.
- SHA-256 is recomputed for every live row, and a mismatch fails the import and
  names the key. There is **no detector re-scan**, which keeps old backups
  restorable.
- The report counts rows whose `detectorVersion` is lower than the current
  version. `dry_run` reports a `redacted` count.

| Incoming \ existing | None             | Live, same key+scope+digest | Tombstone, same key+scope+digest | Different scope or digest         |
| ------------------- | ---------------- | --------------------------- | -------------------------------- | --------------------------------- |
| Live                | Insert           | Skip                        | Skip (never resurrect)           | Fail whole import, naming the key |
| Tombstone           | Insert tombstone | **Redact live** (one-way)   | Skip                             | Fail whole import, naming the key |

Redacting a live row from an incoming tombstone breaks the existing "import
never overwrites" rule. This is deliberate and documented.

### Trust boundary (for SECURITY.md and docs/data-model.md)

- The boundary is the same UID: socket 0600, directory 0700.
  `X-Kindling-Project` routes requests and does not authenticate them.
- Payloads are plaintext at rest and in bundles, so bundles are sensitive.
- The detector is best-effort. Its rules and version are listed, and it cannot
  see encoded or encrypted content. Imported payloads are not re-screened.
- Receipts and digests are not attestation (unkeyed SHA-256), so anvil signs.
- Forget is logical, as described above.
- CodeQL disposition: the detector is not a sanitiser.

## Mixed-version matrix (T1)

| Combination                                                   | Ordinary operations              | Generic events                                                              |
| ------------------------------------------------------------- | -------------------------------- | --------------------------------------------------------------------------- |
| 0.5 client/runtime → 0.4 daemon already on the socket         | Work (wire 6)                    | `Unsupported` (no `features`, or bare 404) until the daemon restarts on 0.5 |
| 0.4 client, hooks, VSIX or TS thin client → 0.5 daemon        | Work                             | Not visible to them                                                         |
| 0.4 in-process binary (CLI, hook) on a revision-7 DB          | Work. `user_version` is still 6. | Invisible but preserved. A 0.4 export omits them.                           |
| 0.5 on a 0.4 DB (`user_version` 6, no row 7)                  | Work                             | Writable open migrates 007. Read-only open: lookup returns `notFound`.      |
| 0.5 on `user_version` 5                                       | 006 then 007                     | Available after migration                                                   |
| Any binary on `user_version > 7` (a future breaking revision) | 0.5 refuses (`SchemaTooNew`)     | —                                                                           |
| 0.4 imports a `"2.0"` bundle                                  | Fails loudly                     | —                                                                           |
| 0.4 imports a 0.5 `"1.0"` bundle                              | Works, byte-identical            | —                                                                           |
| 0.5 imports a 0.4 `"1.0"` bundle                              | Works                            | —                                                                           |
| 0.5 client → future daemon with `wireVersion` > 6             | `SchemaMismatch` (upper bound)   | —                                                                           |
| macOS, 0.4 process writing the same DB as a 0.5 daemon        | Work                             | Process-crash durable. Power-loss guarantee void until every writer is 0.5. |
| Tools that read `user_version` as the schema revision         | —                                | Must read `MAX(schema_migrations.version)` (documented)                     |

## Performance contract and bench (T3, G-2)

The KINTEG-013/014 budgets are inherited unchanged. Warm ordinary append p95
must stay under 1 ms. Outage (spooled) append p95 must stay under 5 ms. These
are measured by the existing `daemon` and `outage-recovery` groups
(`daemon_group`, `spool_group` in `crates/kindling-bench/src/workloads.rs`).

A new `keyed-durable` group is added to `GROUP_NAMES` (`workloads.rs:75`) and
run with `--only-group keyed-durable`.

| Workload                     | Shape                                                                           | Status                                                |
| ---------------------------- | ------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `ordinary-append-mixed`      | Closed-loop ordinary writer, with a keyed writer at **5/s** on the same project | **Gate:** ordinary p95 < 1 ms. p99 and max reported.  |
| `keyed-append-replay`        | Same-key matches                                                                | **Gate:** p95 < 1 ms, which shows replay never fsyncs |
| `keyed-append-durable`       | Sequential unique keys                                                          | Reported                                              |
| Rate sweep                   | Keyed at 1, 5, 20 and 50/s, plus saturated                                      | Reported. The report documents the knee.              |
| `ordinary-append-mixed-open` | Open-loop variant                                                               | Reported                                              |

- **Gate conditions:** release prep on a real local disk (ext4 or APFS) with
  `durability_measured=true`, taking the median of 5 runs. The report records
  filesystem type, synchronous mode and fullfsync. tmpfs runs set
  `durability_measured=false` and cannot pass the gate.
- **CI:** smoke runs only, with no threshold. No workflow runs kindling-bench
  today.
- **Stated limit:** keyed appends at 5/s or less keep the ordinary budget.
  Ordinary p95 degrades once keyed rate × fsync latency exceeds about 5 % of
  the time. macOS checkpoint cost lands at p99, not p95.
- Release-prep results are recorded in ENGINEERING-HISTORY.

## Test and validation plan

| Area                    | Test                                                                                                                                                                                                                                                                                                                    | Item       |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- |
| Digest                  | Golden vectors from `schema/fixtures/generic-event-digest-v1.json` (minimal, all scope, max payload), asserted in CI                                                                                                                                                                                                    | 015a       |
| Detector                | `schema/fixtures/generic-event-detector-v1.json` clean and rejected vectors, including key cases. Parity test for the shared rule bodies. Test that the generic path never calls `hook/filter.rs`.                                                                                                                      | 015a       |
| Envelope                | Grammar boundaries for each field (length 0, 1, max, max+1), strict base64 (unpadded, URL alphabet and whitespace rejected), `""` scope rejected                                                                                                                                                                        | 015a       |
| Schema                  | Migration 007 is idempotent across racing opens. `schema_sql_pins_user_version` (`kindling-store/src/schema.rs:59`) is rewritten to pin `minReader`. The assertion at `kindling-store/tests/store_integration.rs:64` is fixed. A golden test shows no FK, trigger or CHECK coupling. Read-only open returns `notFound`. | 015a       |
| Frozen 0.4.1 open check | A test embeds the 0.4.1 open gate (refuse 0 or >6, migrate only 5) and asserts it accepts a revision-7 DB and can still INSERT, UPDATE and DELETE observations                                                                                                                                                          | 015a       |
| Wire range              | Rust and TS clients accept 6, reject 5 and 7. Const asserts. The parse trap is covered by reading `wireVersion` from a `version.json` whose first `"version"` is 7.                                                                                                                                                     | 015a       |
| Pragma state            | Read `PRAGMA synchronous` and `fullfsync` inside the closure (FULL) and after success, error and panic (NORMAL, 5 s busy). `checkpoint_fullfsync=ON` on writable connections.                                                                                                                                           | 015b       |
| Keyed path              | New insert, replay (byte-identical receipt), mismatch, redacted conflict, detector hit on key miss vs replay after a rule bump, quota (counter), low disk (injected probe), `SQLITE_FULL` mapping                                                                                                                       | 015b       |
| Export/import           | `"1.0"` byte-identical with no generic events. `"2.0"` with tombstones. `"1.0"`+`genericEvents` rejected. **All-or-nothing**: a mismatch on the last row leaves zero entity and zero generic-event writes. Digest recompute. Precedence table. `dry_run` counts.                                                        | 015b       |
| Daemon                  | Each code, status and `details` shape. 413 with and without a body. Capability golden shape. Unknown feature entry tolerated. Ordinary handlers and `/v1/health` stay responsive during a keyed fsync.                                                                                                                  | 015c       |
| Client recovery         | Deadline, transport drop after send, `commit_indeterminate` → exactly one re-append. Recovery returns 503, 507 or 500 → `Indeterminate`. Bare 404 → `Unsupported`.                                                                                                                                                      | 015c       |
| **Negative**            | For quota, low disk, outage, busy, failed commit, indeterminate, rejected and conflict, the test asserts **no receipt is returned, the spool file is untouched** and no `AppendOutcome::Spooled` is produced, through the client and the runtime                                                                        | 015c, 015d |
| Failpoint crash         | `--features test-failpoints`, `KINDLING_FAILPOINT=keyed_before_commit` gives `notFound` and then a fresh insert. `keyed_after_commit` gives the same event id and digest with exactly one row after restart. Runs on ubuntu, macos and windows runners (Windows over TCP).                                              | 015d       |
| Nightly                 | Random-SIGKILL loop asserts that client-held receipts are a subset of the rows on disk. Optional non-blocking Linux LazyFS job.                                                                                                                                                                                         | 015d       |
| Runtime                 | `cargo test -p kindling-runtime --all-features`: feature check before the first write and after a reconnect. Attach to a 0.4-like daemon gives `Unsupported`. A restart round-trip returns the same receipt.                                                                                                            | 015d       |
| Release                 | Package-content check, then a clean scratch-crate install of the published 0.5.0 runs the keyed round-trip                                                                                                                                                                                                              | 015d       |

## Documentation and release notes

The following files are updated in the item that changes the behaviour, with a
final sweep in 015d.

| File                                                                    | Change                                                                                                                                                                                   |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `schema/README.md`                                                      | `user_version` = minReader, revision = `MAX(schema_migrations.version)`, `wireVersion`, the additive rule and the updated migration procedure                                            |
| `schema/schema.sql:11`, `schema/version.json`, vendored copies          | New comment and fields. Run `scripts/sync-vendored-schema.sh`.                                                                                                                           |
| `CONTEXT.md:219`                                                        | Schema contract wording. New terms: generic event, keyed append, receipt, digest, tombstone, capability feature.                                                                         |
| `docs/architecture.md:385`                                              | Schema version and revision, endpoint table, client and runtime surfaces, `serve` multi-thread runtime                                                                                   |
| `docs/data-model.md:247,259`                                            | Schema version, `generic_events`, bundle `"1.0"`/`"2.0"` contract, trust boundary, logical forget                                                                                        |
| `docs/development.md`                                                   | Invariant 2 (the generic path detects and rejects in the service), invariant 5, endpoint recipe (no TS mirror method, per Q-03), schema recipe, failpoint tests, new fixture drift gates |
| `docs/integrations.md`                                                  | Rust SDK rows: generic events are available through the client and runtime, not through embedded `kindling-service`                                                                      |
| `SECURITY.md`                                                           | "Generic events" section and the CodeQL disposition                                                                                                                                      |
| `crates/kindling-runtime/README.md`, `crates/kindling-client/README.md` | Usage, the closed failure set, recovery semantics and the mixed-version note                                                                                                             |
| `crates/kindling-bench/README.md`                                       | `keyed-durable` group and gate procedure                                                                                                                                                 |
| `AGENTS.md`                                                             | One line on generic events in the domain model                                                                                                                                           |

**CHANGELOG.md (Unreleased → 0.5.0):**

- Added: `generic-event@1` and keyed durable append, lookup and forget through
  `kindling-runtime`, `kindling-client` and `/v1/generic-events*`; the
  capability `features`.
- Changed: schema revision 7, with `user_version` still 6; the client
  schema-version range check; bundle `"2.0"` when generic events are present;
  `kindling serve` uses a multi-thread runtime.
- Security: detector rejection, the trust boundary, logical forget and the
  detector version.
- Upgrade: restart the daemon after upgrading, because a 0.4 daemon left on the
  socket reports `Unsupported`. 0.4 can open 0.5 DBs but does not see or export
  generic events, and cannot import `"2.0"` bundles. **macOS:** stop 0.4 hooks
  and daemons for the power-loss guarantee. Tools that read `user_version` as
  the revision must read `schema_migrations`.

**ENGINEERING-HISTORY.md:** council plan-9ad9b830 and this spec; the version
stamp split; the `durable_write` mechanism and the rejected alternatives
(second connection, `wal_checkpoint(FULL)`, out-of-lock fsync); recovery
re-append vs lookup; detector v1 rules and the false-positive analysis;
release-prep bench numbers; the crash-test approach. Run
`python3 scripts/check-release-docs.py` after editing either record.

## Discovered follow-up work

These are not APS items in this change. The bookkeeping pass decides whether to
add them.

- **Reconcile hook-filter detector drift.** `crates/kindling/src/hook/filter.rs`
  has thresholds that differ from the service rules (`sk-ant {90,}` vs `{20,}`,
  `sk- {48,}` vs `{40,}`) plus an entropy-like heuristic. It should use the
  shared rule table.
- **Add `privateKeyBlock` to observation masking.** The generic profile catches
  PEM private keys, but observation masking does not.
- **Physical erasure on forget.** `secure_delete` plus a checkpoint on the forget
  path, designed so it does not tax ordinary appends or block writers.
- **Expose the keyed counters.** Surface the `AppState` counters through status
  or health once a stable shape is agreed.
- **Canonicalise `project_root`.** `/repo` and `/repo/` hash to different
  project DBs, so generic-event keys split across them.
- **Listing and pagination** as a new `generic-event-list@1` feature entry,
  possibly with batch append.

## Amendments to the original KINTEG-015 wording

1. **Uniqueness is per project store, not per profile.** "Unique across the
   owner-local profile" becomes "unique per (schema name, key) within a project
   DB". The project id is part of the canonical scope and digest.
2. **The closed failure set gains `Indeterminate`.** It is a distinct,
   retry-safe outcome after the single same-key recovery re-append.
3. **`StoreBusy` folds into `FailedCommit { kind: Busy }`.** It stays a definite
   "not committed".
4. **A forget endpoint is added** (`POST /v1/generic-events/forget`), so the
   `redacted` lookup state and `key_redacted` conflict have a write path.
5. **The release is final 0.5.0, with no release candidate**, not coordinated
   with C14.

## Open questions

1. **anvil:** confirm that per-project key uniqueness (amendment 1) meets
   KFIT-005, given that evidence from different project roots lands in
   different stores.
2. **anvil:** confirm the key grammar (`[\x21-\x7E]{1,256}`) and
   `anvil.governance.v1` field naming against detector v1 using the fixture
   file, including the accepted `"token": "<uuid>"` false positive.
3. **Low-disk floor default.** The council fixed the mechanism but not the
   number. This spec proposes 64 MiB, configurable.
4. **Quota and floor configuration surface.** This spec proposes
   `ServerConfig` fields set through `RuntimeConfig`. A `kindling serve` flag
   would be a CLI change in the "Discuss first" lane and is left out of 0.5.0
   unless approved.
5. **Free-space probe dependency.** A safe cross-platform probe under
   `unsafe_code = "forbid"`, with `rustix` already in the lock for Unix and the
   Windows equivalent to be chosen. A new dependency may need the "Discuss
   first" lane.
6. **macOS CI runner.** No macOS test job exists today, only cross-builds. The
   failpoint crash job needs one added.
7. **Synthesis decisions 38 and 39** (the recovery-attempt mapping and reopen
   failure) and the forget transport semantics need confirming at design review.
