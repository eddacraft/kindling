#!/bin/sh
# kindling native-binary installer.
#
#   curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/packaging/install.sh | sh
#
# Downloads the prebuilt `kindling` binary for your OS/arch from the latest
# GitHub Release, verifies its SHA256 against the published sidecar, and installs
# it into a bin directory on your PATH.
#
# Environment overrides:
#   KINDLING_VERSION       Install a specific version (e.g. 0.1.2) instead of
#                          the latest release. A leading "v" is accepted.
#   KINDLING_INSTALL_DIR   Install directory (default: $HOME/.local/bin).
#   KINDLING_REPO          GitHub repo (default: eddacraft/kindling).
#
# Platforms: linux (x86_64, aarch64) and macOS (x86_64, aarch64).
# Windows is not covered by this shell installer — download the
# *-pc-windows-gnu.zip asset from the GitHub Release manually, or use the
# planned `winget`/scoop channel.
#
# POSIX sh, shellcheck-clean. Idempotent: re-running upgrades/replaces the binary.

set -eu

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
REPO="${KINDLING_REPO:-eddacraft/kindling}"
INSTALL_DIR="${KINDLING_INSTALL_DIR:-${HOME}/.local/bin}"
BIN_NAME="kindling"

# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------
info()  { printf '==> %s\n' "$1"; }
warn()  { printf 'warning: %s\n' "$1" >&2; }
err()   { printf 'error: %s\n' "$1" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Tooling checks
# ---------------------------------------------------------------------------
have() { command -v "$1" >/dev/null 2>&1; }

require_tools() {
  have curl || err "curl is required but not found. Install curl and re-run."
  have tar  || err "tar is required but not found. Install tar and re-run."

  # SHA256 verifier: linux ships sha256sum, macOS ships shasum.
  if have sha256sum; then
    SHA_CMD="sha256sum"
  elif have shasum; then
    SHA_CMD="shasum -a 256"
  else
    err "no SHA256 tool found (need 'sha256sum' or 'shasum'). Install one and re-run."
  fi
}

# ---------------------------------------------------------------------------
# Platform detection -> release target triple
# ---------------------------------------------------------------------------
# Maps to the artefact names produced by .github/workflows/release.yml:
#   kindling-<version>-<target>.tar.gz   (+ .sha256 sidecar)
# Linux uses the gnu target, macOS uses apple-darwin.
detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)  os_part="unknown-linux-gnu" ;;
    Darwin) os_part="apple-darwin" ;;
    *)      err "unsupported OS: ${os}. This installer supports Linux and macOS only. For Windows, download the *-pc-windows-gnu.zip asset from the GitHub Release." ;;
  esac

  case "$arch" in
    x86_64 | amd64)        arch_part="x86_64" ;;
    aarch64 | arm64)       arch_part="aarch64" ;;
    *)                     err "unsupported architecture: ${arch}. Supported: x86_64, aarch64." ;;
  esac

  printf '%s-%s' "$arch_part" "$os_part"
}

# ---------------------------------------------------------------------------
# Version resolution
# ---------------------------------------------------------------------------
# Returns the version WITHOUT a leading "v" (artefact names use the bare
# version, while release tags are "vX.Y.Z").
resolve_version() {
  if [ -n "${KINDLING_VERSION:-}" ]; then
    # Strip a leading "v" if the user supplied one.
    printf '%s' "${KINDLING_VERSION#v}"
    return 0
  fi

  api_url="https://api.github.com/repos/${REPO}/releases/latest"
  # Extract "tag_name": "vX.Y.Z" from the JSON without requiring jq.
  tag="$(curl -fsSL "$api_url" \
    | grep -m1 '"tag_name"' \
    | sed -e 's/.*"tag_name"[[:space:]]*:[[:space:]]*"//' -e 's/".*//')"

  [ -n "$tag" ] || err "could not resolve the latest release version from ${api_url}. Set KINDLING_VERSION to install a specific version."
  printf '%s' "${tag#v}"
}

# ---------------------------------------------------------------------------
# Download + verify
# ---------------------------------------------------------------------------
# Verify the archive against its .sha256 sidecar. The sidecar is the output of
# `sha256sum <archive>` (PORT-010 release.yml), i.e. "<hex>  <filename>".
verify_sha256() {
  archive="$1"
  sidecar="$2"

  expected="$(awk '{print $1}' "$sidecar")"
  [ -n "$expected" ] || err "checksum sidecar ${sidecar} is empty or malformed."

  actual="$($SHA_CMD "$archive" | awk '{print $1}')"
  [ -n "$actual" ] || err "failed to compute SHA256 of ${archive}."

  if [ "$expected" != "$actual" ]; then
    err "SHA256 mismatch for ${archive}
  expected: ${expected}
  actual:   ${actual}
Refusing to install a tampered or corrupt download."
  fi
  info "Checksum verified (${actual})."
}

# ---------------------------------------------------------------------------
# Install
# ---------------------------------------------------------------------------
install_binary() {
  target="$1"
  version="$2"
  workdir="$3"

  archive="kindling-${version}-${target}.tar.gz"
  base_url="https://github.com/${REPO}/releases/download/v${version}"

  info "Downloading ${archive}..."
  curl -fSL --proto '=https' --tlsv1.2 -o "${workdir}/${archive}" \
    "${base_url}/${archive}" \
    || err "failed to download ${base_url}/${archive}. Check that v${version} has a ${target} asset."

  info "Downloading checksum..."
  curl -fSL --proto '=https' --tlsv1.2 -o "${workdir}/${archive}.sha256" \
    "${base_url}/${archive}.sha256" \
    || err "failed to download checksum ${base_url}/${archive}.sha256."

  ( cd "$workdir" && verify_sha256 "$archive" "${archive}.sha256" )

  info "Extracting..."
  tar -xzf "${workdir}/${archive}" -C "$workdir" \
    || err "failed to extract ${archive}."

  [ -f "${workdir}/${BIN_NAME}" ] || err "extracted archive did not contain a '${BIN_NAME}' binary."

  mkdir -p "$INSTALL_DIR" || err "could not create install dir ${INSTALL_DIR}."

  # Atomic-ish replace: move into place then chmod.
  install_path="${INSTALL_DIR}/${BIN_NAME}"
  if ! mv -f "${workdir}/${BIN_NAME}" "$install_path" 2>/dev/null; then
    cp -f "${workdir}/${BIN_NAME}" "$install_path" \
      || err "could not write ${install_path}. Try a writable KINDLING_INSTALL_DIR (e.g. KINDLING_INSTALL_DIR=/usr/local/bin with sudo)."
  fi
  chmod +x "$install_path" || err "could not chmod +x ${install_path}."

  info "Installed ${BIN_NAME} ${version} to ${install_path}"
}

# ---------------------------------------------------------------------------
# PATH hint
# ---------------------------------------------------------------------------
path_hint() {
  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
      : # already on PATH
      ;;
    *)
      warn "${INSTALL_DIR} is not on your PATH."
      printf '  Add it for your shell, e.g.:\n'
      # The literal "$PATH" is meant to be printed verbatim for the user to
      # paste into their shell profile, not expanded here.
      # shellcheck disable=SC2016
      printf '    export PATH="%s:$PATH"\n' "$INSTALL_DIR"
      printf '  (add that line to ~/.bashrc, ~/.zshrc, or your shell profile)\n'
      ;;
  esac
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
  printf '\n  kindling installer\n  Local memory for AI-assisted development\n\n'

  require_tools

  target="$(detect_target)"
  info "Detected platform: ${target}"

  version="$(resolve_version)"
  info "Installing version: ${version}"

  # Create a self-cleaning work directory.
  workdir="$(mktemp -d "${TMPDIR:-/tmp}/kindling-install.XXXXXX")" \
    || err "could not create a temporary directory."
  trap 'rm -rf "$workdir"' EXIT INT TERM

  install_binary "$target" "$version" "$workdir"
  path_hint

  printf '\n  Done. Verify with:\n    %s --version\n\n' "$BIN_NAME"
}

main
