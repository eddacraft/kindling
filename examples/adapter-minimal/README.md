# Minimal kindling adapter

A smallest-possible TypeScript adapter that exercises the full capture lifecycle
through [`@eddacraft/kindling`](../../packages/kindling/).

## What it does

1. Opens a `session` capsule
2. Appends two observations (a message and an error)
3. Retrieves context for the query `JWT`
4. Closes the capsule with a summary

See [docs/adapters/cookbook.md](../../docs/adapters/cookbook.md) for the
step-by-step guide.

## Prerequisites

- Node.js >= 20 and pnpm
- The `kindling` binary on `PATH`:

  ```bash
  curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
  ```

  Or build from source: `cargo build -p eddacraft-kindling --bin kindling`

## Run from the monorepo

From the repository root:

```bash
pnpm install
pnpm --filter @eddacraft/kindling run build
pnpm --filter kindling-adapter-minimal dev
```

Expected output includes a health check, capsule id, retrieve candidates, and a
closed capsule status.

## Run standalone (after publish)

If you have copied this example outside the monorepo:

```bash
npm install @eddacraft/kindling
npx tsc index.ts --module nodenext --moduleResolution nodenext --target es2022
node index.js
```

## Troubleshooting

| Problem                  | Fix                                                                    |
| ------------------------ | ---------------------------------------------------------------------- |
| `DaemonUnavailableError` | Ensure `kindling` is on `PATH`, or set `$KINDLING_BIN` to your binary. |
| Empty retrieve results   | Observations need a moment to index; retry, or check `scopeIds` match. |
| Schema mismatch          | Upgrade the CLI and client to the same version.                        |
