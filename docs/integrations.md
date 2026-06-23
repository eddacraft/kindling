# Integrations

kindling is a local memory engine. Integrations capture context into it (write)
or retrieve context from it (read). Most paths go through the `kindling` daemon
(`kindling serve`) or the in-process Rust service for embedded use.

This matrix is for developers who are **not** using [anvil](https://eddacraft.ai).
anvil builds on kindling with governed plans and policy; kindling itself is
tool-agnostic.

## Matrix

| Integration                                                   | Status           | Package / entry point                                                                                                                                                    | Notes                                                                                                                                                |
| ------------------------------------------------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | **Supported**    | Built into the `kindling` binary; [plugin](../../plugins/kindling-claude-code/)                                                                                          | Hooks capture tool calls, edits, commands and errors automatically. Run `kindling init --claude-code` or use the [install script](../../install.sh). |
| [OpenCode](https://github.com/opencode-ai/opencode)           | **Supported**    | [`@eddacraft/kindling-adapter-opencode`](https://www.npmjs.com/package/@eddacraft/kindling-adapter-opencode) · [source](../../packages/kindling-adapter-opencode/)       | Session lifecycle adapter over the thin Node client.                                                                                                 |
| [PocketFlow](https://github.com/The-Pocket/PocketFlow)        | **Supported**    | [`@eddacraft/kindling-adapter-pocketflow`](https://www.npmjs.com/package/@eddacraft/kindling-adapter-pocketflow) · [source](../../packages/kindling-adapter-pocketflow/) | Workflow node adapter; one `pocketflow_node` capsule per node.                                                                                       |
| VS Code / Cursor / Windsurf                                   | **Supported**    | [`@eddacraft/kindling-adapter-vscode`](https://www.npmjs.com/package/@eddacraft/kindling-adapter-vscode) · [source](../../packages/kindling-adapter-vscode/)             | Extension captures file saves; commands for search, log, and status.                                                                                 |
| Plain CLI                                                     | **Supported**    | [`eddacraft-kindling`](https://crates.io/crates/eddacraft-kindling) (binary: `kindling`)                                                                                 | Read and write without any SDK. `kindling log`, `kindling search`, `kindling browse`, and more.                                                      |
| Rust SDK                                                      | **Supported**    | [`kindling-client`](https://crates.io/crates/kindling-client)                                                                                                            | Daemon-backed client; recommended for Rust integrations. Auto-spawns `kindling serve`.                                                               |
| Rust SDK (embedded)                                           | **Supported**    | [`kindling-service`](https://crates.io/crates/kindling-service)                                                                                                          | In-process API when you want a single binary with no IPC. Same operations as the client.                                                             |
| Node client                                                   | **Supported**    | [`@eddacraft/kindling`](https://www.npmjs.com/package/@eddacraft/kindling) · [source](../../packages/kindling/)                                                          | Thin TypeScript client over the daemon. Ships a per-platform optional binary dependency.                                                             |
| Any editor or agent (manual)                                  | **Via CLI only** | `kindling` binary                                                                                                                                                        | Log observations and search from the terminal. No automatic capture until you wire an adapter.                                                       |
| Custom adapter                                                | **Supported**    | [`@eddacraft/kindling`](https://www.npmjs.com/package/@eddacraft/kindling) or [`kindling-client`](https://crates.io/crates/kindling-client)                              | Build your own capture layer. Start with the [cookbook](./adapters/cookbook.md) and [minimal example](../../examples/adapter-minimal/).              |

## Status definitions

| Status           | Meaning                                                                     |
| ---------------- | --------------------------------------------------------------------------- |
| **Supported**    | Published package or built-in path, maintained in this repo.                |
| **Planned**      | On the roadmap; use CLI or client APIs in the meantime.                     |
| **Via CLI only** | No dedicated adapter yet; the `kindling` binary covers read/write manually. |

## Choosing an integration path

```text
Need automatic capture from a specific tool?
  ├─ Claude Code     → kindling init --claude-code (built-in hooks)
  ├─ OpenCode        → @eddacraft/kindling-adapter-opencode
  ├─ PocketFlow      → @eddacraft/kindling-adapter-pocketflow
  ├─ VS Code family  → @eddacraft/kindling-adapter-vscode
  └─ Your own agent  → kindling-client (Rust) or @eddacraft/kindling (Node)

Need manual capture from the shell?
  └─ kindling log / kindling capsule open|close

Need programmatic read/write from application code?
  ├─ Rust + shared daemon  → kindling-client
  ├─ Rust + embedded       → kindling-service
  └─ Node/TypeScript       → @eddacraft/kindling
```

## Related documentation

- [Quickstart without Claude Code](./quickstart/without-claude-code.md)
- [Build your adapter in 10 minutes](./adapters/cookbook.md)
- [Architecture](./architecture.md)
- [Retrieval contract](./retrieval-contract.md)
