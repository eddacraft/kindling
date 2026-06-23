# kindling-runtime

The **anvil-first integration facade** for [kindling](https://github.com/eddacraft/kindling):
one Cargo dependency that bundles daemon startup, client wiring, and durable
emit, so a Rust downstream needs **one binary** and gets **daemon semantics**
without a separate `kindling` CLI install.

It composes the existing crates — it does not fork their wire shapes:

- [`kindling-client`](https://docs.rs/kindling-client) for the HTTP-over-UDS surface
- the opt-in `spool` layer for durable emit (`SpooledClient`)
- [`kindling-server`](https://docs.rs/kindling-server) started in-process on a tokio task
- [`kindling-types`](https://docs.rs/kindling-types) re-exported as `kindling_runtime::types`

kindling is **mechanism, not policy**: the runtime owns process lifecycle and
client wiring; it does not encode downstream governance.

## Quickstart

```toml
[dependencies]
kindling-runtime = "0.2"   # default features: client + spool + embedded-daemon
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust,no_run
use kindling_runtime::{Runtime, RuntimeConfig};
use kindling_runtime::types::{ObservationInput, ObservationKind, ScopeIds};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Embedded daemon (default), durable spooled emit, default ~/.kindling home.
    // Attaches to a daemon already listening on the socket; otherwise starts one
    // in-process — no `kindling` on PATH required.
    let runtime = Runtime::start(RuntimeConfig::embedded("/path/to/my/project")).await?;

    let input = ObservationInput {
        id: None,
        kind: ObservationKind::Message,
        content: "gate evaluated: pass".to_string(),
        provenance: None,
        ts: None,
        scope_ids: ScopeIds::default(),
        redacted: None,
    };

    // Durable append: reaches the daemon, or buffers to the spool on outage and
    // drains on the next successful append / flush.
    runtime
        .spooled_client()
        .append_observation(input, None, None)
        .await?;

    runtime.shutdown().await?;
    Ok(())
}
```

## Attach-or-start

`Runtime::start` never pre-emptively starts a daemon. It builds a client for the
configured socket; the client only spawns when the socket does not answer. So if
a daemon (the CLI, a Claude Code hook, or another runtime) is already listening
on the same socket, the runtime **attaches** to it. Use
`runtime.spawned_embedded_daemon()` to tell whether this process started the
daemon (`true`) or attached to an existing one (`false`).

`SpawnStrategy` selects what happens when a spawn is actually required:

| Strategy            | Behaviour                                                          | Feature           |
| ------------------- | ----------------------------------------------------------------- | ----------------- |
| `Embedded` (default) | start `kindling-server` in-process on a tokio task               | `embedded-daemon` |
| `External`          | exec the real `kindling` binary on `PATH`                         | `external-spawn`  |
| `AttachOnly`        | never spawn — attach to a running daemon or error                 | (always)          |

## Feature flags

| Feature           | Default | Pulls                   | Purpose                         |
| ----------------- | ------- | ----------------------- | ------------------------------- |
| `client`          | yes     | `kindling-client`       | HTTP client surface             |
| `spool`           | yes     | `kindling-client/spool` | `SpooledClient` as primary API  |
| `embedded-daemon` | yes     | `kindling-server`       | in-process `serve()`            |
| `external-spawn`  | no      | —                       | fall back to `kindling` on PATH |

To trim binary size (drop the embedded server + store), disable default features
and opt back into only what you need:

```toml
kindling-runtime = { version = "0.2", default-features = false, features = ["client", "spool", "external-spawn"] }
```

## License

Apache-2.0
