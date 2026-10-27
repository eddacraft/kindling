# Integrations

kindling is a local memory engine. Integrations capture context into it (write)
or retrieve context from it (read). Most paths go through the `kindling` daemon
(`kindling serve`) or the in-process Rust service for embedded use.

This matrix is for developers who are **not** using [anvil](https://eddacraft.ai).
anvil builds on kindling with governed plans and policy; kindling itself is
tool-agnostic.

## Matrix

| Integration                                                   | Status           | Package / entry point                                                                                                          | Notes                                                                                                                                                                                                                                                                                  |
| ------------------------------------------------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | **Supported**    | Built into the `kindling` binary; [plugin](../plugins/kindling-claude-code/)                                                   | Hooks capture tool calls, edits, commands and errors automatically. Install the plugin from inside Claude Code: `/plugin marketplace add eddacraft/kindling`, then `/plugin install kindling@kindling-plugins`. `kindling init --claude-code` checks the setup and prints these steps. |
| [OpenCode](https://github.com/opencode-ai/opencode)           | **Supported**    | [source](../packages/kindling-adapter-opencode/) (`@eddacraft/kindling-adapter-opencode`, not on npm)                          | Session lifecycle adapter over the thin Node client. Use from source; a bundled plugin installed by the binary is planned.                                                                                                                                                             |
| [PocketFlow](https://github.com/The-Pocket/PocketFlow)        | **Supported**    | [source](../packages/kindling-adapter-pocketflow/) (`@eddacraft/kindling-adapter-pocketflow`, not on npm)                      | Workflow node adapter; one `pocketflow_node` capsule per node. Use from source.                                                                                                                                                                                                        |
| VS Code / Cursor / Windsurf                                   | **Supported**    | VSIX on each [GitHub Release](https://github.com/eddacraft/kindling/releases) · [source](../packages/kindling-adapter-vscode/) | Extension captures file saves; commands for search, log, and status. Install from VSIX: [Cursor and Windsurf quickstart](./quickstart/cursor-windsurf.md).                                                                                                                             |
| Plain CLI                                                     | **Supported**    | [`eddacraft-kindling`](https://crates.io/crates/eddacraft-kindling) (binary: `kindling`)                                       | Read and write without any SDK. `kindling log`, `kindling search`, `kindling browse`, and more.                                                                                                                                                                                        |
| Rust SDK                                                      | **Supported**    | [`kindling-client`](https://crates.io/crates/kindling-client)                                                                  | Daemon-backed client; recommended for Rust integrations. Auto-spawns `kindling serve`.                                                                                                                                                                                                 |
| Rust SDK (embedded)                                           | **Supported**    | [`kindling-service`](https://crates.io/crates/kindling-service)                                                                | In-process API when you want a single binary with no IPC. Same operations as the client.                                                                                                                                                                                               |
| Node client                                                   | **Supported**    | [source](../packages/kindling/) (`@eddacraft/kindling`, not on npm)                                                            | Thin TypeScript client over the daemon. Use from source or bundle it; needs the `kindling` binary.                                                                                                                                                                                     |
| Any editor or agent (manual)                                  | **Via CLI only** | `kindling` binary                                                                                                              | Log observations and search from the terminal. No automatic capture until you wire an adapter.                                                                                                                                                                                         |
| Custom adapter                                                | **Supported**    | [`packages/kindling`](../packages/kindling/) (from source) or [`kindling-client`](https://crates.io/crates/kindling-client)    | Build your own capture layer. Start with the [cookbook](./adapters/cookbook.md) and [minimal example](../examples/adapter-minimal/).                                                                                                                                                   |

None of the Node packages are published to npm: the `@eddacraft/*` versions on
npm are the retired 0.1.x TypeScript engine and are deprecated. Install the
`kindling` binary (install script, `cargo install eddacraft-kindling` or
Homebrew) and use the Node packages from a clone of this repository.

### How this matrix is tested

`crates/kindling/tests/integration_matrix.rs` fails when a row here or in the
README's integrations table is marked Supported without a named test that
drives it. The named tests run in CI, through the daemon or in-process as the
integration does, each against temporary state rather than your real profile.
The check finds each test by name; it does not prove that the test ran, so CI
running those suites is part of the guarantee. It also checks that the matrix
links resolve and that the Claude Code plugin manifests agree with what
`kindling init --claude-code` prints.

No test performs a real install: not `install.sh`, `cargo install`,
Homebrew, `/plugin install` in Claude Code, or the VSIX in a real editor. The
release workflow checks that the VSIX version matches the tag. The tests drive
the same binary, hooks and session manager those installs provide.

## Status definitions

| Status           | Meaning                                                                      |
| ---------------- | ---------------------------------------------------------------------------- |
| **Supported**    | Released artefact, built-in path or in-repo source package, maintained here. |
| **Planned**      | On the roadmap; use CLI or client APIs in the meantime.                      |
| **Via CLI only** | No dedicated adapter yet; the `kindling` binary covers read/write manually.  |

## Choosing an integration path

```text
Need automatic capture from a specific tool?
  ├─ Claude Code     → /plugin install kindling@kindling-plugins (hooks call the binary)
  ├─ OpenCode        → packages/kindling-adapter-opencode (from source)
  ├─ PocketFlow      → packages/kindling-adapter-pocketflow (from source)
  ├─ VS Code family  → kindling VSIX from the GitHub Release
  └─ Your own agent  → kindling-client (Rust) or packages/kindling (Node, from source)

Need manual capture from the shell?
  └─ kindling log / kindling capsule open|close

Need programmatic read/write from application code?
  ├─ Rust + shared daemon  → kindling-client
  ├─ Rust + embedded       → kindling-service
  └─ Node/TypeScript       → packages/kindling (from source)
```

## Related documentation

- [Quickstart without Claude Code](./quickstart/without-claude-code.md)
- [Quickstart: Cursor and Windsurf](./quickstart/cursor-windsurf.md)
- [Build your adapter in 10 minutes](./adapters/cookbook.md)
- [Architecture](./architecture.md)
- [Retrieval contract](./retrieval-contract.md)
