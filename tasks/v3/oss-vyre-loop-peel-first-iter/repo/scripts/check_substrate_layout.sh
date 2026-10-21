#!/usr/bin/env bash
# P0 inventory #67  -  substrate concepts have one canonical home.
#
# The three substrate kinds map to exactly three workspace dirs:
#   substrate       → vyre-primitives/src/<domain>/
#   self-substrate  → vyre-driver/src/self_substrate/
#   pass-substrate  → vyre-foundation/src/pass_substrate/
#
# Any other directory ending in `_substrate` or `substrate` (case-
# insensitive) is a layering smell  -  banned unless the audit row
# `contracts/substrate_layout.md` is updated in the same patch.
#
# Usage:
#   scripts/check_substrate_layout.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DOC="contracts/substrate_layout.md"
[[ -f "$DOC" ]] || { echo "substrate-layout gate: $DOC missing" >&2; exit 1; }

ALLOWED_PATTERNS=(
    "^vyre-primitives/src/"
    "^vyre-driver/src/self_substrate(/.*)?$"
    "^vyre-foundation/src/pass_substrate(/.*)?$"
    # Examples are documentation, not code that creates new substrate homes.
    "^examples/three_substrate_parity(/.*)?$"
    "^vyre-self-substrate(/.*)?$"
    "^vyre-foundation/src/optimizer/fact_substrate(/.*)?$"
)

errors=()

while IFS= read -r dir; do
    [[ -z "$dir" ]] && continue
    rel="${dir#./}"
    is_allowed=0
    for a in "${ALLOWED_PATTERNS[@]}"; do
        if [[ "$rel" =~ $a ]]; then
            is_allowed=1
            break
        fi
    done
    # `vyre-primitives/src/<domain>/` is allowed; the dir itself isn't
    # named *substrate*, so any *substrate* dir under primitives is
    # also a violation.
    if (( is_allowed == 0 )); then
        errors+=("$rel: not on the canonical substrate home list")
    fi
done < <(find . -type d \( -iname '*substrate*' -o -iname '*_substrate' \) \
    -not -path '*/target/*' -not -path '*/target-*/*' -not -path '*/.cargo-target/*' 2>/dev/null)

# Block primitives/src dirs that themselves embed "_substrate" (e.g.
# vyre-primitives/src/foo_substrate/)  -  they leak the substrate
# concept upward and should consolidate into the canonical home.
while IFS= read -r dir; do
    [[ -z "$dir" ]] && continue
    rel="${dir#./}"
    if [[ "$rel" == vyre-primitives/src/*substrate* ]]; then
        errors+=("$rel: vyre-primitives subdomain dir contains 'substrate'  -  collapse into canonical home")
    fi
done < <(find vyre-primitives/src -mindepth 1 -maxdepth 2 -type d 2>/dev/null)

if (( ${#errors[@]} > 0 )); then
    echo "substrate-layout gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: move the directory into one of the three canonical homes" >&2
    echo "(see contracts/substrate_layout.md). Adding a new substrate" >&2
    echo "home requires an explicit audit row + a doc update." >&2
    exit 1
fi

echo "substrate-layout gate: every substrate dir is on the canonical home list."
exit 0
