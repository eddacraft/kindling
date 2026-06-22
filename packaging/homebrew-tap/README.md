# eddacraft/homebrew-tap

Homebrew tap for installing the kindling CLI on macOS.

The formula template lives in this repo at
[`packaging/homebrew/kindling.rb`](../homebrew/kindling.rb). The tap repository
is separate so users can run `brew install eddacraft/tap/kindling` without
cloning the main kindling source tree.

## Create the tap repository

1. Create a new GitHub repository named `homebrew-tap` under the `eddacraft`
   organisation:

   ```
   https://github.com/eddacraft/homebrew-tap
   ```

2. Initialise the standard Homebrew layout:

   ```bash
   git clone git@github.com:eddacraft/homebrew-tap.git
   cd homebrew-tap
   mkdir -p Formula
   ```

3. Copy the formula from the kindling repo:

   ```bash
   cp /path/to/kindling/packaging/homebrew/kindling.rb Formula/kindling.rb
   git add Formula/kindling.rb
   git commit -m "Add kindling formula"
   git push origin main
   ```

4. Verify installation:

   ```bash
   brew install eddacraft/tap/kindling
   kindling --version
   ```

## Per-release maintenance

After each GitHub Release that publishes macOS tarballs:

1. From the kindling repo, update the formula with release checksums:

   ```bash
   ./scripts/generate-homebrew-formula.sh vX.Y.Z
   ```

   This fetches the `.sha256` sidecars from the GitHub Release and updates
   `version` plus both `REPLACE_WITH_SHA256_*` placeholders in
   `packaging/homebrew/kindling.rb`.

2. Copy the updated formula to the tap:

   ```bash
   cp packaging/homebrew/kindling.rb /path/to/homebrew-tap/Formula/kindling.rb
   cd /path/to/homebrew-tap
   git add Formula/kindling.rb
   git commit -m "kindling vX.Y.Z"
   git push origin main
   ```

3. Smoke test:

   ```bash
   brew update
   brew upgrade kindling
   kindling --version
   ```

## What the formula installs

The formula downloads prebuilt macOS binaries from GitHub Releases:

| Architecture          | Archive                                          |
| --------------------- | ------------------------------------------------ |
| Apple Silicon (arm64) | `kindling-<version>-aarch64-apple-darwin.tar.gz` |
| Intel (x86_64)        | `kindling-<version>-x86_64-apple-darwin.tar.gz`  |

Linux users should use the [install script](../../install.sh) or download release
tarballs directly. See [packaging/README.md](../README.md) for the full
distribution matrix.

## Automation (optional)

A release workflow can call `scripts/generate-homebrew-formula.sh`, commit to
`eddacraft/homebrew-tap`, and push automatically. Until that is wired up,
maintainers update the tap manually using the steps above.

## User-facing install instructions

Add to docs and README once the tap is live:

```bash
brew install eddacraft/tap/kindling
kindling init
kindling demo
```
