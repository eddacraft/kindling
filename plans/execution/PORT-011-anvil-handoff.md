# PORT-011: anvil integration handoff

**From:** kindling (eddacraft/kindling)  
**To:** anvil (eddacraft/anvil-001) — KDS module / `KindlingDaemonSink`  
**Date:** 2026-06-24  
**Completed:** 2026-06-24 — anvil PR #2897 (KDS-001 + KDS-003) + #2906 (KDS-002).  
**Status:** Done (kindling PORT-011 Merged; anvil follow-on KDS-004/005 blocked on kindling D-009)

This document is the implementation guide anvil needs to complete **PORT-011**
(kindling) and **KDS-003** (anvil): prove direct Rust-to-Rust observation emit
with no TypeScript bridge.

---

## What “done” means

| Side     | Item              | Acceptance                                                                                                                                                          |
| -------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| kindling | PORT-011          | anvil lands a parity test: same `CommandInvokedObservation` input → identical persisted row via daemon vs today’s NDJSON path (modulo daemon-assigned `id` / `ts`). |
| anvil    | KDS-001 + KDS-003 | `KindlingDaemonSink` in `anvil-cli` calls `kindling-client` (+ spool); parity test green in CI.                                                                     |

**Minimum scope for PORT-011:** one sink implementation for `command.invoked`
only. Full KDS-002..005 (sink selection, usage views, NDJSON retirement) can
follow in the same branch or immediately after — they are not blockers for
closing PORT-011.

**Explicitly out of scope for PORT-011:**

- `kindling-runtime` facade (KINTEG-008) — use raw `kindling-client` first;
  migrate to runtime after the proof documents baseline wiring pain.
- Replacing `@eddacraft/anvil-kindling-integration` npm package (PORT-019) —
  that deprecation tracks anvil’s full cutover, not this proof slice.
- Daemon-side dedup-on-id (KINTEG-002) — spool is at-least-once in v1.

---

## Prerequisites (satisfied)

- All seven kindling workspace crates published at **0.2.0** on crates.io
  (KINTEG-001, 2026-06-24).
- `kindling-client` ships `SpooledClient` behind the **`spool` feature** — there
  is **no** standalone `kindling-spool` crate.
- Daemon auto-spawn on first client call (`kindling serve --daemonize`) is
  production-ready.

Scratch verification:

```bash
cargo new /tmp/kindling-scratch && cd /tmp/kindling-scratch
cargo add kindling-client@0.2 --features spool
cargo check
```

---

## Architecture constraints (anvil)

These are non-negotiable; they come from anvil’s own KDS draft and ADR-064.

1. **Sink lives in `anvil-cli`, never `anvil-intercept`.** The networking
   client must not cross the daemon dependency boundary. `daemon_dep_boundary`
   tests must stay green.
2. **Producers unchanged.** USAGE / DPO emitters keep calling
   `KindlingObservationSink`; only the sink implementation swaps from NDJSON
   append to daemon `append_observation`.
3. **Non-blocking emit.** Wrap the daemon sink in `NonBlockingObservationSink`
   (same pattern as `DaemonUsageSink` today) so dispatch / save-time paths never
   block on socket I/O.
4. **Default to daemon path** for any observation that could overlap with
   interactive Claude Code on the same project. In-process `kindling-service` is
   a follow-up, not PORT-011.

---

## Dependency

In `crates/anvil-cli/Cargo.toml`:

```toml
kindling-client = { version = "0.2", features = ["spool"] }
```

Pin policy: caret on `0.2` is fine; `Client::health()` fails loud on schema
version mismatch against the client’s compile-time `EXPECTED_SCHEMA_VERSION`
(currently **5**, from `schema/version.json`).

Do **not** add `kindling-server`, `kindling-store`, or `kindling-service` to
`anvil-cli` for PORT-011 — the client auto-spawns the daemon binary on `PATH`
(or use an in-process test daemon; see Testing).

---

## Wire mapping (mirror the TS adapter)

The TS bridge in `packages/kindling-integration/src/adapter.ts` is the canonical
mapping. Reproduce it in Rust for every anvil observation kind you route through
the daemon.

### Kind map

| anvil `kind` (wire)   | kindling `ObservationKind` |
| --------------------- | -------------------------- |
| `command.invoked`     | `Command`                  |
| `gate_evaluated`      | `Command`                  |
| `action_executed`     | `Command`                  |
| `error`               | `Error`                    |
| all other anvil kinds | `Message`                  |

For PORT-011, implement at least `command.invoked` → `Command`.

### `ObservationInput` shape

```rust
use kindling_client::{ObservationInput, ObservationKind, ScopeIds};
use serde_json::{json, Map, Value};

fn to_kindling_input(
    anvil_obs: &CommandInvokedObservation, // or GateEvaluatedObservation, etc.
    repo_id: Option<&str>,
) -> ObservationInput {
    let content = serde_json::to_string(anvil_obs).expect("anvil obs serialises");

    let mut provenance = Map::new();
    provenance.insert("anvil_kind".into(), json!(anvil_obs.kind));
    provenance.insert(
        "anvil_contract_version".into(),
        json!("1.0.0"), // OBSERVATION_CONTRACT_VERSION
    );

    ObservationInput {
        id: None, // let client/spool assign stable v4 before spool
        kind: ObservationKind::Command,
        content,
        provenance: Some(provenance),
        ts: parse_rfc3339_ms(&anvil_obs.timestamp), // or None → daemon assigns
        scope_ids: ScopeIds {
            session_id: Some(anvil_obs.session_id.clone()),
            repo_id: repo_id.map(str::to_string),
            ..Default::default()
        },
        redacted: None,
    }
}
```

**TRACE-003 redaction** stays on the anvil side (already applied before the sink
receives the row). Kindling adds its own non-bypassable secret masking at the
service boundary.

### `project_root` / `repo_id`

`kindling-client` routes per-project SQLite via `ClientConfig::project_root`
(sent as `X-Kindling-Project` on every data call). Set this to the anvil
project / workspace root the observation belongs to — the same string you would
use as `repoId` in the TS adapter.

---

## `KindlingDaemonSink` behaviour

Implement `KindlingObservationSink` in `anvil-cli` (suggested:
`crates/anvil-cli/src/kindling_daemon_sink.rs`).

### Construction

- Build `kindling_client::Client::with_config(...)` once (or lazily) with:
  - `project_root` = workspace root for the emitting context
  - default socket under `~/.kindling/kindling.sock` (client default)
- Wrap in `kindling_client::spool::SpooledClient` with spool path e.g.
  `<credentials_dir>/kindling/spool.ndjson` (not `usage.ndjson` — that file is
  the legacy sidecar KDS-005 retires).

### `try_emit_command_invoked`

1. Map observation → `ObservationInput` (above).
2. `spooled.append_observation(input, None, Some(true)).await` (or sync bridge
   via `tokio::spawn` / dedicated runtime thread if the call site is sync).
3. Outcomes:
   - `AppendOutcome::Delivered` → `Ok(())`
   - `AppendOutcome::Spooled` (daemon down) → `Ok(())` — **never** surface
     connectivity failure to the caller (matches today’s best-effort NDJSON
     contract).
   - `SpoolError::Client(ClientError::Api { .. })` (daemon rejected) →
     `Err(KindlingSinkError::Unavailable(...))` or a dedicated rejection variant.

Only connectivity failures (`Unavailable`, `Http`) spool. Schema mismatch,
validation errors, and API rejections must propagate — never spool them.

### Other trait methods

For PORT-011 minimum: default `try_emit` / `try_emit_constraint_applied` to
`Ok(())` (same as `DaemonUsageSink` today). KDS-003 parity for `gate_evaluated`
can be a fast follow once `command.invoked` is green.

---

## Parity test (PORT-011 / KDS-003 acceptance)

Add an integration test in `anvil-cli` (e.g. `tests/kindling_daemon_parity.rs`):

1. **Fixture:** one canonical `CommandInvokedObservation` (copy from
   `anvil-intercept` test fixtures or `command-invoked.test.ts` golden).
2. **NDJSON path:** append via existing `append_usage_observation_to` to a temp
   file; parse the line back.
3. **Daemon path:** start an in-process daemon on a temp socket (pattern below),
   emit via `KindlingDaemonSink`, read back from the store.
4. **Assert equality** on:
   - deserialised anvil payload inside `content` (round-trip JSON)
   - `provenance["anvil_kind"]` == `"command.invoked"`
   - `scope_ids.session_id`
   - kindling `kind` == `command`
5. **Ignore** `id` and `ts` (daemon-assigned).
6. **Spool replay:** stop daemon → emit (spooled) → restart → `flush()` → row
   retrievable with same `content` / provenance.

### In-process daemon test pattern (from kindling)

kindling’s client tests spin up a real `kindling-server` on a temp UDS. anvil
can either:

- **Dev-dep** `kindling-server` + `tempfile` in the parity test only, copying
  the support helpers from
  `crates/kindling-client/tests/support/mod.rs`, or
- **Path-dep** kindling client tests as reference and reimplement the ~40-line
  `TestDaemon` helper locally.

Example read-back after append:

```rust
let result = client
    .retrieve(RetrieveOptions {
        query: "command.invoked".to_string(), // or search on session_id
        scope_ids: scope.clone(),
        token_budget: None,
        max_candidates: Some(10),
        include_redacted: None,
    })
    .await?;
// Inspect result.provider_hits or pins for the appended observation
```

---

## Suggested implementation sequence

| Step | Action                                                                     | Checkpoint                         |
| ---- | -------------------------------------------------------------------------- | ---------------------------------- |
| 1    | Add `kindling-client` dep; `to_kindling_input` mapper + unit test          | Mapper round-trips golden JSON     |
| 2    | `KindlingDaemonSink` + `try_emit_command_invoked`                          | Delivered-when-up integration test |
| 3    | Spooled-when-down + replay-on-reconnect test                               | Spool file drains after flush      |
| 4    | NDJSON vs daemon parity test (KDS-003)                                     | CI green — **closes PORT-011**     |
| 5    | (Optional) Wire behind config flag `daemon` \| `ndjson` \| `off` (KDS-002) | Default privacy contract unchanged |

---

## Corrections to the KDS draft module

Update `plans/modules/kindling-daemon-sink.aps.md` in anvil when starting work:

| Draft says                      | Reality (2026-06-24)                                 |
| ------------------------------- | ---------------------------------------------------- |
| Separate `kindling-spool` crate | `kindling-client` feature `spool` only               |
| Blocked on crates.io `>=0.1`    | **0.2.0 published** — Ready Checklist item 1 is done |
| `kindling_spool::SpooledClient` | `kindling_client::spool::SpooledClient`              |

Move module status from **Proposed** → **Ready** after anvil council accepts the
placement decisions below.

---

## Open decisions (anvil-side)

Record answers in the KDS module before merge:

1. **Crate placement** — `anvil-cli` only vs small `anvil-kindling` adapter
   crate (still app-layer). PORT-011 can land in `anvil-cli`; extract later if
   JSON-RPC and CLI both need the sink.
2. **Async bridge** — `SpooledClient` is async; `KindlingObservationSink` is
   sync. Options: dedicated tokio runtime on the drain thread (fits
   `NonBlockingObservationSink`), or `tokio::runtime::Handle::block_on` inside
   the drain worker only (never on the hot path).
3. **Usage views (KDS-004)** — defer until after PORT-011; views can keep
   reading `usage.ndjson` until the daemon read path is agreed.

---

## kindling contacts / references

| Topic                   | Location                                                               |
| ----------------------- | ---------------------------------------------------------------------- |
| Client + spool API      | `crates/kindling-client/README.md`, `src/spool.rs`                     |
| Spool integration tests | `crates/kindling-client/tests/spool.rs`                                |
| Domain types            | Re-exported from `kindling_client::*`                                  |
| Schema version          | `schema/version.json` (currently v5)                                   |
| Capability handshake    | `GET /v1/health`, `client.health().await`                              |
| Runtime facade (later)  | `plans/specs/2026-06-24-kindling-runtime-design.md`                    |
| Dedup follow-up         | KINTEG-002 in `plans/modules/06-downstream-integration-surface.aps.md` |

---

## Closing the loop

When anvil merges the parity test:

1. anvil: mark KDS-001 + KDS-003 **Complete**; link PR in KDS module.
2. kindling: mark PORT-011 **Merged** in `plans/modules/05-rust-port.aps.md` and
   check the box in `plans/reviews/post-merge/feat-kinteg-001-publish-readiness.md`.
3. kindling: check index success criterion — “anvil emits observations directly
   via `kindling-client` — no TS bridge” — for the `command.invoked` path.
