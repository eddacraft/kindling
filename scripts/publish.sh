#!/bin/sh
# Publish the kindling workspace crates to crates.io in dependency order.
#
# crates.io requires a crate's dependencies to already be published before it
# can be published, so the order below is a topological sort of the intra-
# workspace dependency graph (leaves first, umbrella last):
#
#   kindling-types       (no intra-workspace deps)
#   kindling-store       -> types
#   kindling-provider    -> store, types
#   kindling-service     -> types, store, provider   (filter folded in)
#   kindling-server      -> types, store, service
#   kindling-client      -> types   (spool is an opt-in feature). NOTE: a
#                          versioned dev-dependency on kindling-server forces
#                          server to publish FIRST, even though it's not a prod dep.
#   kindling-runtime     -> types, client, server   (the anvil-first integration
#                          facade; bundles embedded daemon + durable spooled emit.
#                          Publishes AFTER client + server since it depends on both.)
#   eddacraft-kindling   -> types, store, service, client, server   (the binary,
#                          installs as `kindling`; cli + hook folded in. Published
#                          under `eddacraft-` because the bare `kindling` crate
#                          name on crates.io is owned by an unrelated project.)
#
# Prerequisites (CREDENTIAL-GATED — the maintainer must do these):
#   * A crates.io account + API token: pipe to stdin, e.g.
#     `printf '%s' "$TOKEN" | cargo login` (or set CARGO_REGISTRY_TOKEN).
#   * A clean, committed tree on the release commit/tag.
#   * `scripts/sync-vendored-schema.sh` already run (CI enforces no drift).
#
# Usage:
#   scripts/publish.sh            # real publish, pausing between crates
#   DRY_RUN=1 scripts/publish.sh  # cargo publish --dry-run for every crate
#
# Notes:
#   * crates.io needs a short moment to index a freshly published crate before
#     the next (dependent) crate can resolve it. We pause between publishes; if
#     a dependent publish fails with "no matching package", wait and re-run from
#     that crate.
#   * `cargo publish --dry-run` of a dependent crate FAILS until its deps are on
#     crates.io ("no matching package named ..."). That is expected for a
#     not-yet-published workspace and does NOT indicate a packaging problem —
#     verify packaging with `cargo package --list -p <crate>` instead.
#
# POSIX sh, shellcheck-clean.

set -eu

CRATES="
kindling-types
kindling-store
kindling-provider
kindling-service
kindling-server
kindling-client
kindling-runtime
eddacraft-kindling
"

DRY_RUN="${DRY_RUN:-0}"
# Seconds to wait after a successful publish for crates.io to index it.
INDEX_WAIT="${INDEX_WAIT:-20}"

for crate in $CRATES; do
  if [ "$DRY_RUN" = "1" ]; then
    printf '==> cargo publish --dry-run -p %s\n' "$crate"
    cargo publish --dry-run -p "$crate" || {
      printf 'note: dry-run of %s may fail until its deps are on crates.io\n' "$crate" >&2
    }
  else
    printf '==> cargo publish -p %s\n' "$crate"
    cargo publish -p "$crate"
    printf '    waiting %ss for crates.io to index %s...\n' "$INDEX_WAIT" "$crate"
    sleep "$INDEX_WAIT"
  fi
done

printf 'done.\n'
