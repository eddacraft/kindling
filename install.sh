#!/bin/sh
set -eu

# kindling installer
#
# Downloads the prebuilt `kindling` binary from a GitHub release and installs
# it. No Node.js or Rust toolchain required.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
#
# Environment overrides:
#   KINDLING_VERSION      release tag to install (e.g. v0.1.0); default: latest
#   KINDLING_INSTALL_DIR  install directory; default: ~/.local/bin
#   KINDLING_REPO         GitHub repo to install from; default: eddacraft/kindling
#   KINDLING_NO_DEMO      set to 1 to skip the sample memory prompt
#
# POSIX sh, shellcheck-clean.

REPO="${KINDLING_REPO:-eddacraft/kindling}"
BIN_NAME="kindling"
INSTALL_DIR="${KINDLING_INSTALL_DIR:-${HOME}/.local/bin}"

# Set once the binary is installed; used by setup + the closing hints so we work
# even when INSTALL_DIR is not yet on PATH.
KINDLING_CMD=""

# "yes" once the sample memory is loaded, "skipped" otherwise; tailors the
# closing hints.
DEMO_LOADED="skipped"

# --- helpers ---

info()  { printf '\033[1;34m==>\033[0m %s\n' "$1"; }
warn()  { printf '\033[1;33mwarning:\033[0m %s\n' "$1"; }
error() { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }

command_exists() { command -v "$1" >/dev/null 2>&1; }

# True when a controlling terminal can be opened for prompts. Under
# `curl | sh` stdin is the script, so prompts read from /dev/tty instead; in CI
# or other headless runs there is no terminal, so prompts are skipped.
has_tty() { (: </dev/tty) 2>/dev/null; }

# --- platform detection ---

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$arch" in
    x86_64 | amd64) cpu="x86_64" ;;
    aarch64 | arm64) cpu="aarch64" ;;
    *) error "unsupported architecture: ${arch}" ;;
  esac

  case "$os" in
    Linux)
      libc="gnu"
      if [ -f /etc/alpine-release ] || (ldd --version 2>&1 | grep -qi musl); then
        libc="musl"
      fi
      TARGET="${cpu}-unknown-linux-${libc}"
      EXT="tar.gz"
      ;;
    Darwin)
      TARGET="${cpu}-apple-darwin"
      EXT="tar.gz"
      ;;
    MINGW* | MSYS* | CYGWIN* | Windows_NT)
      TARGET="x86_64-pc-windows-gnu"
      EXT="zip"
      ;;
    *) error "unsupported OS: ${os}" ;;
  esac

  info "Detected platform: ${TARGET}"
}

# --- version resolution ---

resolve_version() {
  if [ -n "${KINDLING_VERSION:-}" ]; then
    TAG="${KINDLING_VERSION}"
  else
    command_exists curl || error "curl is required to resolve the latest release"
    info "Resolving the latest release..."
    TAG="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
      | grep '"tag_name"' | head -1 \
      | sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')"
    [ -n "$TAG" ] ||
      error "could not resolve the latest release (set KINDLING_VERSION=vX.Y.Z to pin a version, e.g. a pre-release)"
  fi
  VERSION="${TAG#v}"
}

# --- install ---

cargo_fallback() {
  if command_exists cargo; then
    warn "Falling back to 'cargo install eddacraft-kindling'."
    cargo install eddacraft-kindling
    KINDLING_CMD="$BIN_NAME"
  else
    error "No prebuilt binary for ${TARGET}. Install Rust (https://rustup.rs) and run: cargo install eddacraft-kindling"
  fi
}

install_binary() {
  command_exists curl || error "curl is required to download the binary"

  archive="${BIN_NAME}-${VERSION}-${TARGET}.${EXT}"
  base_url="https://github.com/${REPO}/releases/download/${TAG}"

  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  info "Downloading ${archive} (${TAG})..."
  if ! curl -fsSL --proto '=https' --tlsv1.2 "${base_url}/${archive}" -o "${tmp}/${archive}"; then
    warn "Could not download ${archive}."
    cargo_fallback
    return
  fi

  if curl -fsSL --proto '=https' --tlsv1.2 "${base_url}/${archive}.sha256" -o "${tmp}/${archive}.sha256" 2>/dev/null; then
    info "Verifying checksum..."
    (
      cd "$tmp"
      if command_exists sha256sum; then
        sha256sum -c "${archive}.sha256"
      elif command_exists shasum; then
        shasum -a 256 -c "${archive}.sha256"
      else
        warn "no sha256 tool found; skipping verification"
      fi
    )
  else
    warn "No checksum sidecar found; skipping verification."
  fi

  info "Extracting..."
  case "$EXT" in
    tar.gz) tar -xzf "${tmp}/${archive}" -C "$tmp" ;;
    zip)
      command_exists unzip || error "unzip is required to extract ${archive}"
      unzip -q "${tmp}/${archive}" -d "$tmp"
      ;;
  esac

  bin_file="$BIN_NAME"
  [ "$EXT" = "zip" ] && bin_file="${BIN_NAME}.exe"

  mkdir -p "$INSTALL_DIR"
  if command_exists install; then
    install -m 0755 "${tmp}/${bin_file}" "${INSTALL_DIR}/${bin_file}"
  else
    cp "${tmp}/${bin_file}" "${INSTALL_DIR}/${bin_file}"
    chmod 0755 "${INSTALL_DIR}/${bin_file}"
  fi

  KINDLING_CMD="${INSTALL_DIR}/${bin_file}"
  info "Installed ${BIN_NAME} ${VERSION} to ${KINDLING_CMD}"

  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *) warn "${INSTALL_DIR} is not on your PATH. Add it: export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
  esac
}

# --- setup ---

setup_claude_code() {
  [ -n "$KINDLING_CMD" ] || return

  printf '\n'
  printf '  Set up Claude Code integration?\n'
  printf '  This checks for Claude Code and prints the two /plugin commands\n'
  printf '  to run inside it. It does not edit your Claude Code config.\n'
  printf '\n'
  printf '  Check Claude Code setup? [y/N] '

  answer="n"
  read -r answer </dev/tty || answer="n"

  case "$answer" in
    [yY] | [yY][eE][sS])
      info "Running kindling init --claude-code..."
      "$KINDLING_CMD" init --claude-code
      ;;
    *)
      info "Running kindling init..."
      "$KINDLING_CMD" init
      ;;
  esac
}

offer_demo() {
  [ -n "$KINDLING_CMD" ] || return 0

  case "${KINDLING_NO_DEMO:-}" in
    1 | [yY] | [yY][eE][sS] | [tT][rR][uU][eE]) return 0 ;;
  esac

  # Never block a headless install: without a terminal there is no one to ask.
  has_tty || return 0

  printf '\n'
  printf '  Load sample memory to try search? [Y/n] '

  answer="y"
  read -r answer </dev/tty || answer="y"

  case "$answer" in
    "" | [yY] | [yY][eE][sS])
      info "Running kindling demo..."
      if "$KINDLING_CMD" demo --reset; then
        DEMO_LOADED="yes"
      else
        warn "kindling demo failed; retry later with: kindling demo"
      fi
      ;;
  esac
}

# --- main ---

main() {
  printf '\n'
  printf '  \360\237\224\245 kindling Installer\n'
  printf '  Local memory for AI-assisted development\n'
  printf '\n'

  detect_target
  resolve_version
  install_binary
  setup_claude_code
  offer_demo

  printf '\n'
  printf '  \342\234\223 kindling installed successfully!\n'
  printf '\n'
  printf '  Get started:\n'
  printf '    kindling status          Show database status\n'
  printf '    kindling log "note"      Capture an observation\n'
  printf '    kindling search "query"  Search your memory\n'
  printf '    kindling browse          Explore your memory in a browser\n'
  if [ "$DEMO_LOADED" != "yes" ]; then
    printf '    kindling demo            Load sample memory to try search\n'
  fi
  printf '\n'
  printf '  Documentation: https://docs.eddacraft.ai/kindling/overview\n'
  printf '\n'
}

main
