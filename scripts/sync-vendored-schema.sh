#!/bin/sh
# Sync the canonical cross-language schema contract into the per-crate vendored
# copies that `cargo publish` packages.
#
# The repo-root `schema/` directory is the single canonical source of the
# cross-language SQLite schema contract (TypeScript and Rust both read it).
# `cargo publish` only packages files under a crate's own directory, so each
# Rust crate that embeds the contract with `include_str!` keeps a COMMITTED
# vendored copy inside the crate. This script regenerates those copies from the
# canonical source.
#
# Usage:
#   scripts/sync-vendored-schema.sh
#
# CI (the `vendored-schema` job in .github/workflows/rust.yml) runs this and then
# `git diff --exit-code` over the vendored copies; a canonical schema change that
# was not re-synced (leaving a dirty tree) fails the build.
#
# POSIX sh, shellcheck-clean. Safe to re-run (idempotent).

set -eu

# Resolve the repository root from this script's location so the script works
# regardless of the caller's working directory. Unset CDPATH so a user's CDPATH
# cannot redirect the `cd` below.
unset CDPATH
script_dir=$(cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(cd -- "${script_dir}/.." && pwd)

canonical_dir="${repo_root}/schema"

# Map of "source file -> destination path" pairs. Each crate vendors only the
# contract files it actually embeds:
#   * kindling-store  embeds schema.sql + version.json
#   * kindling-client embeds version.json (schema-version constant only)
copy() {
  src="$1"
  dst="$2"
  if [ ! -f "${src}" ]; then
    echo "error: canonical source missing: ${src}" >&2
    exit 1
  fi
  dst_dir=$(dirname -- "${dst}")
  mkdir -p "${dst_dir}"
  cp -- "${src}" "${dst}"
  echo "synced ${src#"${repo_root}/"} -> ${dst#"${repo_root}/"}"
}

copy "${canonical_dir}/schema.sql" \
  "${repo_root}/crates/kindling-store/schema/schema.sql"
copy "${canonical_dir}/version.json" \
  "${repo_root}/crates/kindling-store/schema/version.json"
copy "${canonical_dir}/version.json" \
  "${repo_root}/crates/kindling-client/schema/version.json"

echo "vendored schema copies are up to date."
