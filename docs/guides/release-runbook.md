# kindling Release Runbook

Purpose: ship kindling safely and consistently across its two distribution
channels.

kindling is Rust-canonical and ships through two channels:

1. **crates.io** — the seven Rust crates in `crates/` (the engine). Published
   manually with `scripts/publish.sh` (credential-gated). See
   [crates.io release](#cratesio-release) below.
2. **npm** — the thin `@eddacraft/kindling` client plus the adapters, published
   to the `@eddacraft/*` scope. Automated via `.github/workflows/publish.yml`,
   triggered by GitHub Releases.

## Release policy

- **Distribution:** crates.io (Rust crates) and the npm `@eddacraft/*` scope
  (thin client + adapters), both public, npm with provenance.
- **Trigger:** GitHub Release published from a tag on `main` (drives the npm
  workflow; the crates.io publish is run by the maintainer).
- **Workflow source of truth:** `.github/workflows/publish.yml` (npm),
  `scripts/publish.sh` (crates.io).
- **Version source of truth:** the Rust workspace version in the root
  `Cargo.toml` for crates; root `package.json` `version` for npm. The npm
  publish workflow asserts the release tag matches `package.json`.

> The Rust workspace and the npm packages are versioned independently. Keep the
> crates in lockstep with each other (one workspace version) and the npm
> packages in lockstep with each other.

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

# npm (thin client + adapters)
pnpm install --frozen-lockfile
pnpm run build
pnpm run type-check
pnpm run lint
pnpm run test
```

All checks must pass. If any fails, stop and fix on a branch targeting `main`
before continuing.

Sanity assertions before promoting:

- Root `Cargo.toml` workspace version matches the crates you intend to publish.
- Root `package.json` version matches the tag you intend to push.
- All crates share one workspace version; all npm packages share one version
  (lockstep within each channel).
- `CHANGELOG.md` has notes for this release.
- README install instructions (the install script, `cargo install eddacraft-kindling`, and npm) still work.

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

1. Bump root `package.json` version.
2. Bump every workspace `packages/*/package.json` version to the same value.
3. Update internal `@eddacraft/kindling-*` dependency ranges to the new
   version.
4. Update `CHANGELOG.md` with release notes.
5. Update README install snippets if any pin a version.
6. Commit on the release-prep branch:

   ```bash
   git add package.json packages/*/package.json CHANGELOG.md README.md
   git commit -m "chore(release): prepare vX.Y.Z"
   ```

7. Merge the release-prep branch to `main`, or include the bump on the active
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

Publishing the GitHub Release triggers `publish.yml`, which:

1. Re-runs CI as a gate.
2. Validates the tag matches the root `package.json` version.
3. Publishes every workspace package with `pnpm publish -r --access public`
   and npm provenance enabled.

Monitor the workflow until it completes:

```bash
gh run watch
```

## 5. Verify the release

- `npm view @eddacraft/kindling version` returns the new version.
- `npx @eddacraft/kindling-cli@latest --version` works.
- The GitHub Release page lists the tag and the publish workflow run is green.

## crates.io release

The Rust crates are published to crates.io by the maintainer with
`scripts/publish.sh`. This step is **credential-gated** and is not run by CI.

Prerequisites:

- A crates.io account and API token: `cargo login <token>` (or set
  `CARGO_REGISTRY_TOKEN`).
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

## Hotfix flow

If a critical bug is discovered on a published version:

1. Branch `hotfix/<slug>` from `main` (or the active `release/*` if one
   exists).
2. Land the fix and bump the patch version.
3. PR `hotfix/* → main`. Merge after CI passes.
4. Tag `vX.Y.Z+1` on `main`, create the GitHub Release. The publish workflow
   handles the rest.

## Dry-run publishing

You can test the publish pipeline without releasing:

```bash
gh workflow run publish.yml -f dry-run=true
```

This runs `npm pack --dry-run` for every workspace package and skips the
actual publish step.

## Troubleshooting

- **Tag/version mismatch:** the publish workflow refuses to publish when the
  release tag doesn't match `package.json`. Bump the version on `main`, retag.
- **CI gate fails:** the publish job re-runs CI before pushing to npm. Fix on a
  branch targeting `main`, then create a follow-up release.
- **Partial publish:** if `pnpm publish -r` fails partway through, do not
  delete the partial publish. Bump the patch version and re-release the full
  set; npm doesn't allow republishing the same version.

## Related Docs

- [Branching Strategy](branching-strategy.md)
- [Worktree Policy](worktree-policy.md)
