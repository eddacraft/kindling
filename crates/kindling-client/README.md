# kindling-client

**Daemon-backed Rust SDK for kindling** — local-first memory for AI-assisted development.

Use `kindling-client` when your Rust application, agent, editor extension or workflow tool needs to read and write kindling memory safely while other tools may be running at the same time.

This is the recommended Rust SDK for most integrations. It is a thin async client that speaks HTTP/1 over a Unix domain socket (TCP fallback on Windows), auto-spawns the daemon on first use, and re-exports the domain types so you depend on this crate alone.

## Install

```toml
[dependencies]
kindling-client = "0.1"
```

## Example

```rust
use kindling_client::{Client, CapsuleType, ScopeIds};

#[tokio::main]
async fn main() -> Result<(), kindling_client::ClientError> {
    let client = Client::new()?;

    let health = client.health().await?;
    println!("kindling daemon schema v{}", health.schema_version);

    let capsule = client
        .open_capsule(
            CapsuleType::Session,
            "investigate flaky test",
            ScopeIds::default(),
            None,
        )
        .await?;

    println!("opened capsule {}", capsule.id);

    Ok(())
}
```

## When to use this crate

Use `kindling-client` when you want:

- Daemon-backed access to kindling.
- Safe concurrent integration from multiple tools.
- A lightweight SDK without pulling in SQLite directly.
- Domain types re-exported from one crate.

For embedded, single-process usage, use [`kindling-service`](https://crates.io/crates/kindling-service) instead.

## How it fits into kindling

`kindling-client` talks to the daemon you start with `kindling serve` (from the [`kindling`](https://crates.io/crates/kindling) binary). The daemon owns the local SQLite store and keeps access safe when several tools touch the same project memory at once.

## Documentation

Full docs: **[docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)**

Relevant guides:

- [Core concepts](https://docs.eddacraft.ai/kindling/concepts/capsules)
- [Writing adapters](https://docs.eddacraft.ai/kindling/adapters/custom)
- [Configuration](https://docs.eddacraft.ai/kindling/reference/config)

## Licence

Apache-2.0
