# eddacraft/homebrew-tap

Shared Homebrew tap for EddaCraft CLI tools:
[github.com/eddacraft/homebrew-tap](https://github.com/eddacraft/homebrew-tap)

**anvil** already ships from this tap (`brew install eddacraft/tap/anvil`).
**kindling** uses the same tap via `Formula/kindling.rb`.

```bash
brew install eddacraft/tap/anvil
brew install eddacraft/tap/kindling
```

## Automated updates (CI)

On every published kindling release, [`.github/workflows/release.yml`](../../.github/workflows/release.yml)
runs a `homebrew-tap-pr` job **after** release tarballs and `.sha256` sidecars are
attached to the GitHub Release (macOS and Linux glibc). It:

1. Runs `./scripts/generate-homebrew-formula.sh <tag>`
2. Copies `packaging/homebrew/kindling.rb` into the tap checkout
3. Opens a PR in `eddacraft/homebrew-tap` via `peter-evans/create-pull-request`

### Required secret

Add to the **kindling** repository (Settings → Secrets → Actions):

| Secret               | Purpose                                                                     |
| -------------------- | --------------------------------------------------------------------------- |
| `HOMEBREW_TAP_TOKEN` | PAT or fine-grained token with `contents:write` on `eddacraft/homebrew-tap` |

Without this secret the job fails when it tries to check out the tap repo. Use an
org machine-user PAT or a fine-grained token scoped to `homebrew-tap` only.

Merge the automated PR to make `brew install eddacraft/tap/kindling` serve the
new version.

## Manual fallback

If CI did not run or you need to republish:

```bash
./scripts/generate-homebrew-formula.sh vX.Y.Z --sync-tap
cd ../homebrew-tap
git checkout -b kindling-vX.Y.Z
git add Formula/kindling.rb
git commit -m "kindling vX.Y.Z"
git push -u origin kindling-vX.Y.Z
gh pr create --repo eddacraft/homebrew-tap
```

## What the formula installs

Prebuilt binaries from [kindling releases](https://github.com/eddacraft/kindling/releases):

| Platform | Architecture          | Archive                                               |
| -------- | --------------------- | ----------------------------------------------------- |
| macOS    | Apple Silicon (arm64) | `kindling-<version>-aarch64-apple-darwin.tar.gz`      |
| macOS    | Intel (x86_64)        | `kindling-<version>-x86_64-apple-darwin.tar.gz`       |
| Linux    | arm64 (glibc)         | `kindling-<version>-aarch64-unknown-linux-gnu.tar.gz` |
| Linux    | x86_64 (glibc)        | `kindling-<version>-x86_64-unknown-linux-gnu.tar.gz`  |

Same layout as [anvil in the tap](https://github.com/eddacraft/homebrew-tap/blob/main/Formula/anvil.rb).
Alpine/musl hosts should use the [install script](../../install.sh) (`*-linux-musl` tarballs).
