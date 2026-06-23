# kindling-runtime — Anvil-First Integration Facade

| Field      | Value                                                    |
| ---------- | -------------------------------------------------------- |
| Status     | Proposed                                                 |
| Owner      | @aneki                                                   |
| Created    | 2026-06-24                                               |
| APS Module | `plans/modules/06-downstream-integration-surface.aps.md` |
| Work Item  | KINTEG-008                                               |

## Context

kindling's primary downstream consumer is **anvil** (KDS module). The current
integration surface expects consumers to:

1. Depend on `kindling-client` (+ opt-in `spool` feature).
2. Install the `kindling` CLI binary separately so auto-spawn can `exec kindling serve --daemonize`.
3. Wire `Spawner::custom` themselves if they want a single shipped binary.

The client integration tests already demonstrate in-process daemon startup
(`kindling_server::serve` on a tokio task + `Spawner::custom`). That pattern is
correct but unpublished as a product — every downstream Rust consumer reinvents
it.

## Decision

Add a **`kindling-runtime`** facade crate: the blessed integration path for
Rust downstreams (chiefly anvil) that need **one binary** and **daemon
semantics** without a separate `kindling` install.

kindling stays **mechanism, not policy**. The runtime owns process lifecycle and
client wiring; it does not encode anvil governance.

## Goals

- **One Cargo dependency** for anvil KDS: `kindling-runtime` with feature flags.
- **Bundled daemon** by default: in-process `kindling-server` on the standard UDS
  path (or an isolated test socket), no `kindling` on `PATH` required.
- **Spool on by default** at the runtime layer (durable emit is KDS-critical).
- **Attach-or-start**: if a daemon is already listening on the configured socket,
  connect instead of starting a second one.
- **Preserve the wire contract**: HTTP/UDS, shared DB with Claude Code hooks and
  CLI when using the default socket layout.

## Non-Goals (v1)

- Replacing `kindling-client` or `kindling-server` — the runtime composes them.
- Embedded `KindlingService` mode (zero-IPC) — defer to a follow-up feature flag
  (`embedded-service`) if anvil proves it needs a hot path without shared tools.
- anvil-specific observation kinds or policy types.

## Crate Shape

```
kindling-runtime/
  Cargo.toml
  src/lib.rs          # Runtime, RuntimeConfig, Mode
  src/spawn.rs        # attach-or-start, Spawner wiring
  README.md
  tests/runtime.rs    # cold start, attach, spooled client round-trip
```

### Feature flags

| Feature           | Default | Pulls                   | Purpose                         |
| ----------------- | ------- | ----------------------- | ------------------------------- |
| `client`          | yes     | `kindling-client`       | HTTP client surface             |
| `spool`           | yes     | `kindling-client/spool` | `SpooledClient` as primary API  |
| `embedded-daemon` | yes     | `kindling-server`       | In-process `serve()`            |
| `external-spawn`  | no      | —                       | Fall back to `kindling` on PATH |

Default feature set: `["client", "spool", "embedded-daemon"]`.

### Public API (sketch)

```rust
pub struct RuntimeConfig {
    pub kindling_home: PathBuf,       // default ~/.kindling
    pub project_root: String,         // X-Kindling-Project routing
    pub spool_path: Option<PathBuf>,  // default <home>/spool.ndjson
    pub spawn: SpawnStrategy,         // Embedded | External | AttachOnly
}

pub struct Runtime { /* owns server handle + client config */ }

impl Runtime {
    pub async fn start(config: RuntimeConfig) -> Result<Self, RuntimeError>;
    pub fn client(&self) -> &Client;
    pub fn spooled_client(&self) -> &SpooledClient;  // when spool enabled
    pub async fn shutdown(self) -> Result<(), RuntimeError>;
}
```

`SpawnStrategy::Embedded` uses the pattern from
`crates/kindling-client/tests/client.rs::cold_spawn_starts_daemon`.
`AttachOnly` never spawns — for tests and hosts that manage the daemon
externally.

## Dependency Flow

```
anvil
  └── kindling-runtime  (facade)
        ├── kindling-client (+ spool)
        ├── kindling-server (embedded-daemon)
        └── kindling-types (re-exported)
```

`scripts/publish.sh` gains `kindling-runtime` after `kindling-server` and
before `kindling-client` is unchanged (runtime depends on both).

## Sequencing

| Order | Item        | Why                                               |
| ----- | ----------- | ------------------------------------------------- |
| 1     | KINTEG-001  | Publish 0.2.0 client + spool                      |
| 2     | PORT-011    | anvil proof with raw client (documents pain)      |
| 3     | KINTEG-002  | Dedup before runtime promotes spool as default    |
| 4     | KINTEG-008  | Runtime facade — anvil cuts deps to one crate     |
| 5     | KINTEG-003… | Query/handshake surface through `Runtime` methods |

## Validation

- Integration test: `Runtime::start` with `Embedded` on a temp home → health OK,
  append via `spooled_client`, observation in store.
- Integration test: pre-started daemon on same socket → `AttachOnly` connects,
  spawner not invoked.
- `cargo package --list -p kindling-runtime` includes README + manifests.
- Scratch consumer: `cargo add kindling-runtime` resolves after publish.

## Risks

| Risk                                         | Mitigation                                                      |
| -------------------------------------------- | --------------------------------------------------------------- |
| Duplicate daemon if anvil and CLI both spawn | Attach-or-start on socket existence + PID lock reuse            |
| Binary size (anvil + server + store)         | Acceptable for primary consumer; document feature flags to trim |
| API drift from raw client                    | Runtime methods delegate; don't fork wire shapes                |
