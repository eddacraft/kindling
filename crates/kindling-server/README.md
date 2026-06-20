# kindling-server

**Daemon runtime for kindling** — local-first memory for AI-assisted development.

`kindling-server` provides the daemon used by the kindling CLI and Rust client. It exposes kindling memory over a local HTTP/1 API, using Unix domain sockets where available and a TCP fallback on Windows.

Most users should not depend on this crate directly. Run the daemon with:

```bash
kindling serve
```

Use this crate when you are extending, embedding or testing the daemon layer itself.

## When to use this crate

Use `kindling-server` if you are:

- Building a custom runtime around the kindling daemon.
- Testing daemon behaviour directly.
- Extending local API endpoints.
- Working on per-project database routing or process lifecycle behaviour.

For normal Rust integrations, use [`kindling-client`](https://crates.io/crates/kindling-client).

## Runtime role

The daemon exists to make kindling safe and predictable when multiple tools are interacting with project memory at the same time. It handles:

- Local API access.
- Project-aware routing.
- Store/service orchestration.
- Daemon lifecycle.
- Cross-tool coordination.

## Documentation

Full docs: **[docs.eddacraft.ai/kindling](https://docs.eddacraft.ai/kindling/overview)**

Relevant docs:

- [Configuration](https://docs.eddacraft.ai/kindling/reference/config)
- [Core concepts](https://docs.eddacraft.ai/kindling/concepts/capsules)

## Licence

Apache-2.0
