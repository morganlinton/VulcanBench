#!/usr/bin/env bash
# P1 inventory #87  -  max public-module surface gate.
#
# Rule: every workspace crate's `src/lib.rs` declares at most a
# crate-specific number of `pub mod` items. The cap is the current count
# at gate-authoring time; the gate ratchets  -  adding a new top-level
# `pub mod` is treated like a public-API expansion and requires either
# an audit decision (bump the cap) or `pub(crate)` instead.
#
# Why ratchet, not a single global cap: every crate has a legitimately
# different surface (vyre-spec advertises 35 dialect modules; vyre-cc
# only 5). A single number would either rubber-stamp surface drift in
# small crates or block legitimate spec growth. Per-crate ratchets keep
# every crate honest about its own surface.
#
# Usage:
#   scripts/check_max_public_module_surface.sh           # enforce
#   scripts/check_max_public_module_surface.sh --report  # print current counts

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Per-crate caps (highwater at gate-authoring date 2026-04-28). Bumps
# require an explicit reviewer decision  -  treat them like a baseline
# regression bump.
declare -A CAPS
CAPS["vyre-spec"]=37
CAPS["vyre-libs"]=29
CAPS["vyre-foundation"]=19
# `vfs` is a real public primitive-domain surface, not an internal helper.
CAPS["vyre-primitives"]=28
CAPS["vyre-reference"]=12
CAPS["vyre-driver"]=40
CAPS["vyre-driver-wgpu"]=8
CAPS["vyre-runtime"]=7
CAPS["vyre-frontend-c"]=5
CAPS["vyre-aot"]=6
CAPS["vyre-intrinsics"]=4
CAPS["vyre-driver-cuda"]=22
CAPS["vyre-harness"]=2
CAPS["vyre-driver-spirv"]=1
CAPS["vyre-macros"]=0
CAPS["vyre-core"]=1
CAPS["vyre-bench"]=9
CAPS["vyre-conform"]=0
CAPS["vyre-debug"]=9
CAPS["vyre-driver-reference"]=0
CAPS["vyre-emit-naga"]=2
CAPS["vyre-emit-ptx"]=1
CAPS["vyre-emit-spirv"]=1
CAPS["vyre-lints"]=4
CAPS["vyre-lower"]=9
CAPS["vyre-ops"]=0
CAPS["vyre-self-substrate"]=14
CAPS["vyre-std"]=0

mode="${1:-enforce}"

if [[ "$mode" == "--report" ]]; then
    for c in "${!CAPS[@]}"; do
        if [[ -f "$c/src/lib.rs" ]]; then
            cur=$(grep -cE '^pub mod ' "$c/src/lib.rs" || true)
            echo "$cur $c (cap=${CAPS[$c]})"
        fi
    done | sort -rn
    exit 0
fi

violations=()
missing=()

for c in "${!CAPS[@]}"; do
    if [[ ! -f "$c/src/lib.rs" ]]; then
        missing+=("$c")
        continue
    fi
    cur=$(grep -cE '^pub mod ' "$c/src/lib.rs" || true)
    cap=${CAPS[$c]}
    if (( cur > cap )); then
        violations+=("$c: $cur top-level \`pub mod\` (cap=$cap)")
    elif (( cur < cap )); then
        # Downward ratchet  -  surface tightenings (item #70) must be
        # captured by lowering the cap in the same patch so a later
        # `pub mod` re-introduction cannot silently regress the
        # tightened state.
        violations+=("$c: $cur top-level \`pub mod\` is BELOW cap=$cap; tighten cap to $cur to lock the gain")
    fi
done

# Discover crates not in CAPS so the gate fails loudly when a new crate
# is added without an explicit cap decision.
unknown=()
while IFS= read -r lib; do
    crate="$(dirname "$(dirname "$lib")")"
    crate="${crate#./}"
    [[ "$crate" == benches/* || "$crate" == conform/* || "$crate" == "xtask" || "$crate" == "vyre-foundation/fuzz" ]] && continue
    if [[ -z "${CAPS[$crate]:-}" && -z "${CAPS[${crate##*/}]:-}" ]]; then
        unknown+=("$crate")
    fi
done < <(find . -maxdepth 3 -path '*/src/lib.rs' -not -path '*/target/*' | sort)

if (( ${#violations[@]} > 0 || ${#unknown[@]} > 0 )); then
    if (( ${#violations[@]} > 0 )); then
        echo "max-public-module-surface gate: ${#violations[@]} violations." >&2
        for v in "${violations[@]}"; do
            echo "  $v" >&2
        done
    fi
    if (( ${#unknown[@]} > 0 )); then
        echo "Unknown crates without caps:" >&2
        for u in "${unknown[@]}"; do
            echo "  $u" >&2
        done
    fi
    echo >&2
    echo "Fix: prefer \`pub(crate) mod\` for new internal modules. If the" >&2
    echo "module is genuinely public, bump the per-crate cap in" >&2
    echo "scripts/check_max_public_module_surface.sh with explicit rationale." >&2
    exit 1
fi

if (( ${#missing[@]} > 0 )); then
    echo "Note: configured crates without lib.rs:"
    for m in "${missing[@]}"; do
        echo "  $m"
    done
fi

echo "max-public-module-surface gate: every workspace crate within cap."
exit 0
