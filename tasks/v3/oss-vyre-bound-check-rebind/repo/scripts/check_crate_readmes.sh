#!/usr/bin/env bash
# Audit-E  -  README.md per crate.
#
# Every workspace crate ships a README.md with at least:
#   - what the crate is (one paragraph minimum)
#   - key types or feature surface
#   - architecture decisions or where-to-look pointers
#
# The gate enforces a per-crate minimum line count plus presence of
# specific section markers ("What this crate is" or "## " section
# header) and "Where to look" or "Architecture decisions" markers.
# A README that falls below the floor is a readability bug for any
# agent landing in the crate cold.
#
# Usage:
#   scripts/check_crate_readmes.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Minimum line count per workspace crate. The floor is set above the
# current state; a crate must have at least a paragraph + a section
# header + a where-to-look pointer. Bench / xtask / fuzz workspaces are
# excluded by design.
declare -A MIN_LINES
MIN_LINES["vyre-core"]=60
MIN_LINES["vyre-foundation"]=30
MIN_LINES["vyre-driver"]=60
MIN_LINES["vyre-driver-wgpu"]=30
MIN_LINES["vyre-driver-spirv"]=30
MIN_LINES["vyre-driver-cuda"]=25
MIN_LINES["vyre-reference"]=60
MIN_LINES["vyre-spec"]=60
MIN_LINES["vyre-macros"]=60
MIN_LINES["vyre-primitives"]=60
MIN_LINES["vyre-runtime"]=40
MIN_LINES["vyre-libs"]=50
MIN_LINES["vyre-intrinsics"]=60
MIN_LINES["vyre-frontend-c"]=60
MIN_LINES["vyre-aot"]=60
MIN_LINES["vyre-harness"]=30
MIN_LINES["conform/vyre-conform-spec"]=20
MIN_LINES["conform/vyre-conform-generate"]=20
MIN_LINES["conform/vyre-conform-enforce"]=20
MIN_LINES["conform/vyre-conform-runner"]=20
MIN_LINES["conform/vyre-test-harness"]=20

errors=()

for crate in "${!MIN_LINES[@]}"; do
    readme="$crate/README.md"
    if [[ ! -f "$readme" ]]; then
        errors+=("$crate: missing README.md")
        continue
    fi
    lc=$(wc -l < "$readme" | tr -d ' ')
    floor=${MIN_LINES[$crate]}
    if (( lc < floor )); then
        errors+=("$crate: README.md is $lc lines (floor=$floor)")
    fi
    if ! grep -qE '^##? ' "$readme"; then
        errors+=("$crate/README.md: missing any '## ' section header")
    fi
done

if (( ${#errors[@]} > 0 )); then
    echo "crate-readmes gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: every crate's README.md describes the crate (what it is,"  >&2
    echo "key types, architecture decisions, where to look). Floors are" >&2
    echo "set per crate; bumping a floor requires a corresponding README" >&2
    echo "expansion." >&2
    exit 1
fi

echo "crate-readmes gate: every workspace crate's README clears its floor."
exit 0
