# GH-216: reject array-shaped `scopeIds` at the type and API boundary

**Authority:** GitHub issue #216 (no APS work item). The issue asks to "reject
non-object forms if they are not supported"; the documented contract is a keyed
object, so this is a bug fix, not a Discuss-first API change.
**Date:** 2026-11-19
**Risk:** standard (narrows accepted input to the documented contract)

## Context

The retired TypeScript validator accepted arrays as `scopeIds`. The triage
comment on #216 concluded the Rust boundary already rejected them. That was
wrong: serde's derived `Deserialize` for a named struct implements `visit_seq`
as well as `visit_map`, binding sequence elements to fields by position. The
first regression test failed, and against the daemon `POST /v1/capsules` with
`"scopeIds": []` returned 201; `["x"]` would be read as `{ "sessionId": "x" }`.

## Work item

- **Intent:** a JSON array in `scopeIds` is rejected at both the type layer and
  every v1 endpoint that accepts `scopeIds`, and the contract says so.
- **Expected Outcome:**
  - `ScopeIds` has a hand-written `Deserialize` that accepts only a map
    (delegating to a private derived field struct), so a sequence fails with
    `invalid type: sequence`.
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
  - TS-era rows stay readable: the store and provider decode an array-shaped
    stored `scope_ids` blob as an empty scope (TS wrote NULL denormalized
    columns for such rows), while other malformed blobs still error. Covered
    by store and provider tests that write a raw legacy row.
  - CHANGELOG `### Fixed` and ENGINEERING-HISTORY entries.
- **Non-scope:** the field set or serialization of `ScopeIds`; the DTOs; the
  status code axum returns for a JSON data error; the TS thin client; the same
  positional-sequence acceptance in other derived request structs (whole
  bodies), which is noted in the PR as separate follow-up work.
- **Validation:**
  - `cargo test -p kindling-types`
  - `cargo test -p kindling-types --features ts-rs` (bindings drift)
  - `node packages/kindling/scripts/sync-types.mjs` then no diff in
    `packages/kindling/src/generated/` (thin-client copy)
  - `cargo test -p kindling-server --test api`
  - `cargo test -p kindling-store -p kindling-provider`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo fmt --all -- --check`
  - `python3 scripts/check-release-docs.py`

## Actions

### 1. Type-level regression and fix

- **Checkpoint:** array forms fail to deserialize for `ScopeIds` and its
  embedding wire types; the test fails before the fix and passes after.
- **Validate:** `cargo test -p kindling-types`

### 2. API-level regression

- **Checkpoint:** every `scopeIds`-carrying route: object → 2xx, array → 4xx.
- **Validate:** `cargo test -p kindling-server --test api`

### 3. Contract doc and release records

- **Checkpoint:** Scoping section names the keyed-object shape; CHANGELOG and
  ENGINEERING-HISTORY entries added.
- **Validate:** `python3 scripts/check-release-docs.py`

### 4. Verify and publish

- **Checkpoint:** fresh verifier passes; draft PR opened with `Closes #216`.

## Landing

Merge authority: none current (standing grant expired 2026-11-12). Terminal
outcome is `review-ready`; merge waits for a reviewer and an explicit operator go.
