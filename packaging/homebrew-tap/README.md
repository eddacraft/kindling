# eddacraft/homebrew-tap

Homebrew tap for EddaCraft tools: [github.com/eddacraft/homebrew-tap](https://github.com/eddacraft/homebrew-tap)

The tap is live. Today it ships **anvil** (`Formula/anvil.rb`). The **kindling**
formula template lives in this repo at [`packaging/homebrew/kindling.rb`](../homebrew/kindling.rb)
and should be published to the tap as `Formula/kindling.rb` so users can run:

```bash
brew install eddacraft/tap/kindling
```

## Add or update the kindling formula

### 1. Generate checksums from a GitHub Release

The release must include macOS tarballs and `.sha256` sidecars (produced by
[`release.yml`](../../.github/workflows/release.yml)):

```bash
./scripts/generate-homebrew-formula.sh vX.Y.Z
```

This updates `packaging/homebrew/kindling.rb` with the bare version and both
darwin SHA256 values.

To copy straight into a local tap clone:

```bash
KINDLING_TAP_DIR=../homebrew-tap ./scripts/generate-homebrew-formula.sh vX.Y.Z --sync-tap
```

`KINDLING_TAP_DIR` defaults to a sibling directory named `homebrew-tap` if it
exists.

### 2. Push to the tap repository

```bash
cd ../homebrew-tap   # or your clone of github.com/eddacraft/homebrew-tap
git add Formula/kindling.rb
git commit -m "kindling vX.Y.Z"
git push origin main
```

### 3. Smoke test

```bash
brew update
brew install eddacraft/tap/kindling
kindling --version
kindling demo
```

## What the formula installs

Prebuilt macOS binaries from [kindling GitHub Releases](https://github.com/eddacraft/kindling/releases):

| Architecture          | Archive                                          |
| --------------------- | ------------------------------------------------ |
| Apple Silicon (arm64) | `kindling-<version>-aarch64-apple-darwin.tar.gz` |
| Intel (x86_64)        | `kindling-<version>-x86_64-apple-darwin.tar.gz`  |

Linux users should use the [install script](../../install.sh) or download release
tarballs directly.

## Automation (optional)

A release workflow can call `generate-homebrew-formula.sh --sync-tap` and push to
`eddacraft/homebrew-tap` automatically. Until that is wired up, update the tap
manually on each release.

## User-facing install

```bash
brew install eddacraft/tap/kindling
kindling init
kindling demo
```
