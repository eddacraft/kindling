# eddacraft/homebrew-tap

Shared Homebrew tap for EddaCraft CLI tools:
[github.com/eddacraft/homebrew-tap](https://github.com/eddacraft/homebrew-tap)

**anvil** already ships from this tap (`brew install eddacraft/tap/anvil`).
**kindling** uses the same tap: add or update `Formula/kindling.rb` from the
template in [`packaging/homebrew/kindling.rb`](../homebrew/kindling.rb).

```bash
brew install eddacraft/tap/anvil      # already available
brew install eddacraft/tap/kindling   # after Formula/kindling.rb is published
```

## Publish or update kindling

### 1. Generate checksums from a GitHub Release

The release must include macOS tarballs and `.sha256` sidecars (from
[`release.yml`](../../.github/workflows/release.yml)):

```bash
./scripts/generate-homebrew-formula.sh vX.Y.Z --sync-tap
```

This updates `packaging/homebrew/kindling.rb` and copies it to
`$KINDLING_TAP_DIR/Formula/kindling.rb` (defaults to a sibling `../homebrew-tap`
clone).

### 2. Push to the tap (same repo anvil uses)

```bash
cd ../homebrew-tap
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

Prebuilt macOS binaries from [kindling releases](https://github.com/eddacraft/kindling/releases):

| Architecture          | Archive                                          |
| --------------------- | ------------------------------------------------ |
| Apple Silicon (arm64) | `kindling-<version>-aarch64-apple-darwin.tar.gz` |
| Intel (x86_64)        | `kindling-<version>-x86_64-apple-darwin.tar.gz`  |

Linux users should use the [install script](../../install.sh).

## User-facing install

```bash
brew install eddacraft/tap/kindling
kindling init
kindling demo
```
