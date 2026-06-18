#!/bin/sh
set -e

# kindling installer
# Usage: curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh

PACKAGE="@eddacraft/kindling-cli"
MIN_NODE_MAJOR=20

# --- helpers ---

info()  { printf '\033[1;34m==>\033[0m %s\n' "$1"; }
warn()  { printf '\033[1;33mwarning:\033[0m %s\n' "$1"; }
error() { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }

command_exists() { command -v "$1" >/dev/null 2>&1; }

# --- checks ---

check_node() {
  if ! command_exists node; then
    error "Node.js is not installed. Install Node.js >= ${MIN_NODE_MAJOR} from https://nodejs.org or use a version manager (fnm, nvm, mise)"
  fi

  local version major
  version=$(node --version)
  major=$(printf '%s' "$version" | sed 's/^v//' | cut -d. -f1)

  if [ "$major" -lt "$MIN_NODE_MAJOR" ] 2>/dev/null; then
    error "Node.js ${version} is too old. kindling requires Node.js >= ${MIN_NODE_MAJOR}. Update via https://nodejs.org or your version manager"
  fi

  info "Found Node.js ${version}"
}

detect_package_manager() {
  if command_exists pnpm; then
    printf 'pnpm'
  elif command_exists yarn; then
    printf 'yarn'
  elif command_exists bun; then
    printf 'bun'
  elif command_exists npm; then
    printf 'npm'
  else
    error "No package manager found. Install one of: pnpm, yarn, bun, npm"
  fi
}

# --- install ---

install_kindling() {
  local pm="$1"
  info "Installing ${PACKAGE} with ${pm}..."

  case "$pm" in
    pnpm) pnpm add -g "$PACKAGE" ;;
    yarn) yarn global add "$PACKAGE" ;;
    bun)  bun add -g "$PACKAGE" ;;
    npm)  npm install -g "$PACKAGE" ;;
    *)    error "Unknown package manager: ${pm}" ;;
  esac

  if ! command_exists kindling; then
    warn "kindling command not found in PATH after install"
    warn "You may need to configure your shell for global packages from ${pm}"
  fi
}

# --- setup ---

setup_claude_code() {
  if ! command_exists kindling; then
    warn "Skipping setup: kindling not found in PATH"
    return
  fi

  printf '\n'
  printf '  Configure Claude Code integration?\n'
  printf '  This adds kindling hooks to your Claude Code config.\n'
  printf '\n'
  printf '  Enable Claude Code integration? [y/N] '

  local answer
  read answer </dev/tty || answer="n"

  case "$answer" in
    [yY]|[yY][eE][sS])
      info "Running kindling init --claude-code..."
      kindling init --claude-code
      ;;
    *)
      info "Running kindling init..."
      kindling init
      ;;
  esac
}

# --- main ---

main() {
  printf '\n'
  printf '  \360\237\224\245 kindling Installer\n'
  printf '  Local memory for AI-assisted development\n'
  printf '\n'

  check_node

  local pm
  pm=$(detect_package_manager)
  info "Using package manager: ${pm}"

  install_kindling "$pm"
  setup_claude_code

  printf '\n'
  printf '  \342\234\223 kindling installed successfully!\n'
  printf '\n'
  printf '  Get started:\n'
  printf '    kindling status          Show database status\n'
  printf '    kindling log "note"      Capture an observation\n'
  printf '    kindling search "query"  Search your memory\n'
  printf '\n'
  printf '  Documentation: https://docs.eddacraft.ai/kindling/overview\n'
  printf '\n'
}

main
