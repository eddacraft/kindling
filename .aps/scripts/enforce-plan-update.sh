#!/usr/bin/env bash
# APS Plan Update Enforcer — disabled in kindling.
#
# Upstream, this Stop hook blocks session end when code changed but no file
# under plans/ did. kindling does not update APS statuses or plan files on
# feature branches: a separate bookkeeping pass, in its own PR, applies them after work
# merges (see "Picking Up Work" in AGENTS.md). Forcing plan edits into every
# feature branch only creates churn and merge conflicts on the plan files, so
# this hook always allows the session to end.
#
# It stays installed (rather than being removed from the hook wiring) so that
# re-running install-hooks.sh does not quietly bring the old behaviour back.
#
# Exit codes:
#   0 — always

exit 0
