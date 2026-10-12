#!/usr/bin/env bash
# P0 inventory #72  -  internal-deps-have-versions gate.
#
# Every publishable crate's [dependencies] block on an internal vyre-*
# crate MUST be either:
#   - `crate.workspace = true`, or
#   - `crate = { version = "X", path = "..." }` (path AND version),
# never just `crate = { path = "..." }`. The path-only form blocks
# `./cargo_full publish` because the published crate cannot resolve its
# sibling from crates.io after publication.
#
# The gate skips:
#   - dev-dependencies and build-dependencies (publish doesn't pin those).
#   - members declared `publish = false` (xtask, conform/*, vyre-cc).
#   - vyre-foundation/fuzz (cargo-fuzz nested workspace).
#
# Run before any `./cargo_full publish`. Wired into release signoff.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Publishable crates only (publish = true / unset).
PUBLISHABLE=(
    "vyre-core/Cargo.toml"
    "vyre-foundation/Cargo.toml"
    "vyre-driver/Cargo.toml"
    "vyre-driver-wgpu/Cargo.toml"
    "vyre-driver-spirv/Cargo.toml"
    "vyre-driver-cuda/Cargo.toml"
    "vyre-reference/Cargo.toml"
    "vyre-spec/Cargo.toml"
    "vyre-macros/Cargo.toml"
    "vyre-primitives/Cargo.toml"
    "vyre-runtime/Cargo.toml"
    "vyre-libs/Cargo.toml"
    "vyre-intrinsics/Cargo.toml"
    "vyre-aot/Cargo.toml"
    "vyre-harness/Cargo.toml"
)

INTERNAL_RE='^(vyre|vyre-foundation|vyre-driver|vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|vyre-reference|vyre-spec|vyre-macros|vyre-primitives|vyre-runtime|vyre-libs|vyre-intrinsics|vyre-aot|vyre-harness|vyre-test-harness|vyre-conform-spec|vyre-conform-generate|vyre-conform-enforce|vyre-conform-runner)$'

errors=()

for cargo in "${PUBLISHABLE[@]}"; do
    if [[ ! -f "$cargo" ]]; then
        errors+=("missing $cargo")
        continue
    fi

    # Walk [dependencies] only (skip dev / build / target.*.dev).
    awk -v cargo="$cargo" '
        BEGIN { in_dep = 0 }
        /^\[(dev-dependencies|build-dependencies)\]/ { in_dep = 0; next }
        /^\[target\.[^]]+\.(dev-dependencies|build-dependencies)\]/ { in_dep = 0; next }
        /^\[(dependencies|target\.[^]]+\.dependencies)\]/ { in_dep = 1; next }
        /^\[/ { in_dep = 0; next }
        in_dep && /^[a-zA-Z][a-zA-Z0-9_-]* *=/ {
            line = $0
            depname = $0
            sub(/[ \t]*=.*/, "", depname)
            print cargo "|" NR "|" depname "|" line
        }
    ' "$cargo" | while IFS='|' read -r cargo_path lineno depname line; do
        # Skip non-internal deps.
        if [[ ! "$depname" =~ $INTERNAL_RE ]]; then
            continue
        fi
        # Pass: workspace = true.
        if [[ "$line" =~ \.workspace[[:space:]]*=[[:space:]]*true ]] || [[ "$line" =~ workspace[[:space:]]*=[[:space:]]*true ]]; then
            continue
        fi
        # Pass: path AND version present.
        if [[ "$line" =~ path[[:space:]]*= ]] && [[ "$line" =~ version[[:space:]]*= ]]; then
            continue
        fi
        # Pass: pure version (no path)  -  this is fine, references crates.io.
        if [[ ! "$line" =~ path[[:space:]]*= ]] && [[ "$line" =~ version[[:space:]]*= ]]; then
            continue
        fi
        echo "$cargo_path:$lineno: $depname is path-only (no version)  -  internal deps need both"
    done > /tmp/.tier_check_$$ || true
    while IFS= read -r issue; do
        [[ -z "$issue" ]] && continue
        errors+=("$issue")
    done < /tmp/.tier_check_$$
    rm -f /tmp/.tier_check_$$
done

if (( ${#errors[@]} > 0 )); then
    echo "internal-deps-have-versions gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: every internal vyre-* dep on a publishable crate MUST be" >&2
    echo "either '<crate>.workspace = true' OR '<crate> = { version = \"X\", path = \"...\" }'." >&2
    echo "Path-only form blocks ./cargo_full publish from resolving siblings on crates.io." >&2
    exit 1
fi

echo "internal-deps-have-versions gate: ${#PUBLISHABLE[@]} publishable crates pinned correctly."
exit 0
