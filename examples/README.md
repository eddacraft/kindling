# kindling examples

Runnable examples for integrating with kindling without anvil. Each example
focuses on a single integration path.

## Examples

| Directory                             | Description                                                | Docs                                             |
| ------------------------------------- | ---------------------------------------------------------- | ------------------------------------------------ |
| [adapter-minimal](./adapter-minimal/) | Smallest TypeScript adapter: open, append, retrieve, close | [Adapter cookbook](../docs/adapters/cookbook.md) |

## Related packages

These production adapters in `packages/` are larger than the examples but follow
the same client API:

| Package                                                                              | Tool       |
| ------------------------------------------------------------------------------------ | ---------- |
| [`@eddacraft/kindling-adapter-opencode`](../packages/kindling-adapter-opencode/)     | OpenCode   |
| [`@eddacraft/kindling-adapter-pocketflow`](../packages/kindling-adapter-pocketflow/) | PocketFlow |

## Running examples in the monorepo

```bash
pnpm install
pnpm --filter @eddacraft/kindling run build
pnpm --filter kindling-adapter-minimal dev
```

See each example's README for prerequisites and expected output.
