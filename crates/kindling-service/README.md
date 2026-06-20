# kindling-service

**In-process orchestration for kindling memory** — local-first memory for AI-assisted development.

`kindling-service` is the embedded Rust service layer for kindling. It gives you direct access to capsule lifecycle, observation capture, retrieval, pins, redaction and import/export without going through the daemon.

For most integrations, prefer [`kindling-client`](https://crates.io/crates/kindling-client). Use this crate when you deliberately want single-process, zero-IPC access.

## Install

```toml
[dependencies]
kindling-service = "0.1"
```

## When to use this crate

Use `kindling-service` when:

- You are embedding kindling inside a Rust process.
- You control access to the underlying store.
- You want zero IPC overhead.
- You are building a headless workflow, test harness or controlled runtime.

Avoid this crate for multi-tool concurrent access unless you know exactly how storage access is coordinated. The daemon-backed [`kindling-client`](https://crates.io/crates/kindling-client) is safer for general integrations.

## What it provides

- Open and close capsules.
- Append observations.
- Retrieve relevant memory.
- Create and remove pins.
- Mask secrets at the service boundary.
- Export and import kindling bundles.

## Documentation

Full docs: **[docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)**

Useful starting points:

- [Core concepts](https://docs.eddacraft.ai/kindling/concepts/capsules)
- [Writing adapters](https://docs.eddacraft.ai/kindling/adapters/custom)

## Licence

Apache-2.0
