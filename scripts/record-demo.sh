#!/usr/bin/env sh
# record-demo.sh: run a kindling demo session suitable for terminal recording.
#
# Usage:
#   ./scripts/record-demo.sh
#
# Records well with asciinema:
#   asciinema rec -c "./scripts/record-demo.sh" kindling-demo.cast
#
# POSIX sh, shellcheck-clean.

set -eu

DEMO_DB="${KINDLING_DEMO_DB:-${HOME}/.kindling/demo/kindling.db}"

info()  { printf '\033[1;34m==>\033[0m %s\n' "$1"; }
warn()  { printf '\033[1;33mwarning:\033[0m %s\n' "$1"; }
error() { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }

command_exists() { command -v "$1" >/dev/null 2>&1; }

# --- install check ---

info "Checking for kindling binary..."

if command_exists kindling; then
  info "Found: $(command -v kindling)"
  kindling --version || true
else
  warn "kindling is not on PATH."
  if [ -f "./install.sh" ]; then
    info "Run the installer first:"
    printf '  curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh\n'
    error "kindling not installed"
  else
    error "kindling not installed. See docs/quickstart/without-claude-code.md"
  fi
fi

# --- demo flow ---

printf '\n'
info "Loading demo memory (sample observations, capsules, pins)..."
kindling demo

printf '\n'
info 'Searching demo memory for "JWT"...'
kindling search "JWT" --db "$DEMO_DB"

printf '\n'
info "Exporting demo memory to a local HTML viewer..."
kindling browse --db "$DEMO_DB" --no-open

# --- recording instructions ---

cat <<'EOF'

Recording with asciinema
------------------------

Install asciinema (https://asciinema.org/):

  # macOS
  brew install asciinema

  # Debian/Ubuntu
  sudo apt install asciinema

Record this demo (from the repo root):

  asciinema rec -c "./scripts/record-demo.sh" kindling-demo.cast

Replay locally:

  asciinema play kindling-demo.cast

Upload to asciinema.org:

  asciinema upload kindling-demo.cast

Tips:
  - Use a terminal at least 100 columns wide.
  - Run `kindling demo --reset` first if you want a clean dataset.
  - Set KINDLING_DEMO_DB to override the default demo database path.

EOF