#!/usr/bin/env bash
# P3.4: Enforce the canonical op-naming scheme for vyre-libs.
# See docs/op-naming.md for the rules.
#
# Exits 0 on clean; non-zero with actionable output otherwise.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
libs_root="$repo_root/vyre-libs/src"

if [ ! -d "$libs_root" ]; then
    echo "vyre-libs/src not found at $libs_root" >&2
    exit 1
fi

violations=()

# Banned prefixes and suffixes on public free functions in op modules.
banned_prefix_re='^pub fn (compute|do|run|make|create|new)_[a-z]'
banned_suffix_re='^pub fn [a-z][a-z0-9_]*_(op|impl|internal)\b'
not_snake_case_re='^pub fn [A-Za-z0-9_]*[A-Z][A-Za-z0-9_]*\s*\('

# Only scan op source files; skip module-root, builder helpers, tests.
op_files="$(find "$libs_root" \
    -type f -name "*.rs" \
    ! -name "mod.rs" ! -name "lib.rs" ! -name "builder.rs" \
    ! -name "tensor_ref.rs" ! -name "region.rs" ! -name "descriptor.rs" \
    ! -name "harness.rs" \
    ! -path "*/tests/*")"

for f in $op_files; do
    while IFS= read -r line; do
        if [[ "$line" =~ $banned_prefix_re ]]; then
            violations+=("$f: banned prefix in '$line'")
        fi
        if [[ "$line" =~ $banned_suffix_re ]]; then
            violations+=("$f: banned suffix in '$line'")
        fi
        # Note: we disable the PascalCase check on free fns because
        # Rust already rejects PascalCase free fns with `non_snake_case`
        # at compile time. Keeping the grep as a belt-and-braces
        # defence against `#[allow(non_snake_case)]` slipping in.
        if [[ "$line" =~ $not_snake_case_re ]]; then
            violations+=("$f: non-snake_case fn in '$line'")
        fi
    done <"$f"
done

if [ "${#violations[@]}" -gt 0 ]; then
    echo "vyre-libs op-naming violations:"
    for v in "${violations[@]}"; do
        echo "  $v"
    done
    echo
    echo "Fix: rename per docs/op-naming.md."
    exit 1
fi

echo "vyre-libs op-naming scheme clean."
