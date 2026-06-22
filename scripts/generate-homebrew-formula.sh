#!/usr/bin/env bash
# generate-homebrew-formula.sh: update packaging/homebrew/kindling.rb with
# release version and macOS tarball SHA256 checksums from GitHub.
#
# Usage:
#   ./scripts/generate-homebrew-formula.sh              # latest release
#   ./scripts/generate-homebrew-formula.sh v0.2.0       # specific tag
#   KINDLING_VERSION=v0.2.0 ./scripts/generate-homebrew-formula.sh
#
# Requires: curl, sed

set -euo pipefail

REPO="${KINDLING_REPO:-eddacraft/kindling}"
FORMULA="${KINDLING_FORMULA:-packaging/homebrew/kindling.rb}"

info()  { printf '\033[1;34m==>\033[0m %s\n' "$1"; }
error() { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }

command_exists() { command -v "$1" >/dev/null 2>&1; }

command_exists curl || error "curl is required"
command_exists sed || error "sed is required"

[ -f "$FORMULA" ] || error "formula not found: $FORMULA"

resolve_tag() {
  if [ -n "${KINDLING_VERSION:-}" ]; then
    printf '%s\n' "$KINDLING_VERSION"
    return
  fi
  if [ "${1:-}" != "" ]; then
    printf '%s\n' "$1"
    return
  fi
  info "Resolving latest release tag..."
  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 \
    | sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/'
}

TAG="$(resolve_tag "${1:-}")"
[ -n "$TAG" ] || error "could not resolve release tag"

# Bare version (no leading v) for the formula's version field and archive names.
VERSION="${TAG#v}"

info "Release tag: ${TAG} (formula version: ${VERSION})"

fetch_sha256() {
  local target="$1"
  local sidecar_url="https://github.com/${REPO}/releases/download/${TAG}/kindling-${VERSION}-${target}.tar.gz.sha256"
  info "Fetching checksum: ${sidecar_url}"
  local line
  line="$(curl -fsSL "$sidecar_url")" || error "failed to download ${sidecar_url}"
  # Sidecar format: "<sha256>  kindling-<version>-<target>.tar.gz"
  printf '%s\n' "$line" | awk '{print $1}'
}

SHA_AARCH64="$(fetch_sha256 "aarch64-apple-darwin")"
SHA_X86_64="$(fetch_sha256 "x86_64-apple-darwin")"

info "aarch64-apple-darwin: ${SHA_AARCH64}"
info "x86_64-apple-darwin:  ${SHA_X86_64}"

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

cp "$FORMULA" "$TMP"

# Update version line (bare semver, no leading v).
sed -i "s/^  version \".*\"/  version \"${VERSION}\"/" "$TMP"

# Replace SHA256 placeholders.
sed -i "s/REPLACE_WITH_SHA256_AARCH64_APPLE_DARWIN/${SHA_AARCH64}/" "$TMP"
sed -i "s/REPLACE_WITH_SHA256_X86_64_APPLE_DARWIN/${SHA_X86_64}/" "$TMP"

mv "$TMP" "$FORMULA"
trap - EXIT

info "Updated ${FORMULA}"
printf '\nNext steps:\n'
printf '  1. Review the diff: git diff %s\n' "$FORMULA"
printf '  2. Copy to eddacraft/homebrew-tap: Formula/kindling.rb\n'
printf '  3. Commit and push the tap repo\n'