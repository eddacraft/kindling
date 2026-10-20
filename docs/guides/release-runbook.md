# kindling Release Runbook

Purpose: ship kindling safely and consistently across its distribution
channels.

kindling is Rust-canonical. A release is a **GitHub Release** plus a
**crates.io** publish:

1. **GitHub Release:** publishing a Release from a `vX.Y.Z` tag on `main`
   triggers `.github/workflows/release.yml`, which cross-builds the `kindling`
   binary for all 7 targets, attaches the archives and `.sha256` sidecars,
   attaches the editor extension VSIX (`kindling-<version>.vsix` plus its
   `.sha256`), and opens the Homebrew tap PR. The install script
   (`install.sh`) downloads from these assets.
2. **crates.io:** the seven Rust crates in `crates/` (the engine). Published
   manually with `scripts/publish.sh` (credential-gated). See
   [crates.io release](#cratesio-release) below.

> **kindling does not publish to npm (D-010).** The packages under `packages/`
> are `private`. The `@eddacraft/*` versions on npm are the retired 0.1.x
> TypeScript engine; deprecate them with `scripts/deprecate.sh` (see
> [npm deprecation](#npm-deprecation)). No release step touches npm.

## Release policy

- **Distribution:** GitHub Release assets (binaries, VSIX), the Homebrew tap
  (`eddacraft/homebrew-tap`, fed from the Release assets), and crates.io.
- **Trigger:** GitHub Release published from a tag on `main` (drives
  `release.yml`; the crates.io publish is run by the maintainer).
- **Workflow source of truth:** `.github/workflows/release.yml` (binaries, VSIX,
  Homebrew tap PR), `scripts/publish.sh` (crates.io).
- **Version source of truth:** the Rust workspace version in the root
  `Cargo.toml` for the crates and the binary. The VSIX takes its version from
  `packages/kindling-adapter-vscode/package.json`, and `release.yml` fails if
  that does not match the release tag.

> Keep the crates in lockstep with each other (one workspace version) and keep
> the root `package.json` and `packages/*/package.json` versions in lockstep
> with the crates, so the VSIX version matches the tag.

See the [branching strategy](branching-strategy.md) for the branch model that
this runbook assumes.

## 1. Preflight (required)

Run from the repo root with a clean working tree on the latest `main`:

```bash
git switch main && git pull --ff-only origin main

# Rust (engine)
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
cargo test
scripts/sync-vendored-schema.sh   # CI fails on schema drift

# TypeScript (thin client, adapters, editor extension)
pnpm install --frozen-lockfile
pnpm run build
pnpm run type-check
pnpm run lint
pnpm run test
pnpm --filter @eddacraft/kindling-adapter-vscode run package -- --out /tmp/kindling-vsix
```

All checks must pass. If any fails, stop and fix on a branch targeting `main`
before continuing.

Sanity assertions before promoting:

- Root `Cargo.toml` workspace version matches the tag you intend to push.
- `packages/kindling-adapter-vscode/package.json` version matches the same tag
  (the VSIX is named from it).
- `CHANGELOG.md` has notes for this release.
- README install instructions (the install script, `cargo install eddacraft-kindling`, Homebrew) still work.

## 2. Stabilise the release

All day-to-day work lands on `main` through PRs. For small, low-risk releases,
release directly from `main`. For anything larger, cut a short-lived
`release/*` branch from `main` and do stabilisation there.

### Option A: direct release (small releases)

Use this when the change set is small, reviewable, and already stable on `main`.

1. Ensure `main` is green on CI.
2. Ensure the version bump and changelog are already merged to `main`.
3. Tag and create the GitHub Release.

### Option B: stabilise on `release/*` (non-trivial releases)

Use this when you want a short hardening window for packaging, docs, final bug
fixes, or release validation.

1. Ensure `main` is green.
2. Create `release/x.y.z` from `main`.
3. Allow only release hardening on the release branch (version bumps,
   changelog, packaging, docs, last-mile fixes).
4. Open a PR from `release/x.y.z` to `main`.
5. Once the release gate passes, merge the PR.

```bash
git switch main && git pull --ff-only origin main
git switch -c release/x.y.z
git push -u origin release/x.y.z

gh pr create --base main --head release/x.y.z --title "release: vX.Y.Z" \
  --body "Promote release/x.y.z to main for release vX.Y.Z"
```

## 3. Bump versions

Do version bumps on a release-prep branch targeting `main`.

1. Bump the workspace version in the root `Cargo.toml` (and the intra-workspace
   `version = "..."` dependency pins), then refresh `Cargo.lock`.
2. Bump root `package.json` and every `packages/*/package.json` to the same
   value (the VSIX version comes from the editor extension's manifest).
3. Update `CHANGELOG.md` with release notes.
4. Update README install snippets if any pin a version.
5. Commit on the release-prep branch:

   ```bash
   git add Cargo.toml Cargo.lock crates/*/Cargo.toml package.json packages/*/package.json CHANGELOG.md README.md
   git commit -m "chore(release): prepare vX.Y.Z"
   ```

6. Merge the release-prep branch to `main`, or include the bump on the active
   `release/*` branch.

## 4. Tag and create the GitHub Release

After the release-prep PR or `release/* → main` PR is merged:

```bash
git switch main && git pull --ff-only origin main
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

Then create the GitHub Release:

```bash
gh release create vX.Y.Z \
  --title "vX.Y.Z" \
  --notes-from-tag \
  --target main
```

Publishing the GitHub Release triggers `release.yml`, which:

1. Cross-builds the 7 targets and smoke-tests them (exec on Linux, format check
   for macOS and Windows).
2. Attaches every archive and `.sha256` sidecar to the Release.
3. Packages the VSIX, checks its version matches the tag, and attaches it with
   its `.sha256`.
4. Opens a PR in `eddacraft/homebrew-tap` with the regenerated formula
   (requires the `HOMEBREW_TAP_TOKEN` secret).

Monitor the workflow until it completes:

```bash
gh run watch
```

## 5. Verify the release

- The GitHub Release lists the 7 archives, their `.sha256` sidecars, and
  `kindling-X.Y.Z.vsix` with its `.sha256`.
- `curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh`
  installs the new binary; `kindling --version` matches.
- The Homebrew tap PR is open; merge it, then `brew install eddacraft/tap/kindling`
  installs the new version.

## crates.io release

The Rust crates are published to crates.io by the maintainer with
`scripts/publish.sh`. This step is **credential-gated** and is not run by CI.

Prerequisites:

- A crates.io account and API token: `printf '%s' "$TOKEN" | cargo login` (token
  is read from stdin; `cargo login <token>` is deprecated), or set
  `CARGO_REGISTRY_TOKEN`.
- A clean, committed tree on the release commit/tag.
- `scripts/sync-vendored-schema.sh` already run (CI enforces no drift).

crates.io requires each crate's dependencies to be published first, so the
script publishes in topological order (leaves first, the `kindling` binary
last): `kindling-types` → `kindling-store` → `kindling-provider` →
`kindling-service` → `kindling-server` → `kindling-client` → `eddacraft-kindling`.
`kindling-client` carries a versioned dev-dependency on `kindling-server`, so the
server must already be on crates.io when the client publishes — this is why the
server precedes the client (the order encoded in `scripts/publish.sh`).

```bash
# Dry run (cargo publish --dry-run for every crate)
DRY_RUN=1 scripts/publish.sh

# Real publish (pauses between crates so crates.io can index each one)
scripts/publish.sh
```

Notes:

- A dependent crate's `--dry-run` fails with "no matching package" until its
  deps are actually on crates.io — that is expected for a not-yet-published
  workspace and does not indicate a packaging problem. Verify packaging with
  `cargo package --list -p <crate>` instead.
- If a publish fails partway, wait for indexing and re-run from the failed
  crate. crates.io does not allow republishing the same version — bump the
  workspace version and re-release the full set if needed.

Verify:

- `cargo search kindling` (or the crate page) shows the new version.
- `cargo install eddacraft-kindling` installs the new binary; `kindling --version` matches.

## npm deprecation

kindling no longer publishes to npm, but the retired 0.1.x packages are still
on the registry. `scripts/deprecate.sh` attaches a deprecation notice that
points users at the binary install. It needs an npm login with publish rights
on the `@eddacraft` scope and is run once by the maintainer, not by CI:

```bash
DRY_RUN=1 scripts/deprecate.sh npm-channel   # preview
scripts/deprecate.sh npm-channel             # @eddacraft/kindling + OpenCode/PocketFlow adapters
scripts/deprecate.sh retired-ts              # re-point the PORT-020 notices (optional)
```

Deprecation is reversible (`npm deprecate '<pkg>@*' ''`) and does not
unpublish anything.

## Hotfix flow

If a critical bug is discovered on a published version:

1. Branch `hotfix/<slug>` from `main` (or the active `release/*` if one
   exists).
2. Land the fix and bump the patch version.
3. PR `hotfix/* → main`. Merge after CI passes.
4. Tag `vX.Y.Z+1` on `main`, create the GitHub Release (`release.yml` handles
   the assets and the tap PR), then run `scripts/publish.sh` for crates.io.

## Rehearsing the release build

You can run the cross-build without releasing:

```bash
gh workflow run release.yml
```

A `workflow_dispatch` run builds all 7 targets as workflow artefacts; it does
not upload to a Release, package the VSIX or open a tap PR. Package the VSIX
locally with the preflight command above.

## Troubleshooting

- **VSIX version mismatch:** `release.yml` fails the `package-vsix` job when
  `kindling-<tag>.vsix` is not produced. Bump
  `packages/kindling-adapter-vscode/package.json` on `main` and re-release.
- **Homebrew tap PR missing:** check the `HOMEBREW_TAP_TOKEN` secret, or run
  `./scripts/generate-homebrew-formula.sh vX.Y.Z --sync-tap` manually.
- **Partial crates.io publish:** see the crates.io notes above.

## Related Docs

- [Branching Strategy](branching-strategy.md)
- [Worktree Policy](worktree-policy.md)
- [Distribution channels](../../packaging/README.md)
