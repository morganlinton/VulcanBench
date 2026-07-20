#!/usr/bin/env bash
# Installs `cargo xtask lego-quick` as the repository pre-commit hook.
#
# Idempotent: re-running replaces the hook with the current contents.
# Wraps the xtask invocation in a brief preamble so the writer sees
# what is being checked and can recover (e.g. `git commit --no-verify`
# with explicit user approval) when the gate fires for legitimate
# reasons. Default behavior is to BLOCK the commit on any finding.

set -euo pipefail
cd "$(dirname "$0")/.."

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$REPO_ROOT" ]; then
    echo "Fix: install_lego_quick_hook.sh must run inside a git checkout" >&2
    exit 1
fi
HOOKS_DIR="$REPO_ROOT/.git/hooks"
mkdir -p "$HOOKS_DIR"
HOOK="$HOOKS_DIR/pre-commit"

printf '%s\n' \
    '#!/usr/bin/env bash' \
    '# Installed by libs/performance/matching/vyre/scripts/install_lego_quick_hook.sh' \
    '# Runs `./cargo_full run --bin xtask -- lego-quick` over the staged diff.' \
    'set -euo pipefail' \
    'REPO_ROOT="$(git rev-parse --show-toplevel)"' \
    'VYRE_DIR="$REPO_ROOT/libs/performance/matching/vyre"' \
    'if [ ! -d "$VYRE_DIR" ]; then' \
    "    # Hook installed in a checkout that doesn't have the vyre tree at the" \
    "    # expected path. Skip silently so the hook isn't user-hostile in" \
    '    # multi-repo workspaces.' \
    '    exit 0' \
    'fi' \
    'cd "$VYRE_DIR"' \
    './cargo_full run --bin xtask --quiet --release -- lego-quick' \
    >"$HOOK"

chmod +x "$HOOK"
echo "installed pre-commit hook at $HOOK"
