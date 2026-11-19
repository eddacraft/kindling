# GH-216: pin the keyed-object shape of `scopeIds` at the API boundary

**Authority:** GitHub issue #216 (no APS work item; regression test and doc
clarification, not a Discuss-first lane)
**Date:** 2026-11-19
**Risk:** standard (tests and docs only; no production code change)

## Context

The retired TypeScript validator accepted arrays as `scopeIds`. Triage on #216
found the Rust boundary already rejects them: `ScopeIds`
(`crates/kindling-types/src/common.rs`) is a plain derived struct with no
`flatten` or `untagged`, so serde's visitor accepts only a JSON map, and the
server's `axum::Json<T>` extractor turns the type error into a 4xx. Nothing pins
that behaviour, so a future change (for example `#[serde(untagged)]` or a custom
`Deserialize`) could silently reopen it.

## Work item

- **Intent:** a JSON array in `scopeIds` is rejected at both the type layer and
  every v1 endpoint that accepts `scopeIds`, and the contract says so.
- **Expected Outcome:**
  - `kindling-types` test: `ScopeIds` rejects `[]` and `["x"]`, and so do the
    wire types that embed it (`ObservationInput`, `RetrieveOptions`,
    `ListObservationsRequest`).
  - `kindling-server` integration test: for each v1 route whose body carries
    `scopeIds` (`POST /v1/capsules`, `/v1/observations`, `/v1/observations/list`,
    `/v1/retrieve`, `/v1/pins`, `/v1/context/session-start`,
    `/v1/context/pre-compact`), the same body succeeds with an object
    `scopeIds` and returns a 4xx with `[]` and with `["s1"]`. The success control
    proves the rejection is caused by the array, not by another invalid field.
  - `docs/retrieval-contract.md` Scoping states that `scopeIds` is a keyed JSON
    object and that arrays are rejected.
- **Non-scope:** changing `ScopeIds`, the DTOs, or the status code axum returns
  for a JSON data error; the TS thin client.
- **Validation:**
  - `cargo test -p kindling-types`
  - `cargo test -p kindling-server --test api`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo fmt --all -- --check`

## Actions

### 1. Type-level regression

- **Checkpoint:** array forms fail to deserialize for `ScopeIds` and its
  embedding wire types.
- **Validate:** `cargo test -p kindling-types`

### 2. API-level regression

- **Checkpoint:** every `scopeIds`-carrying route: object → 2xx, array → 4xx.
- **Validate:** `cargo test -p kindling-server --test api`

### 3. Contract doc

- **Checkpoint:** Scoping section names the keyed-object shape.

### 4. Verify and publish

- **Checkpoint:** fresh verifier passes; draft PR opened with `Closes #216`.

## Landing

Merge authority: none current (standing grant expired 2026-11-12). Terminal
outcome is `review-ready`; merge waits for a reviewer and an explicit operator go.
