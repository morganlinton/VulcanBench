#!/usr/bin/env bash
# Install the wire-ci script as a git pre-push hook on the Santh repo.
#
# Usage:
#   bash libs/performance/matching/vyre/scripts/install_wire_precommit_hook.sh
#
# After install: every `git push` runs scripts/wire_ci_local.sh and the
# push is rejected on the first failed step. Re-run the installer to
# refresh the symlink target after the script moves.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WIRE_CI="${SCRIPT_DIR}/wire_ci_local.sh"

GIT_TOPLEVEL="$(git -C "${SCRIPT_DIR}" rev-parse --show-toplevel)"
HOOK_PATH="${GIT_TOPLEVEL}/.git/hooks/pre-push"

if [ ! -x "${WIRE_CI}" ]; then
    chmod +x "${WIRE_CI}"
fi

if [ -e "${HOOK_PATH}" ] && [ ! -L "${HOOK_PATH}" ]; then
    echo "✘ ${HOOK_PATH} exists and is not a symlink."
    echo "  Back it up or remove it manually before re-running."
    exit 1
fi

ln -sf "${WIRE_CI}" "${HOOK_PATH}"
echo "✓ pre-push hook installed → ${HOOK_PATH}"
echo "  Runs: ${WIRE_CI}"
echo "  Each push blocks if fmt/clippy/check/test fails."
