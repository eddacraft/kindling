# npm Publishing

| ID      | Owner  | Status |
| ------- | ------ | ------ |
| PUBLISH | @aneki | Done   |

## Purpose

Get all kindling packages published to npm so users can install them. This includes merging open PRs, adding package metadata, writing per-package READMEs, setting up publish scripts, and configuring CI for automated releases.

## In Scope

- Merge open PRs (#14 dx-hardening, #15 plugin-v2, #35 recall-skill)
- Claim `@eddacraft` npm scope
- Package metadata (description, keywords, repository, license, engines) for all packages
- Per-package README files
- CHANGELOG consolidation
- Publish scripts (pnpm publish pipeline with topological ordering)
- CI workflow for publish on tag/release

## Out of Scope

- Rust binary distribution (module 02)
- Plugin marketplace submission (separate effort)

## Interfaces

**Depends on:**

- Open PRs merged to main

**Exposes:**

- Published npm packages under `@eddacraft/` scope
- CI workflow for future releases

## Ready Checklist

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one task defined

## Tasks

### PUBLISH-001: Merge open PRs

- **Intent:** All pending work lands on main
- **Expected Outcome:** PRs #14, #15, #35 merged, main branch green
- **Validation:** `git log --oneline -10` shows merged commits, `pnpm run test` passes
- **Status:** Ready

### PUBLISH-002: Claim npm scope

- **Intent:** Reserve the `@eddacraft` scope on npm
- **Expected Outcome:** `@eddacraft` org exists on npmjs.com, team has publish access
- **Validation:** `npm org ls @eddacraft` returns members
- **Status:** Ready

### PUBLISH-003: Package metadata

- **Intent:** All packages have correct metadata for npm discovery
- **Expected Outcome:** Every `package.json` has description, keywords, repository, license, engines, and files fields
- **Validation:** `pnpm -r exec -- node -e "const p=require('./package.json'); console.log(p.name, !!p.description, !!p.repository)"`
- **Status:** Ready

### PUBLISH-004: Per-package READMEs

- **Intent:** Each package has a README suitable for its npm page
- **Expected Outcome:** Every package directory contains a README.md with install, usage, and API overview
- **Validation:** `for d in packages/*/; do test -f "$d/README.md" && echo "OK: $d" || echo "MISSING: $d"; done`
- **Status:** Ready

### PUBLISH-005: Publish pipeline

- **Intent:** One command publishes all packages in correct order
- **Expected Outcome:** Script handles topological ordering (core → store/provider → main → adapters → cli)
- **Validation:** `pnpm publish -r --dry-run` completes without errors
- **Status:** Ready

### PUBLISH-006: CI release workflow

- **Intent:** Tagging a release triggers automated publish
- **Expected Outcome:** GitHub Actions workflow publishes to npm on version tag push
- **Validation:** Workflow file exists and passes `act` dry run or manual trigger
- **Status:** Draft
