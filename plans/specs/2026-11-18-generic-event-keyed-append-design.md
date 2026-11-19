# Generic Events and Keyed Durable Append (`generic-event@1`)

| Field      | Value                                                    |
| ---------- | -------------------------------------------------------- |
| Status     | Accepted                                                 |
| Owner      | @aneki                                                   |
| Created    | 2026-11-18                                               |
| APS Module | `plans/modules/06-downstream-integration-surface.aps.md` |
| Work Item  | KINTEG-015 (015a–015e)                                   |
| Council    | plan-9ad9b830                                            |

Planning council plan-9ad9b830 produced this spec from the user decisions Q-00 to
Q-13 and the gates G-1 to G-3, the negotiated outcomes T1 to T4, and two Phase 4
reviews (architect and devil's advocate). Release 0.5.0 ships without a release
candidate (Q-12), so this contract must be right the first time. Where it
disagrees with the original KINTEG-015 wording, this spec wins (see
[Amendments](#amendments-to-the-original-kinteg-015-wording)).

## Context

anvil KFIT-005 (`eddacraft/anvil-001`, `plans/modules/kindling-product-fit.aps.md`)
must store governance and audit evidence (its `anvil.governance.v1` payload)
durably and retrieve it by exact key after a restart; anvil Track B (KFIT-007,
-009, -010, -011, -013) waits on this release. Through `kindling-runtime`,
kindling must provide a versioned envelope around an opaque, size-bounded
payload; a keyed append that returns a receipt only after a durable commit, is
idempotent on a same-key retry and reports a conflict on a mismatch; exact lookup
by key; a closed failure set with no spool fallback; and capability negotiation,
in which 0.4.x reports unsupported.

The relevant code as of v0.4.1:

| Area                   | v0.4.1 behaviour                                                                                                                                                                                                           | Reference                                                                                                  |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Storage and routing    | One DB per project at `<home>/projects/<sha256(root)[..12]>/kindling.db`, keyed on the raw root string. `X-Kindling-Project` routes requests but does not authenticate them. One std `Mutex<KindlingService>` per project. | `kindling-store/src/paths.rs:15-37`, `kindling-server/src/state.rs:18-20,60-90`                            |
| Durability             | WAL with `synchronous=NORMAL` and a 5 s busy timeout. No checkpoint tuning.                                                                                                                                                | `kindling-store/src/db.rs:158-166`                                                                         |
| Schema gate            | Refuses `user_version` of 0 or above 6, migrates only from 5, has no down path. `version.json` = `{version 6, minCompatible 1}`.                                                                                           | `db.rs:172-207`                                                                                            |
| Client gate            | Requires `schemaVersion == 6`. The const parser reads the first `"version"` key. The TS client does the same.                                                                                                              | `kindling-client/src/config.rs:20-40`, `packages/kindling/src/client.ts:105`                               |
| Masking and dedup      | Every append masks content, provenance and scope. Dedup is id-based `INSERT OR IGNORE`.                                                                                                                                    | `kindling-service/src/service.rs:293-347`, `kindling-store/src/store.rs:71-95`                             |
| Detectors              | 7 service regexes plus a drifted hook-filter copy that has a 32+ alnum heuristic.                                                                                                                                          | `kindling-service/src/filter/secrets.rs`, `kindling/src/hook/filter.rs:60-90`                              |
| Forget, export, import | Forget is a soft redaction. Bundles are `"1.0"`. Import is id-based, can apply partially, and runs in-process only.                                                                                                        | `store.rs:303-307`, `kindling-service/src/export.rs`, `kindling/src/commands/export.rs:1,135`              |
| Capability and errors  | No feature flags. `ApiError` covers 400/404/409/500, and store errors become a free-text 500.                                                                                                                              | `kindling-types/src/capability.rs:28-38`, `kindling-server/src/error.rs:33-55`                             |
| Client and spool       | Only connect is bounded, with no per-request deadline. `SpooledClient` returns `Spooled` on an outage. The spool is flushed but not fsynced.                                                                               | `kindling-client/src/transport.rs:181-260`, `spool.rs:216-227,576-582`                                     |
| Runtime and daemon     | The runtime attaches to any daemon already on the socket. `serve` runs on a `current_thread` runtime. Handlers lock inline and `.expect()` on a poisoned mutex. `shutdown` only calls `abort_and_join`.                    | `kindling-runtime/src/lib.rs:241-370`, `kindling/src/lib.rs:221-225`, `handlers.rs:82-302`                 |
| Config structs         | `RuntimeConfig` is `#[non_exhaustive]`. `ServerConfig` and `ClientConfig` have pub fields and are not.                                                                                                                     | `kindling-runtime/src/lib.rs:125`, `kindling-server/src/config.rs:38`, `kindling-client/src/config.rs:205` |
| Gaps                   | No quota, free-space, `SQLITE_FULL` or `ENOSPC` handling. The UDS socket is 0600 inside a 0700 directory. `sha2` and `base64 0.22.1` are already dependencies.                                                             | `kindling-server/src/lib.rs:255-266`, `Cargo.lock`                                                         |

## Goals and non-goals

**Goals:** ship `generic-event@1` (keyed durable append, lookup, forget) in final
0.5.0 via the runtime, client and v1 endpoints; a receipt, `found` or `redacted`
answer means the row is durable; failures are a closed, exhaustive set that never
touches the spool; 0.4.x still opens 0.5 DBs; ordinary-append budgets unchanged.

**Non-goals:** payload meaning or anvil policy; unkeyed events, listing,
pagination, batch (later feature entries); a TS method, CLI verb or
semver-covered embedded API (Q-03); FTS/retrieve/inject/list visibility;
attestation; physical erasure; a detector allow-list; retention; cloud or sync.

## Decisions

| #   | Source (R = Phase 4 review) | Choice                                                                                                                           | Section                                                                      |
| --- | --------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| 1   | Q-01                        | Key unique per `(schemaName, key)` within one project DB; `projectId` is part of the scope                                       | [Storage](#storage-and-schema-compatibility-t1)                              |
| 2   | Q-02                        | Separate `generic_events` table; not FTS/retrieve/inject/list; exported/imported                                                 | [Storage](#storage-and-schema-compatibility-t1)                              |
| 3   | Q-03                        | v1 endpoints + `kindling-client` + `kindling-runtime`; ts-rs only for TS                                                         | [Rust API](#rust-api)                                                        |
| 4   | Q-00                        | Additive `features` on `/v1/health`; absent = unsupported; server re-checks                                                      | [Capability](#capability-and-versioning)                                     |
| 5   | Q-04                        | Read-only detector; match → `RejectedPayload`; bytes never mutated                                                               | [Detector](#secret-detection-forget-and-exportimport-t4)                     |
| 6   | Q-05                        | Forget tombstones; key stays reserved; later append → `Redacted` conflict                                                        | [Forget](#forget)                                                            |
| 7   | Q-06                        | Tagged SHA-256 over framed schema, scope and payload; key excluded; lookup recomputes                                            | [Digest](#digest-framing)                                                    |
| 8   | Q-13                        | Import: full match skipped, mismatch fails whole import naming the key                                                           | [Import](#import)                                                            |
| 9   | Q-07                        | FULL sync around generic-event transactions; ordinary append stays NORMAL (see amendment 7)                                      | [Durable write](#durable-write)                                              |
| 10  | Q-08/G-1/R                  | Per-call deadline; one same-key re-append on which only a receipt or a 409 is terminal; else `Indeterminate`                     | [Recovery](#recovery-flow)                                                   |
| 11  | Q-09                        | Fixed max payload; optional project quota; low-disk floor; typed codes                                                           | [Quota](#quota-and-low-disk)                                                 |
| 12  | Q-10                        | Additive revision 7 with a `minReader` floor; client range check                                                                 | [Storage](#storage-and-schema-compatibility-t1)                              |
| 13  | Q-11                        | KINTEG-015a–e, sequential, one capability                                                                                        | [Delivery](#delivery-and-release)                                            |
| 14  | Q-12                        | Final 0.5.0 via `scripts/publish.sh`, no RC, not coupled to C14                                                                  | [Delivery](#delivery-and-release)                                            |
| 15  | G-2                         | Ext4 release-prep gate at keyed 5/s plus replay gate; sweep reported                                                             | [Bench](#performance-contract-and-bench-t3-g-2)                              |
| 16  | G-3                         | Forget endpoint ships in 015d; macOS caveat documented                                                                           | [Delivery](#delivery-and-release), [Guarantee](#guarantee-and-macos-caveat)  |
| 17  | T1                          | `user_version` = minReader 6, revision 7, `wireVersion` 6; bundle `"1.0"`/`"2.0"`                                                | [Storage](#storage-and-schema-compatibility-t1), [Export](#export-bundles)   |
| 18  | T2                          | Strict envelope, base64 payload, keys in bodies, byte-identical receipt; exhaustive top-level enums, `#[non_exhaustive]` details | [Wire contract](#wire-contract-t2), [Rust API](#rust-api)                    |
| 19  | T3/R                        | Durability barrier and keyed-suspect state; COMMIT-step errors never yield a receipt                                             | [Barrier](#durability-barrier-and-keyed-suspect-state)                       |
| 20  | T3/R                        | Multi-thread `serve`, owned permit in `spawn_blocking`, stated wait bound; usage counter; failpoint tests                        | [Concurrency](#concurrency-and-shutdown), [Tests](#test-and-validation-plan) |
| 21  | T4                          | One rule table, two profiles; detection counts only on key miss; per-row detector version                                        | [Detector](#genericeventdetector-v1-rules)                                   |
| 22  | T4                          | Tombstones always exported; import all-or-nothing, digests recomputed, no re-scan                                                | [Export/import](#export-bundles)                                             |

## Storage and schema compatibility (T1)

`generic_events` columns: `event_id` TEXT primary key (server UUIDv4);
`envelope_version` INTEGER (1); `schema_name`, `event_key` TEXT with
`UNIQUE(schema_name, event_key)`, compared byte for byte; `schema_version`
INTEGER (u32 ≥ 1); `project_id` TEXT (12 lowercase hex); nullable TEXT
`session_id`, `repo_id`, `agent_id`, `user_id`, `task_id` (NULL = absent);
`digest` TEXT (`sha256:<64 hex>`); `payload` BLOB, NULL once tombstoned;
`committed_at` INTEGER ms; nullable `redacted_at` INTEGER, set once;
`detector_version` INTEGER, outside the digest and receipt.

`generic_event_usage` is a single row (`id = 1`) holding `payload_bytes` and
`event_count`. Migration 007 seeds it, and AFTER INSERT, UPDATE and DELETE
triggers on `generic_events` maintain it, with byte counts taken from
`length(CAST(payload AS BLOB))`. `generic_events` has no foreign key, trigger
or CHECK coupling to existing tables and no FTS. A golden test enforces this.

**Version stamps:** `PRAGMA user_version` = **minReader** = 6, the lowest
schema revision a binary must know to open the DB; writers stamp it and never
lower it. The **revision** is `MAX(schema_migrations.version)` = 7.
`version.json` = `{version 7, minReader 6, wireVersion 6, minCompatible 1, ftsTokenizer}`,
synced to every vendored copy by `scripts/sync-vendored-schema.sh`.
`wireVersion` is reported as Capability `schemaVersion` and rises only on a
breaking wire change.

**Open rules (0.5):**

| DB state                               | Writable open                                                     | Read-only open                                            |
| -------------------------------------- | ----------------------------------------------------------------- | --------------------------------------------------------- |
| `user_version` 0                       | Refused (`SchemaTooOld`)                                          | Refused                                                   |
| `user_version` 1–4                     | Opens as in 0.4 without running 006; runs 007 if row 7 is missing | Opens as in 0.4. Generic reads return `notFound`.         |
| `user_version` 5                       | Runs 006, then 007                                                | As in 0.4                                                 |
| `user_version` 6 (or 7), row 7 missing | Runs 007 (present: opens)                                         | Opens without migrating. Generic reads return `notFound`. |
| `user_version` above 7                 | Refused (`SchemaTooNew`)                                          | Refused                                                   |

Every writable open then runs the [durability barrier](#durability-barrier-and-keyed-suspect-state).

Migration 007 runs inside `BEGIN IMMEDIATE`. It uses `CREATE … IF NOT EXISTS`
for the tables, index and triggers, seeds the usage row, and
`INSERT OR IGNORE`s row 7. It never writes `user_version`, so it is safe when
several 0.5 processes race, and a 0.4 daemon simply re-prepares on
`SQLITE_SCHEMA`. `schema.sql` seeds row 7 and `user_version = 6`. **Additive
rule** (new in `schema/README.md`): a revision is additive only if every older
writer's INSERT, UPDATE and DELETE stays valid on every table, including new
ones; anything else raises `user_version`.

**Wire version:** one helper (`kindling_store::wire_version()`) feeds
`handlers.rs:60` and `commands/status.rs:62`. The Rust and TS clients accept
`6 <= schemaVersion <= MAX_WIRE_VERSION` (6), and the parse traps at
`kindling-client/src/config.rs:31` and `packages/kindling/src/config.ts:47` read
`wireVersion`. `EXPECTED_SCHEMA_VERSION` and `expected_schema_version` keep their
names as the lower bound; const asserts pin `EXPECTED_SCHEMA_VERSION == 6` and
`KNOWN_SCHEMA_REVISION == 7`. Additive features never raise `wireVersion`.

## Wire contract (T2)

### Envelope and endpoints

```jsonc
// Envelope (request body of POST /v1/generic-events)
{ "envelopeVersion": 1,               // must be 1
  "schemaName": "example.audit",      // ^[a-z0-9][a-z0-9._-]{0,127}$
  "schemaVersion": 3,                 // u32 >= 1
  "key": "kfit/run-42/evt-7",         // ^[\x21-\x7E]{1,256}$, compared byte for byte
  "scopeIds": { "sessionId": "s" },   // ScopeIds; each 1..=256 bytes, no control chars, "" rejected
  "payload": "eyJrIjoidiJ9" }         // padded strict base64 (RFC 4648 §4); 1..=maxPayloadBytes decoded
// GenericEventReceipt: byte-identical on every replay, lookup and forget; no replayed/payloadBytes/durability field
{ "eventId": "<uuid>", "envelopeVersion": 1, "schemaName": "example.audit", "schemaVersion": 3,
  "key": "kfit/run-42/evt-7", "scope": { "projectId": "0123456789ab", "sessionId": "s" },
  "digest": "sha256:<hex>", "committedAt": 1763452800000 }
// Lookup/forget answers, tagged on "state"; a repeat forget keeps the original redactedAt
{ "state": "found", "receipt": {}, "payload": "<base64>" }
{ "state": "redacted", "receipt": {}, "redactedAt": 1763456400000 }
{ "state": "notFound" }
```

The daemon derives `projectId` from the routing root (`paths::project_id`); it is
never in the envelope. Scope values are screened, never masked. The route body
limit is `4 * ceil(maxPayloadBytes / 3) + 16 KiB`. All routes require
`X-Kindling-Project`; `notFound` is a normal 200, and a bare 404 means the route
does not exist (a 0.4 daemon). 201 versus 200 tells the caller about a replay.

| Route                            | Body                | Success                                         | Ships in |
| -------------------------------- | ------------------- | ----------------------------------------------- | -------- |
| `POST /v1/generic-events`        | envelope            | 201 new receipt, 200 original receipt on replay | 015c     |
| `POST /v1/generic-events/lookup` | `{schemaName, key}` | 200 `found`, `redacted` or `notFound`           | 015c     |
| `POST /v1/generic-events/forget` | `{schemaName, key}` | 200 `redacted` or `notFound`; idempotent        | 015d     |

### Digest framing

`digest = "sha256:" + lowercase_hex(SHA-256(M))`. `M` is built from these fields,
in this order:

| Order | Field                                                             | Encoding                                                                     |
| ----- | ----------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| 1     | Domain tag                                                        | The 25 ASCII bytes `kindling.generic-event.v1`, then `0x00` (26 bytes total) |
| 2     | `schemaName`                                                      | `u32be` length ‖ UTF-8 bytes                                                 |
| 3     | `schemaVersion`                                                   | `u32be`                                                                      |
| 4–9   | `projectId`, `sessionId`, `repoId`, `agentId`, `userId`, `taskId` | `u32be` length ‖ UTF-8 bytes. Length 0 means absent, since `""` is rejected  |
| 10    | payload                                                           | `u32be` length ‖ raw decoded bytes                                           |

The key, `envelopeVersion` (implied by the tag) and `detector_version` are not
in `M`. Only the server computes digests (no public helper), and
[lookup](#error-codes-and-invariants) recomputes them. The authoritative vectors
live in `schema/fixtures/generic-event-digest-v1.json`; the values below were
recomputed independently during review.

| Vector      | Inputs                                                                                                     | `len(M)` | Digest                                                                    |
| ----------- | ---------------------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------- |
| minimal     | `"a"`, 1, `projectId "000000000000"`, payload `00`                                                         | 76       | `sha256:43852a2e7ef09166831bb33fce56d5ad008c97be6e8d3770326316b1d6b48c6f` |
| all scope   | `"example.audit"`, 3, `"0123456789ab"`, `sess-1`/`repo-1`/`agent-1`/`user-1`/`task-1`, payload `{"k":"v"}` | 127      | `sha256:d4391e955ce4cf16ae13fccb7bacb97f4263415f956f599c4e4beef35de9e57d` |
| max payload | `"x"×128`, 4294967295, `"ffffffffffff"`, 262144 bytes with byte `i = i mod 251`                            | 262346   | `sha256:cc16e8457b2d01706b41c43ac52af81f45132fe3ea7e9c8cca2124154b794457` |

The minimal vector's `M` is
`6b696e646c696e672e67656e657269632d6576656e742e763100 00000001 61 00000001 0000000c 303030303030303030303030 00000000×5 00000001 00`.

An imported row keeps its original `projectId`, so its digest still verifies.
A later same-key append in the destination DB produces a different `projectId`
and is reported as `Conflict(Mismatch)`.

### Error codes and invariants

Error bodies are `{error, code, details}`. Only generic-event routes use this
shape, and a body never echoes payload text. A 4xx, a 409 and a 507 are always
terminal on the first attempt. What happens on a recovery attempt is defined in
[Recovery](#recovery-flow).

| Code                                                     | HTTP | Routes         | `details`                                  | Append result                                      | Lookup / forget result                                                  |
| -------------------------------------------------------- | ---- | -------------- | ------------------------------------------ | -------------------------------------------------- | ----------------------------------------------------------------------- |
| `invalid_envelope`, `missing_project`                    | 400  | all            | `{field}` (invalid only)                   | `RejectedPayload(InvalidEnvelope)`                 | `InvalidRequest`                                                        |
| `unsupported_envelope_version`                           | 400  | append         | `{supported}`                              | `Unsupported`                                      | —                                                                       |
| `payload_too_large`, bare 413                            | 413  | append         | `{limit, actual}` (absent on a bare 413)   | `RejectedPayload(TooLarge)`                        | —                                                                       |
| `secret_detected`                                        | 422  | append         | `{field, classes, count, detectorVersion}` | `RejectedPayload(SecretDetected)`                  | —                                                                       |
| `key_conflict`                                           | 409  | append         | `{existing}`                               | `Conflict(Mismatch)`                               | —                                                                       |
| `key_redacted`                                           | 409  | append         | `{existing, redactedAt}`                   | `Conflict(Redacted)`, even when the digest matches | —                                                                       |
| `quota_exceeded`                                         | 507  | append         | `{limitBytes, usedBytes}`                  | `Quota`                                            | —                                                                       |
| `low_disk`                                               | 507  | append         | `{freeBytes, floorBytes}`                  | `LowDisk`                                          | —                                                                       |
| `store_busy`                                             | 503  | append, forget | —                                          | `FailedCommit{Busy}`                               | forget: `FailedCommit{Busy}`                                            |
| `commit_failed`                                          | 500  | append, forget | —                                          | `FailedCommit{Storage}`                            | forget: `FailedCommit{Storage}`                                         |
| `commit_indeterminate`                                   | 500  | append, forget | —                                          | Starts recovery                                    | forget: starts the re-send                                              |
| `lookup_failed`                                          | 500  | lookup         | `{reason}`                                 | —                                                  | `FailedLookup`                                                          |
| bare 404 or 405                                          | —    | all            | —                                          | `Unsupported`                                      | `Unsupported`                                                           |
| unknown 4xx                                              | —    | all            | —                                          | `RejectedPayload(Unrecognized)`                    | `InvalidRequest`                                                        |
| unknown 5xx, or transport failure or deadline after send | —    | all            | —                                          | Starts recovery                                    | lookup: `FailedLookup{reason: "transport"}`; forget: starts the re-send |
| nothing sent                                             | —    | all            | —                                          | `Unavailable`                                      | `Unavailable`                                                           |

`lookup_failed` carries a documented `details.reason`: `digest_mismatch` (the
recomputed digest differs), `durability_unconfirmed` (the project is
[keyed-suspect](#durability-barrier-and-keyed-suspect-state)) or `storage`.
Invariants:

- **Append, not committed:** any server-issued 4xx, `503 store_busy`, 507 or
  `500 commit_failed` means the request committed nothing. Any 4xx code added
  later must keep this meaning.
- **Forget, not written:** any server-issued 4xx, `503 store_busy` or
  `500 commit_failed` means no tombstone was written. The client never
  synthesises `FailedCommit`.
- **Receipts:** no failure returns a receipt, writes the spool or produces
  `AppendOutcome::Spooled`. A receipt, `found` or `redacted` answer is given only
  for a row whose durability this process confirmed, by its own FULL commit or a
  completed [barrier](#durability-barrier-and-keyed-suspect-state).
- **Send boundary:** `Unavailable` only if no request byte reached the socket;
  once any byte is written the outcome is ambiguous.

### Capability and versioning

```jsonc
// GET /v1/health additions (serde(default), no deny_unknown_fields); quota omitted when unset
{ "schemaVersion": 6, "schemaRevision": 7, "features": [{ "name": "generic-event", "version": 1,
  "limits": { "maxPayloadBytes": 262144, "maxKeyBytes": 256, "maxSchemaNameBytes": 128,
              "projectQuotaBytes": 1073741824, "secretDetectorVersion": 1 } }] }
// before 015d merges (and in 0.4.x): no generic-event entry
{ "schemaVersion": 6, "schemaRevision": 7, "features": [] }
```

An unknown feature entry or `limits` shape never fails a health parse; the
client types `limits` only for `generic-event` v1 (golden test pins the shape).
The floor is not advertised. `maxPayloadBytes` never decreases within `@1`:
consumers rely on at least 65536 and read the advertised value (262144 in 0.5.0).
One entry covers append, lookup and forget, advertised only once 015d merges;
before that a non-default `test-advertise` cargo feature on `kindling-server`
(not re-exported by `kindling-runtime`) enables it for tests.

| Needs `generic-event@2` and a new 0.x minor                                                 | Additive within `@1`                                   |
| ------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| Any change to the digest framing or payload encoding                                        | New optional response fields                           |
| Lowering `maxPayloadBytes`, or narrowing a charset or range                                 | New `limits` keys; raising `maxPayloadBytes`           |
| Widening the key or `schemaName` charset                                                    | New 4xx codes that still mean not committed            |
| Changing idempotency, uniqueness or conflict semantics                                      | New variants of `#[non_exhaustive]` detail enums       |
| A new lookup or forget state, or a new top-level error variant                              | New feature entries (`generic-event-list@1`, batch)    |
| Changing the status-to-code mapping, or a new `details.reason` meaning for an existing code | A detector rule change (bumps `secretDetectorVersion`) |

### Rust API

Wire shapes live in `kindling-types`, serialised as camelCase with ts-rs
bindings. In the bindings, `payload: Vec<u8>` is
`#[ts(type = "string")]`, and every `i64` or `u64` is `#[ts(type = "number")]`,
following `observation.rs:53`. The outcome enums encode transport results, so
they live in `kindling-client` and `kindling-runtime` re-exports them.

```rust
// kindling-types (all fields pub; serde camelCase; ts-rs)
pub struct GenericEvent { envelope_version: u32, schema_name: String, schema_version: u32, key: String,
                          scope_ids: ScopeIds, payload: Vec<u8> }
pub struct GenericEventScope { project_id: String, session_id: Option<String>, repo_id: Option<String>,
                               agent_id: Option<String>, user_id: Option<String>, task_id: Option<String> }
pub struct GenericEventReceipt { event_id: String, envelope_version: u32, schema_name: String, schema_version: u32,
                                 key: String, scope: GenericEventScope, digest: String, committed_at: i64 }
pub enum GenericEventLookup { Found { receipt: GenericEventReceipt, payload: Vec<u8> },
                              Redacted { receipt: GenericEventReceipt, redacted_at: i64 }, NotFound }
pub enum GenericEventForget { Redacted { receipt: GenericEventReceipt, redacted_at: i64 }, NotFound }
pub struct GenericEventLimits { max_payload_bytes: u64, max_key_bytes: u32, max_schema_name_bytes: u32,
                                project_quota_bytes: Option<u64>, secret_detector_version: u32 }
#[non_exhaustive] pub enum RejectReason { InvalidEnvelope { field: String, message: String },
    TooLarge { limit: u64, actual: u64 /* caller's decoded length */ },
    SecretDetected { field: String, classes: Vec<String>, count: u32, detector_version: u32 },
    Unrecognized { code: String, message: String } }
#[non_exhaustive] pub enum ConflictKind { Mismatch { existing: GenericEventReceipt },
                                          Redacted { existing: GenericEventReceipt, redacted_at: i64 } }
#[non_exhaustive] pub enum CommitFailure { Busy, Storage }

// kindling-client, re-exported by kindling-runtime. Top-level enums are exhaustive (NOT non_exhaustive).
pub enum AppendEventError { Unsupported { detail: String }, Unavailable { message: String },
    RejectedPayload(RejectReason), Conflict(ConflictKind), Quota { limit_bytes: u64, used_bytes: u64 },
    LowDisk { free_bytes: u64, floor_bytes: u64 }, FailedCommit { kind: CommitFailure, message: String },
    Indeterminate { message: String } }
pub enum LookupEventError { Unsupported { detail: String }, Unavailable { message: String },
    InvalidRequest { code: String, message: String }, FailedLookup { reason: String, message: String } }
pub enum ForgetEventError { Unsupported { detail: String }, Unavailable { message: String },
    InvalidRequest { code: String, message: String }, FailedCommit { kind: CommitFailure, message: String },
    Indeterminate { message: String } }
#[non_exhaustive] #[derive(Debug, Clone)]   // Default: 3 s; applies to generic-event methods only
pub struct GenericEventOptions { pub attempt_timeout: Duration }
impl GenericEventOptions { pub fn with_attempt_timeout(self, d: Duration) -> Self; }

// These three async methods exist on both Client and Runtime with identical signatures:
pub async fn append_generic_event(&self, event: &GenericEvent, opts: &GenericEventOptions)
    -> Result<GenericEventReceipt, AppendEventError>;
pub async fn lookup_generic_event(&self, schema_name: &str, key: &str, opts: &GenericEventOptions)
    -> Result<GenericEventLookup, LookupEventError>;
pub async fn forget_generic_event(&self, schema_name: &str, key: &str, opts: &GenericEventOptions)
    -> Result<GenericEventForget, ForgetEventError>;
impl Client { pub async fn generic_event_support(&self) -> Result<Option<GenericEventLimits>, ClientError>; }

// kindling-runtime additions
#[non_exhaustive] pub enum GenericEventSupport {
    Supported { limits: GenericEventLimits, settings_applied: bool }, Unsupported { detail: String } }
#[non_exhaustive] #[derive(Debug, Clone, Default)]   // RuntimeConfig (already non_exhaustive) gains `pub generic_events`
pub struct GenericEventSettings { pub project_quota_bytes: Option<u64>, pub low_disk_floor_bytes: Option<u64> }
impl Runtime {
    pub fn project_id(&self) -> &str;   // paths::project_id(config.project_root)
    pub async fn generic_event_support(&self) -> Result<GenericEventSupport, RuntimeError>;
}
```

- **Deadline:** a per-call option, not a `ClientConfig` field (`ClientConfig` has
  pub fields and is not `#[non_exhaustive]`).
- **Feature checks:** client methods do not pre-check (bare 404/405 →
  `Unsupported`). The runtime checks before the first write and after any
  reconnect and pre-validates limits; a re-check during recovery → `Indeterminate`.
- **Settings on attach:** `GenericEventSettings` applies only to a daemon the
  runtime starts. On attach, the runtime compares the advertised
  `projectQuotaBytes` with the requested one and reports `settings_applied:
  false` in the return value, without logging. The floor cannot be compared.
- **Service methods:** keyed `KindlingService` methods are `pub` +
  `#[doc(hidden)]`, not semver-covered, not an embedded API.

## Durability, concurrency and failure handling (T3)

### Durable write

`SqliteKindlingStore::durable_write(busy, |tx| …)` wraps every transaction that
writes `generic_events` (keyed append, forget, import of a bundle with generic
events). On the project's existing connection it sets `synchronous=FULL` (plus
`fullfsync=ON` on macOS), reads it back and aborts before BEGIN unless it is 2,
sets the busy timeout (1 s on the daemon keyed path, today's 5 s for import),
then runs `BEGIN IMMEDIATE`, the closure and COMMIT. A guard restores `NORMAL`,
`fullfsync=OFF` and 5 s on every path; a failed restore is logged and counted.

`configure()` (`db.rs:158`) sets `checkpoint_fullfsync=ON` on every writable
connection, or a macOS checkpoint could copy keyed rows with a plain fsync
(amendment 7); on Linux both fullfsync pragmas are no-ops. Import of a `"1.0"`
bundle without generic events keeps today's semantics, partial-apply behaviour
and 5 s busy timeout, without `durable_write`.

### Durability barrier and keyed-suspect state

After a process crash, the next opener recovers WAL frames that were never
fsynced and could replay a receipt for a row a power loss would still erase. So
every writable open and in-place reopen (daemon, in-process CLI, embedded daemon)
runs a **barrier** before answering any keyed or forget request from existing rows.

- **Barrier:** under `synchronous=FULL` with `checkpoint_fullfsync=ON`, run
  `PRAGMA wal_checkpoint(FULL)`; it succeeds only when the result shows
  `busy = 0` and `checkpointed = log` (WAL fully backfilled, DB file synced). If
  readers block completion, it may fall back to a write transaction run through
  `durable_write` (so `synchronous=FULL` and, on macOS, `fullfsync=ON` are set,
  read back before `BEGIN` and restored on every path), which fsyncs the WAL
  inode and the recovered frames still in the page cache. The pragma-state test
  covers both the checkpoint and the fallback branch.
  After an I/O error only a completed checkpoint clears suspect, because Linux
  may have marked the failed pages clean. It may be skipped when
  `generic_events` is empty after WAL recovery.
- **Keyed-suspect:** from open until the barrier succeeds, and again after any
  COMMIT-step error or panic. While suspect, each keyed append, forget, or lookup
  that hits an existing row retries the barrier once inside the project permit,
  so a lookup-only consumer cannot stay stuck; a lookup that misses never runs it.

| Request while keyed-suspect                | Answer                                                                                       |
| ------------------------------------------ | -------------------------------------------------------------------------------------------- |
| Append or forget that hits an existing row | `commit_indeterminate`                                                                       |
| Append that misses                         | Proceeds normally, with its own FULL commit                                                  |
| Forget or lookup that finds nothing        | 200 `notFound`                                                                               |
| Lookup that hits an existing row           | Barrier retried once; if it still fails, `lookup_failed`, `reason: "durability_unconfirmed"` |
| Ordinary routes                            | Unaffected                                                                                   |
| `/v1/health`                               | Unchanged, and still advertises the feature (suspect is transient)                           |

### Sequences and server errors

| Step | Keyed append                                                                                                           | Forget                                                            |
| ---- | ---------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| 1    | Without the lock: validate, run the detector, compute the digest, refresh the free-space cache if it is older than 5 s | Validate the key request                                          |
| 2    | `acquire_owned()` the project permit, move it into `spawn_blocking`, then lock the mutex                               | Same                                                              |
| 3    | Run the barrier if the project is suspect, then `durable_write { SELECT (schemaName, key)`                             | Same                                                              |
| 4    | Match → stored receipt (200). Mismatch → `key_conflict`. Tombstone → `key_redacted`. Each is a ROLLBACK with no fsync  | Not found → `notFound`. Tombstone → original `redacted`. ROLLBACK |
| 5    | Key miss with a detector hit → `secret_detected` and ROLLBACK; nothing is reserved                                     | Live row: UPDATE to tombstone (the trigger frees its bytes)       |
| 6    | Quota check, then the cached floor check, then INSERT and COMMIT → 201                                                 | COMMIT → 200 `redacted`                                           |

Quota and floor are checked after the key lookup, so an exact replay still gets
its receipt; forget skips both. A failed in-place reopen leaves the slot suspect
and is retried by the next request.

| Failure                                                                                    | Append                                                               | Forget                                   |
| ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------- | ---------------------------------------- |
| `SQLITE_BUSY` before any write                                                             | `store_busy`                                                         | `store_busy`                             |
| `SQLITE_FULL` from a statement before COMMIT                                               | ROLLBACK → `low_disk`                                                | ROLLBACK → re-check                      |
| Any other error before COMMIT                                                              | ROLLBACK → re-check                                                  | ROLLBACK → re-check                      |
| Any error returned by COMMIT, including `SQLITE_FULL`, IOERR, and ENOSPC or EIO from fsync | `commit_indeterminate`. Mark the project suspect and reopen in place | Same                                     |
| Panic. `catch_unwind` sits inside the guard's scope, so the mutex is never poisoned        | Same as a COMMIT error                                               | Same                                     |
| Re-check: match and the project is not suspect                                             | Original receipt                                                     | Already tombstoned → original `redacted` |
| Re-check: mismatch                                                                         | `key_conflict` or `key_redacted`                                     | —                                        |
| Re-check: row absent, or still live for forget                                             | `commit_failed`                                                      | `commit_failed`                          |
| Re-check: error, or the project is suspect                                                 | `commit_indeterminate`                                               | `commit_indeterminate`                   |

### Crash windows

| Window | Point of failure                                   | State after restart                                                                                        | Client saw                  | Resolution                                                                                                                                                                                                            |
| ------ | -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| W0     | Before any frame is written                        | Nothing written                                                                                            | Transport error             | The recovery re-append inserts the row (201)                                                                                                                                                                          |
| W1     | Commit frame written, before fsync                 | Frames are in the page cache and the next opener recovers them. A power loss before the barrier loses them | Transport error             | The barrier runs first. Recovery then gets 200 (the row is now durable) or 201 (the frames were lost). Before the barrier finishes, a hit returns `commit_indeterminate`, which the client reports as `Indeterminate` |
| W2     | After fsync, before the reply                      | Durable                                                                                                    | Transport error or deadline | Recovery gets the original receipt (200)                                                                                                                                                                              |
| W3     | COMMIT returns an error (EIO or ENOSPC from fsync) | Unknown, because failed pages may be marked clean                                                          | `commit_indeterminate`      | The project is suspect, so recovery gets `Indeterminate`. After a completed checkpoint, a same-key retry gets 200 or 201                                                                                              |

A timed-out client drops its connection, but the blocking task keeps its permit
and may commit after the recovery attempt: harmless, since the first to commit
inserts and the other replays.

### Recovery flow

Each attempt is bounded by `GenericEventOptions::attempt_timeout` (default 3 s:
1 s busy, about 250 ms fsync, margin; worst case about 6 s), as are lookup and forget.

| Append, first attempt                                                                      | Action                                                                |
| ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- |
| 200 or 201, any 4xx, 409, 503, 507, `commit_failed`                                        | Terminal. Mapped per the [code table](#error-codes-and-invariants)    |
| No byte sent                                                                               | `Unavailable`                                                         |
| Bytes sent then a transport failure, a deadline, `commit_indeterminate`, or an unknown 5xx | **Exactly one** re-append of the identical envelope with the same key |
| **Recovery attempt:** 200 or 201                                                           | The receipt                                                           |
| **Recovery attempt:** 409 `key_conflict` or `key_redacted`                                 | `Conflict(Mismatch)` or `Conflict(Redacted)`                          |
| **Recovery attempt:** anything else                                                        | `Indeterminate { message }`, naming the result                        |

"Anything else" includes a bare 404/405, a missing feature,
`unsupported_envelope_version`, 401/403, 413, any other 4xx or 5xx, a transport
failure and a deadline: the reconnect may have auto-spawned a different daemon
(such as a 0.4 binary from `PATH` or a hook), so only the row's own state counts.

Forget gets one idempotent re-send under the same conditions; on it, `redacted`
or `notFound` is terminal and anything else is `Indeterminate`. Lookup does not
retry. `Indeterminate` is always retry-safe with the same key; nothing is spooled.

### Quota and low disk

`maxPayloadBytes` is fixed. `projectQuotaBytes` (optional, off by default) caps
live payload bytes in `generic_event_usage`; tombstones free theirs. The low-disk
floor is a cached, unadvertised free-space probe refreshed outside the lock. Its
64 MiB default, the configuration surface and the probe dependency are
**proposals** pending [open questions](#open-questions) 3–5.

### Concurrency and shutdown

- **Runtime:** `commands/serve.rs:33` gets a 2–4 worker `serve_runtime()`; the
  shared `runtime()` (`kindling/src/lib.rs:221`) stays `current_thread`.
- **Permit:** keyed append and forget `acquire_owned()` the per-project
  `Semaphore(1)` and move the `OwnedSemaphorePermit` into the `spawn_blocking`
  closure, so a dropped handler future cannot release it early. Import is
  in-process only, serialised by the SQLite write lock (if the daemon ever serves
  it, same path).
- **Fair lock:** `std::sync::Mutex` grants waiters in no particular order, so a
  queued keyed task could repeatedly win the lock ahead of an ordinary waiter.
  The per-project service lock therefore becomes FIFO-fair, for example
  `parking_lot::FairMutex` (`parking_lot` 0.12.5 is already in `Cargo.lock`);
  it has no poisoning, which also removes the poison path for ordinary handlers.
- **True bound:** with the fair lock and the keyed permit, an ordinary append on
  the same project waits for at most one keyed transaction or barrier attempt
  (≤ 1 s busy plus its fsyncs), plus one barrier per open. Ordinary handlers still lock inline, so with concurrent
  same-project requests ≥ `serve` workers, other projects, `/v1/health` and a
  `current_thread` embedding host can stall up to that bound.
- **Shutdown:** `serve` shutdown and `Runtime::shutdown` wait for in-flight keyed
  and forget blocking tasks; `abort_and_join` alone does not stop them.

### Guarantee and macOS caveat

A receipt implies the row survives any process crash, and power loss on a local
disk that honours cache flush; on macOS the latter holds only while every writer
of the project DB is 0.5+ (0.4 checkpoints with a plain fsync), so the upgrade
note says stop 0.4 hooks and daemons. Network filesystems and foreign tools that
checkpoint without `checkpoint_fullfsync` are excluded. `AppState` keeps
`AtomicU64` counters (inserted, matched, conflict, redacted_conflict, rejected,
busy, low_disk, commit_failed, indeterminate, keyed_suspect, max/last commit µs),
not on Capability; errors and commits over 100 ms go to stderr, payloads never.

## Secret detection, forget and export/import (T4)

### GenericEventDetector v1 rules

One rule table, `crates/kindling-service/src/filter/rules.rs`, serves two
profiles. Observation masking keeps today's seven rules unchanged (a parity test
pins the shared bodies). The v1 profile is exactly the block below, each rule
compiled as a `regex::bytes::Regex` with Unicode off (`(?-u)`), so negated
classes match any byte and non-UTF-8 input is scanned as is. All rules are
case-sensitive except `credentialField`. Minimum lengths follow `secrets.rs`:
`sk-ant-` 20, `sk-` 40, `Bearer`/`Basic` 20, `gh?_` 36, `AKIA` exactly 16;
`credentialField` values need 8.

```text
anthropicKey     (?:^|[^A-Za-z0-9_-])sk-ant-[A-Za-z0-9\-_]{20,}
openaiKey        (?:^|[^A-Za-z0-9_-])sk-[A-Za-z0-9]{40,}
githubToken      (?:^|[^A-Za-z0-9_-])gh[pousr]_[A-Za-z0-9]{36,}
awsAccessKeyId   (?:^|[^A-Za-z0-9_-])AKIA[0-9A-Z]{16}
bearerToken      Bearer\s+[A-Za-z0-9\-._~+/]{20,}
basicAuth        Basic\s+[A-Za-z0-9+/]{20,}={0,2}(?:[^A-Za-z0-9+/=]|$)        # match discarded if the value contains "//"
privateKeyBlock  -----BEGIN [A-Z ]*PRIVATE KEY-----
credentialField  (?i)(?:^|[^A-Za-z0-9_-])(?:password|passwd|secret|token|api[-_]?key|access[-_]?token|refresh[-_]?token|client[-_]?secret|secret[-_]?key|private[-_]?key)["']?\s*[:=]\s*["']?[^\s"']{8,}
```

`schema/fixtures/generic-event-detector-v1.json` carries these bodies plus clean
and rejected vectors (including key cases) for anvil CI; tests assert the fixture
bodies equal this block line for line and the compiled table equals the fixture.
Excluded: the substring `credentialAssignment` rule, the shell and URL transforms,
and the `hook/filter.rs` heuristic (it rejects every sha256 hex; a test asserts
the generic path never calls it).

| Surface      | Screening                                                                                                                                                           |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `payload`    | All v1 rules, on raw bytes, with no decoding                                                                                                                        |
| `scopeIds.*` | All v1 rules                                                                                                                                                        |
| `key`        | Grammar check, plus `anthropicKey`, `openaiKey`, `githubToken` and `awsAccessKeyId`. Keys appear in clear in receipts, exports, errors and logs, and survive forget |
| `schemaName` | Grammar check only                                                                                                                                                  |

Keys `kfit/run-42/evt-7`, `task-<sha>`, `desk-<sha>` and `risk_sk-<sha>` pass;
`approval:sk-<40hex>`, `run/sk-<40hex>`, `k.sk-<40hex>`, `AKIA…` and `ghp_…` are
rejected. `"token": "<uuid>"` is a known false positive (anvil renames, e.g.
`idempotencyToken`). A rejection returns `{field, classes, count,
detectorVersion}`, never matched text or offsets, reserves nothing and is never
spooled. Detection counts only on a key miss, so a rule change (which bumps
`secretDetectorVersion` with a CHANGELOG entry) never breaks replay. There is no
allow-list in 0.5.0: rename the field and retry with the same key.

### Forget

Forget clears the payload and records `redacted_at`. It keeps the key, scope,
digest, event id, `committed_at` and `detector_version`. Forget is **logical**:
residual bytes may remain in the WAL, in free pages, in backups and in older
bundles. The docs must never claim that forget erases data.

### Export bundles

| Content                                 | `bundleVersion` and `dataset.version` | Readers                    |
| --------------------------------------- | ------------------------------------- | -------------------------- |
| No generic events and no tombstones     | `"1.0"`, byte-identical to 0.4        | 0.4 and 0.5                |
| Any generic events or tombstones        | `"2.0"`                               | 0.5 only. 0.4 fails loudly |
| `"1.0"` bundle carrying `genericEvents` | Invalid                               | 0.5 rejects it             |

`genericEvents` is omitted when empty. Tombstones are always exported, whatever
`include_redacted` says, and `limit` never truncates generic events. A
scope-filtered bundle is not a complete key reservation.

A live row is `{eventId, envelopeVersion, schemaName, schemaVersion, key, scope,
digest, committedAt, detectorVersion, payload}` (payload base64). A tombstone is
`{eventId, envelopeVersion, schemaName, schemaVersion, key, scope, digest,
committedAt, redacted: true, redactedAt, detectorVersion}`, with no payload field.

### Import

When a bundle carries generic events, every generic-event check runs before any
entity write, inside the same `durable_write`. Import recomputes SHA-256 for every
live row (a mismatch fails the whole import naming the key), does not re-run the
detector, and reports rows with an older `detectorVersion`; `dry_run` reports the
redacted count.

| Incoming \ existing | None             | Live with same key, scope and digest                                             | Tombstone with same key, scope and digest | Different scope or digest             |
| ------------------- | ---------------- | -------------------------------------------------------------------------------- | ----------------------------------------- | ------------------------------------- |
| Live                | Insert           | Skip                                                                             | Skip; never resurrect                     | Fail the whole import, naming the key |
| Tombstone           | Insert tombstone | Redact the live row (one-way; documented exception to "import never overwrites") | Skip                                      | Fail the whole import, naming the key |

**Trust boundary** (for `SECURITY.md` and `docs/data-model.md`): same UID only
(socket 0600, directory 0700; `X-Kindling-Project` routes, never authenticates);
payloads are plaintext at rest and in bundles; the detector is best effort and
imports are not re-screened; digests are unkeyed, not attestation (anvil signs);
forget is logical; for CodeQL the detector is not a sanitiser.

## Mixed-version matrix (T1)

| Combination                                                           | Ordinary operations                              | Generic events                                                                         |
| --------------------------------------------------------------------- | ------------------------------------------------ | -------------------------------------------------------------------------------------- |
| 0.5 client or runtime → 0.4 daemon already on the socket              | Work (wire version 6)                            | `Unsupported` until the daemon restarts on 0.5                                         |
| 0.4 client, hooks, VSIX or TS client → 0.5 daemon                     | Work                                             | Not visible                                                                            |
| 0.4 in-process binary on a revision-7 DB                              | Work (`user_version` is still 6)                 | Invisible but preserved. A 0.4 export omits them                                       |
| 0.5 on a 0.4 DB                                                       | Work                                             | A writable open migrates 007 and runs the barrier. A read-only open returns `notFound` |
| 0.4 importing a `"2.0"` bundle, or 0.4/0.5 importing a `"1.0"` bundle | Fails loudly for `"2.0"`; `"1.0"` works as today | —                                                                                      |
| 0.5 client → a daemon with `wireVersion` above 6                      | `SchemaMismatch` (upper bound)                   | —                                                                                      |
| macOS, with a 0.4 process writing the same DB                         | Work                                             | Durable against process crash. Power-loss guarantee void until every writer is 0.5     |
| Tools that read `user_version` as the revision                        | —                                                | Must read `MAX(schema_migrations.version)`                                             |

## Consumer guidance for anvil

- **Dedicated `kindling_home`** (`RuntimeConfig::with_home`), so a hook-spawned
  0.4 daemon on the default socket cannot leave generic events `Unsupported`.
- **One canonical absolute `project_root`.** Routing hashes the raw string, so
  `/repo` vs `/repo/`, symlinks and macOS temp dirs (`/var/…` vs `/private/var/…`)
  reach different DBs, where lookup returns `notFound` and looks like data loss.
  Canonicalisation is follow-up and open question 7.
- **Fixture assertions:** `receipt.scope.projectId == runtime.project_id()`
  before the lookup, and a lookup under a different root returns `notFound`
  (amendment 6).

## Performance contract and bench (T3, G-2)

KINTEG-013/014 budgets still apply (warm ordinary append p95 < 1 ms, outage
append p95 < 5 ms; `daemon_group`/`spool_group` in
`crates/kindling-bench/src/workloads.rs`). A `keyed-durable` group joins
`GROUP_NAMES` (`workloads.rs:75`), run with `--only-group keyed-durable`:

| Workload                                 | Shape                                                                         | Status                                                                      |
| ---------------------------------------- | ----------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `ordinary-append-mixed`                  | Closed-loop ordinary writes, with keyed writes at 5/s on the same project     | **Ext4 gate:** ordinary p95 < 1 ms. p99 and max reported                    |
| `keyed-append-replay`                    | Same-key replays                                                              | **Ext4 gate:** p95 < 1 ms (shows that a replay never fsyncs)                |
| `keyed-append-durable`, sweep, open loop | Unique keys; keyed at 1, 5, 20 and 50/s, plus saturated; an open-loop variant | Reported, with the knee documented                                          |
| APFS run                                 | Same workloads on macOS                                                       | Reported only, including ordinary **p99** (the `checkpoint_fullfsync` cost) |

The gate runs at release prep on a real local ext4 disk with
`durability_measured=true`, median of 5 runs; the report records filesystem,
synchronous mode and fullfsync, and tmpfs cannot pass. **If the ext4 gate fails,
the release is blocked and the decision returns to the maintainer.** Documented
APFS limit: `F_FULLFSYNC` takes about 10–30 ms, so keyed writes at 5/s hold the
lock 5–15 % of the time and may exceed the ordinary p95 budget. Stated rule:
ordinary p95 degrades once keyed rate × fsync time exceeds about 5 %. CI runs
smoke only, no threshold. Results go in ENGINEERING-HISTORY.

## Test and validation plan

| Area                        | Test                                                                                                                                                                                                                                                                                                                                                                     | Item       |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- |
| Digest                      | The three golden vectors from `schema/fixtures/generic-event-digest-v1.json`                                                                                                                                                                                                                                                                                             | 015a       |
| Detector                    | Fixture vectors, including the key cases. The fixture's rule bodies equal the spec block, and the compiled table equals the fixture. The masking parity test passes                                                                                                                                                                                                      | 015a       |
| Envelope, types, wire range | Grammar boundaries (0, 1, max, max+1). Strict base64 (rejects unpadded input, the URL alphabet and whitespace). `""` scope is rejected. ts-rs types `payload` as string and timestamps as number. Rust and TS accept 6 and reject 5 and 7. The parse trap is covered by a `version.json` whose first `"version"` is 7                                                    | 015a       |
| Schema                      | Migration 007 is idempotent when opens race. `schema_sql_pins_user_version` (`kindling-store/src/schema.rs:59`) and `kindling-store/tests/store_integration.rs:64` are rewritten. No-coupling golden test. Open rules for each `user_version` row. The 0.4.1 gate accepts a revision-7 DB and can still INSERT, UPDATE and DELETE observations                           | 015a       |
| **Barrier**                 | Reopen a DB whose WAL holds an unsynced commit frame. The barrier must complete before any replay or `found`. While suspect, a hit returns `commit_indeterminate` or `durability_unconfirmed`, ordinary routes are unaffected, and health still advertises                                                                                                               | 015b       |
| COMMIT errors, pragma state | An injected COMMIT error or panic returns `commit_indeterminate`, never a receipt. The project becomes suspect and the mutex is not poisoned. `SQLITE_FULL` before COMMIT returns `low_disk`. FULL inside the closure. NORMAL and a 5 s busy timeout restored after success, error and panic. `checkpoint_fullfsync=ON` on writable connections                          | 015b       |
| Keyed path and lookup       | Insert, byte-identical replay, both conflicts, detector hit only on a miss, quota, injected low-disk probe. A tampered row returns `digest_mismatch`. Read-only open returns `notFound`. The generic path never calls `hook/filter.rs`                                                                                                                                   | 015b       |
| Export and import           | `"1.0"` is byte-identical; `"2.0"` includes tombstones (with `envelopeVersion`); a `"1.0"` bundle carrying `genericEvents` is rejected. A mismatch on the last row writes nothing. The precedence table holds. `"1.0"` import behaves as today                                                                                                                           | 015b       |
| Daemon                      | Every code, status and `details` shape. A 413 with and without a body. Capability golden shape and tolerance of unknown features                                                                                                                                                                                                                                         | 015c       |
| Concurrency                 | More concurrent ordinary appends than workers during an injected slow keyed commit, and `/v1/health` still responds. A client deadline that fires mid-fsync leaves the ordinary wait within one keyed transaction, and the permit is held until the blocking task ends                                                                                                   | 015c       |
| Recovery                    | Deadline, transport drop after send, and `commit_indeterminate` each cause exactly one re-append. Every non-receipt, non-409 recovery result (404, 413, 401, 503, 507, 500) returns `Indeterminate`. The send boundary returns `Unavailable` only before the first byte                                                                                                  | 015c       |
| **Negative**                | For quota, low disk, outage, busy, failed commit, indeterminate, rejected and conflict, through both the client and the runtime: no receipt, the spool file is untouched, and no `Spooled` outcome                                                                                                                                                                       | 015c, 015d |
| Forget                      | Forget is idempotent with its original `redactedAt`. A later append returns `key_redacted`. A re-send returns `redacted` or `notFound` as terminal, and anything else returns `Indeterminate`                                                                                                                                                                            | 015d       |
| Failpoint crash, nightly    | `--features test-failpoints`. `KINDLING_FAILPOINT=keyed_before_commit` gives `notFound` then a fresh insert. `keyed_after_commit` gives the same id and digest with one row. Runs on ubuntu, macos (**new job**) and windows (over TCP). A **new workflow** runs a random-SIGKILL loop and checks that every receipt matches a row. An optional LazyFS job runs on Linux | 015d       |
| Runtime                     | Feature check before the first write and after a reconnect. Attaching to a 0.4-like daemon returns `Unsupported`. `settings_applied` is reported. `project_id()` matches the receipt. A lookup under a different root returns `notFound`. Shutdown waits for blocking tasks                                                                                              | 015d       |
| Release                     | The pre-publish packaged check and the post-publish install (see [Delivery](#delivery-and-release))                                                                                                                                                                                                                                                                      | 015e       |

## Delivery and release

| Item | Scope                                                                                                                                                   | Advertised                  |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| 015a | Types, ts-rs, detector rule table and fixtures, revision 7, version stamps, client range check, digest vectors                                          | No                          |
| 015b | `durable_write`, barrier and suspect state, the keyed sequence, quota and floor, tombstone forget in the store, export and import                       | No                          |
| 015c | Append and lookup routes, codes, `serve` runtime and concurrency, client append and lookup with recovery, counters, `test-advertise`                    | Only under `test-advertise` |
| 015d | Forget route, client and runtime methods, the runtime facade, advertising the feature, failpoint crash tests, the new macOS CI job and nightly workflow | **Yes**                     |
| 015e | Release prep: bench gate, docs sweep, pre-publish packaged check, publish, post-publish install                                                         | —                           |

- **Release freeze:** no release is cut from `main` between the 015a and 015d
  merges; any 0.4.x patch comes from a `release/0.4` branch.
- **Per-item records:** each sub-item adds its own `Unreleased` CHANGELOG and
  ENGINEERING-HISTORY entries and runs `python3 scripts/check-release-docs.py`.
- **Pre-publish packaged check (015e), on the release commit:**
  `cargo package --workspace`; a scratch crate outside the workspace runs
  commit, restart and lookup against the packaged contents (`[patch.crates-io]`
  on `target/package/<crate>-0.5.0/`, or a local registry); the failpoint suite;
  `DRY_RUN=1 scripts/publish.sh`. Then `scripts/publish.sh`, tag `v0.5.0`
  (`release.yml`), and a clean install from crates.io as confirmation.

## Documentation and release notes

- `schema/README.md`, `schema/schema.sql:11`, `version.json` + vendored copies:
  minReader, revision, `wireVersion`, additive rule, migration procedure.
- `CONTEXT.md:219`: contract wording; terms (generic event, keyed append,
  receipt, digest, tombstone, keyed-suspect, capability feature).
- `docs/architecture.md:385`: schema version/revision, endpoints, client and
  runtime surface, `serve` runtime and wait bound. `docs/data-model.md:247,259`:
  `generic_events`, bundle `"1.0"`/`"2.0"`, trust boundary, logical forget.
- `docs/development.md`: invariants 2 and 5, endpoint recipe (no TS mirror,
  Q-03), schema recipe, failpoint tests, fixture drift gates.
- `docs/integrations.md` (client and runtime only, not embedded
  `kindling-service`); `SECURITY.md` ("Generic events", CodeQL disposition);
  runtime, client and bench READMEs and `AGENTS.md`.

**CHANGELOG:** _Added_ `generic-event@1` (append, lookup, forget) and Capability
`features`. _Changed_ schema revision 7 (`user_version` still 6), the client
range check, bundle `"2.0"`, the multi-thread `serve` runtime, the barrier
checkpoint on writable open, and macOS `checkpoint_fullfsync`. _Security_ the
detector, the trust boundary and logical forget. _Upgrade_ restart the daemon
after upgrading; 0.4 cannot see or export generic events or import `"2.0"`
bundles; on macOS stop 0.4 hooks and daemons; tools must read
`schema_migrations`, not `user_version`; use a canonical `project_root`.

**ENGINEERING-HISTORY:** the council and Phase 4 review; the version-stamp split;
`durable_write`, the barrier and rejected alternatives; re-append versus lookup;
the detector false-positive analysis; bench numbers including APFS; the
crash-test approach.

## Discovered follow-up work

Not added to APS here: reconcile the `hook/filter.rs` detector drift against the
shared rule table; add `privateKeyBlock` to observation masking; physical erasure
on forget (`secure_delete` plus a checkpoint, without taxing ordinary appends);
expose the keyed counters; canonicalise `project_root` (`/repo/`, symlinks,
`/private/var`); move all project handlers to `spawn_blocking` and re-measure
p95; a runtime attach policy that starts its own embedded daemon when the
attached one lacks `generic-event@1`; listing and pagination as
`generic-event-list@1`.

## Amendments to the original KINTEG-015 wording

1. Uniqueness is per `(schemaName, key)` within a project store, not profile-wide.
2. The closed failure sets gain `Indeterminate` (append and forget).
3. `StoreBusy` folds into `FailedCommit { kind: Busy }`.
4. A forget endpoint is added.
5. Final 0.5.0, no release candidate, not coordinated with C14.
6. "Scope/digest mismatch on lookup" becomes two checks: a lookup under a
   different project root returns `notFound` (with `receipt.scope.projectId`
   asserted), and a tampered row returns `lookup_failed`/`digest_mismatch`.
7. **Change to Q-07:** ordinary append still commits NORMAL, but on macOS
   `checkpoint_fullfsync` adds `F_FULLFSYNC` to auto-checkpoints ordinary commits
   trigger (~1 in 150–250), plus one barrier checkpoint per writable open (APFS p99 benched).

## Open questions

1. **anvil:** does per-project key uniqueness (amendment 1) meet KFIT-005?
2. **anvil:** confirm the key grammar and `anvil.governance.v1` field naming
   against the detector fixtures, including the `"token": "<uuid>"` false positive.
3. **Low-disk floor default:** 64 MiB is proposed.
4. **Settings surface and the Q-09 tension.** Embedded daemons get
   `GenericEventSettings` through `RuntimeConfig`; `ServerConfig` has pub fields
   and is not `#[non_exhaustive]`, so it needs a separate entry point. The shared
   `kindling serve` (auto-spawned or started by hooks) cannot receive a cap
   without a `serve` flag or config file, a Discuss-first CLI change. Does a
   `RuntimeConfig`-only cap meet Q-09?
5. **Free-space probe:** which crate, under `unsafe_code = "forbid"`? (`rustix` is
   locked for Unix; a Windows dependency may be Discuss-first.)
6. **New CI:** approve a macOS test job and a nightly crash workflow (neither exists).
7. **`project_root` canonicalisation:** document only in 0.5.0 (proposed), or fix
   before `@1` is frozen?
8. **Barrier cost on CLI writable opens:** one checkpoint per open (skipped with no generic events)?
