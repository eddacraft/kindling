# Kindling Release Runbook

Purpose: ship Kindling's npm packages safely and consistently.

Kindling publishes 9 workspace packages to the `@eddacraft/*` scope on npm. All
publishing is automated via `.github/workflows/publish.yml`, which is triggered
by GitHub Releases.

## Release policy

- **Distribution:** npm registry, public scope `@eddacraft/*`, with provenance.
- **Trigger:** GitHub Release published from a tag on `main`.
- **Workflow source of truth:** `.github/workflows/publish.yml`.
- **Version source of truth:** root `package.json` `version`. The publish
  workflow asserts the release tag matches.

See the [branching strategy](branching-strategy.md) for the branch model that
this runbook assumes.

## 1. Preflight (required)

Run from the repo root with a clean working tree on the latest `main`:

```bash
git switch main && git pull --ff-only origin main
pnpm install --frozen-lockfile
pnpm run build
pnpm run type-check
pnpm run lint
pnpm run test
```

All four checks must pass. If any fails, stop and fix on a branch targeting
`main` before continuing.

Sanity assertions before promoting:

- Root `package.json` version matches the tag you intend to push.
- All workspace packages share the same version (lockstep release).
- `CHANGELOG.md` (if present) has notes for this version.
- README install instructions still work.

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
