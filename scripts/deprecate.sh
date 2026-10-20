#!/bin/sh
# Mark kindling's npm packages as deprecated.
#
# kindling no longer publishes to npm (D-010). It is distributed as the Rust
# `kindling` binary (cargo install eddacraft-kindling, Homebrew, install.sh),
# the crates on crates.io, and GitHub Release assets (binaries and the editor
# extension VSIX). This script attaches a registry-side deprecation notice to
# the npm packages that are still live, in two groups:
#
#   npm-channel  The packages whose npm channel is discontinued by D-010:
#                @eddacraft/kindling (its `latest` is still the retired 0.1.x
#                TS engine) and the OpenCode and PocketFlow adapters. Their
#                source stays in the repository, but nothing new is published.
#
#   retired-ts   The retired TypeScript implementation packages (PORT-020).
#                Already deprecated on the registry 2026-10-19; re-running
#                rewrites the notice so it points at the binary install rather
#                than the (now also deprecated) npm thin client.
#
#   all          Both groups.
#
# Each package is deprecated at ALL published versions (`@*`). This does NOT
# unpublish anything and is reversible (`npm deprecate '<pkg>@*' ''`).
#
# Prerequisites (CREDENTIAL-GATED: the maintainer must do these):
#   * Logged in to npm with publish rights on the @eddacraft scope:
#       npm login        (or a .npmrc auth token)
#   * `npm whoami` succeeds.
#
# Usage:
#   scripts/deprecate.sh npm-channel            # the D-010 deprecation
#   scripts/deprecate.sh retired-ts             # the PORT-020 deprecation
#   scripts/deprecate.sh all
#   DRY_RUN=1 scripts/deprecate.sh npm-channel  # print, do not run
#
# POSIX sh, shellcheck-clean.

set -eu

INSTALL_URL="https://github.com/eddacraft/kindling#install"
INSTALL_HINT="\`cargo install eddacraft-kindling\`, \`brew install eddacraft/tap/kindling\` or install.sh. See ${INSTALL_URL}"

# Packages whose npm channel is discontinued (D-010).
NPM_CHANNEL_PKGS="
kindling
kindling-adapter-opencode
kindling-adapter-pocketflow
"
NPM_CHANNEL_MESSAGE="DEPRECATED: kindling is no longer published to npm and these versions are built on the retired TypeScript engine. Install the Rust \`kindling\` binary instead: ${INSTALL_HINT}."

# The retired TS implementation surface (PORT-020).
RETIRED_TS_PKGS="
kindling-core
kindling-store-sqlite
kindling-store-sqljs
kindling-provider-local
kindling-server
kindling-cli
kindling-adapter-claude-code
"
RETIRED_TS_MESSAGE="DEPRECATED: this package is retired. kindling is now a Rust binary; install it with ${INSTALL_HINT}."

DRY_RUN="${DRY_RUN:-0}"

usage() {
  printf 'usage: %s {npm-channel|retired-ts|all}\n' "$0" >&2
  exit 2
}

deprecate_group() {
  pkgs="$1"
  message="$2"
  for pkg in $pkgs; do
    spec="@eddacraft/${pkg}@*"
    if [ "$DRY_RUN" = "1" ]; then
      printf '==> [dry-run] npm deprecate %s "%s"\n' "$spec" "$message"
    else
      printf '==> npm deprecate %s\n' "$spec"
      npm deprecate "$spec" "$message"
    fi
  done
}

[ "$#" -eq 1 ] || usage

case "$1" in
  npm-channel)
    deprecate_group "$NPM_CHANNEL_PKGS" "$NPM_CHANNEL_MESSAGE"
    ;;
  retired-ts)
    deprecate_group "$RETIRED_TS_PKGS" "$RETIRED_TS_MESSAGE"
    ;;
  all)
    deprecate_group "$NPM_CHANNEL_PKGS" "$NPM_CHANNEL_MESSAGE"
    deprecate_group "$RETIRED_TS_PKGS" "$RETIRED_TS_MESSAGE"
    ;;
  *)
    usage
    ;;
esac

printf 'done.\n'
