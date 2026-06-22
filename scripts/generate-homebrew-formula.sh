#!/usr/bin/env bash
# generate-homebrew-formula.sh: update packaging/homebrew/kindling.rb with
# release version and macOS tarball SHA256 checksums from GitHub.
#
# Usage:
#   ./scripts/generate-homebrew-formula.sh              # latest release
#   ./scripts/generate-homebrew-formula.sh v0.2.0       # specific tag
#   ./scripts/generate-homebrew-formula.sh v0.2.0 --sync-tap
#   KINDLING_VERSION=v0.2.0 ./scripts/generate-homebrew-formula.sh
#
# Environment:
#   KINDLING_TAP_DIR  local clone of github.com/eddacraft/homebrew-tap (the shared
#                     tap that already ships anvil; default: sibling ../homebrew-tap)
#
# Requires: curl, sed

set -euo pipefail

REPO="${KINDLING_REPO:-eddacraft/kindling}"
FORMULA="${KINDLING_FORMULA:-packaging/homebrew/kindling.rb}"
SYNC_TAP=false
TAG_ARG=""

info()  { printf '\033[1;34m==>\033[0m %s\n' "$1"; }
error() { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }

command_exists() { command -v "$1" >/dev/null 2>&1; }

for arg in "$@"; do
  case "$arg" in
    --sync-tap) SYNC_TAP=true ;;
    -h|--help)
      sed -n '2,14p' "$0"
      exit 0
      ;;
    *)
      if [ -z "$TAG_ARG" ]; then
        TAG_ARG="$arg"
      fi
      ;;
  esac
done

command_exists curl || error "curl is required"
command_exists sed || error "sed is required"

[ -f "$FORMULA" ] || error "formula not found: $FORMULA"

resolve_tag() {
  if [ -n "${KINDLING_VERSION:-}" ]; then
    printf '%s\n' "$KINDLING_VERSION"
    return
  fi
  if [ -n "$TAG_ARG" ]; then
    printf '%s\n' "$TAG_ARG"
    return
  fi
  info "Resolving latest release tag..."
  curl -fsSL "https://api.github.com/repos/${REPO}/releases?per_page=1" \
    | grep '"tag_name"' | head -1 \
    | sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/'
}

TAG="$(resolve_tag)"
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

# Replace SHA256 placeholders (or existing checksums on re-run).
sed -i "s/REPLACE_WITH_SHA256_AARCH64_APPLE_DARWIN/${SHA_AARCH64}/" "$TMP"
sed -i "s/REPLACE_WITH_SHA256_X86_64_APPLE_DARWIN/${SHA_X86_64}/" "$TMP"
sed -i "/aarch64-apple-darwin\.tar\.gz\"$/,+1 s/sha256 \"[a-f0-9]*\"/sha256 \"${SHA_AARCH64}\"/" "$TMP"
sed -i "/x86_64-apple-darwin\.tar\.gz\"$/,+1 s/sha256 \"[a-f0-9]*\"/sha256 \"${SHA_X86_64}\"/" "$TMP"

mv "$TMP" "$FORMULA"
trap - EXIT

info "Updated ${FORMULA}"

resolve_tap_dir() {
  if [ -n "${KINDLING_TAP_DIR:-}" ]; then
    printf '%s\n' "$KINDLING_TAP_DIR"
    return
  fi
  local sibling
  sibling="$(cd "$(dirname "$FORMULA")/../.." && pwd)/../homebrew-tap"
  if [ -d "$sibling/Formula" ]; then
    printf '%s\n' "$sibling"
  fi
}

if [ "$SYNC_TAP" = true ]; then
  TAP_DIR="$(resolve_tap_dir || true)"
  [ -n "${TAP_DIR:-}" ] || error "--sync-tap requires KINDLING_TAP_DIR or a ../homebrew-tap clone"
  mkdir -p "${TAP_DIR}/Formula"
  cp "$FORMULA" "${TAP_DIR}/Formula/kindling.rb"
  info "Copied to ${TAP_DIR}/Formula/kindling.rb"
fi

printf '\nNext steps:\n'
printf '  1. Review: git diff %s\n' "$FORMULA"
if [ "$SYNC_TAP" != true ]; then
  printf '  2. Sync to tap: KINDLING_TAP_DIR=../homebrew-tap %s %s --sync-tap\n' "$0" "$TAG"
  printf '  3. In homebrew-tap: git add Formula/kindling.rb && git commit && git push\n'
else
  printf '  2. In homebrew-tap: git add Formula/kindling.rb && git commit -m "kindling %s" && git push\n' "$VERSION"
fi
printf '  3. Test: brew update && brew install eddacraft/tap/kindling\n'