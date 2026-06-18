#!/bin/sh
# Mark the retired TypeScript implementation packages as deprecated on npm.
#
# The Rust `kindling` daemon + the thin `@eddacraft/kindling` HTTP-over-UDS
# client now own the surface these packages used to provide (PORT-020). They are
# kept in the workspace until the 1.0.0 source-removal cut (the `-core`,
# `-store-sqlite`, and `-provider-local` removals are additionally gated on the
# eddacraft anvil TS-bridge cutover), but the published npm versions should warn
# consumers now.
#
# This runs `npm deprecate` for each retired package, which attaches a
# registry-side deprecation notice to ALL published versions (`@*`). It does NOT
# unpublish anything and is reversible (`npm deprecate '<pkg>@*' ''`).
#
# NOTE: `@eddacraft/kindling` (the umbrella thin client) is the REPLACEMENT and
# is intentionally NOT deprecated here.
#
# Prerequisites (CREDENTIAL-GATED — the maintainer must do these):
#   * Logged in to npm with publish rights on the @eddacraft scope:
#       npm login        (or set NPM_TOKEN / a .npmrc auth token)
#   * `npm whoami` succeeds.
#
# Usage:
#   scripts/deprecate.sh            # apply the deprecation notice
#   DRY_RUN=1 scripts/deprecate.sh  # print the npm commands without running them
#
# POSIX sh, shellcheck-clean.

set -eu

# Packages to deprecate (the retired TS implementation surface).
# The umbrella @eddacraft/kindling is deliberately omitted — it is the successor.
PKGS="
kindling-core
kindling-store-sqlite
kindling-store-sqljs
kindling-provider-local
kindling-server
kindling-cli
kindling-adapter-claude-code
"

MESSAGE="DEPRECATED: this package is deprecated and will be removed at v1.0.0. kindling is now a Rust daemon; use @eddacraft/kindling (thin client) or the \`kindling\` binary. See https://github.com/eddacraft/kindling."

DRY_RUN="${DRY_RUN:-0}"

for pkg in $PKGS; do
  spec="@eddacraft/${pkg}@*"
  if [ "$DRY_RUN" = "1" ]; then
    printf '==> [dry-run] npm deprecate %s\n' "$spec"
  else
    printf '==> npm deprecate %s\n' "$spec"
    npm deprecate "$spec" "$MESSAGE"
  fi
done

printf 'done.\n'
