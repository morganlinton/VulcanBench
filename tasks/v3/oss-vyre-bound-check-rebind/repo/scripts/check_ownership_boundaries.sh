#!/usr/bin/env bash
# Enforce docs/OWNERSHIP.md dependency and concrete-driver isolation boundaries.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ ! -f docs/OWNERSHIP.md ]]; then
    echo "ownership-boundary gate: docs/OWNERSHIP.md missing." >&2
    exit 1
fi

errors=()

check_forbidden_dep() {
    local crate="$1"
    local forbidden_re="$2"
    local cargo="$crate/Cargo.toml"
    if [[ ! -f "$cargo" ]]; then
        errors+=("$crate: missing Cargo.toml")
        return
    fi

    local in_dep_table=0
    local lineno=0
    while IFS= read -r line; do
        lineno=$((lineno + 1))
        if [[ "$line" =~ ^\[(dev-dependencies|build-dependencies|features|\[bench|\[\[bench|\[lib|\[\[bin|target\.[^\]]+\.dev-dependencies) ]]; then
            in_dep_table=0
            continue
        fi
        if [[ "$line" =~ ^\[(dependencies|target\.[^\]]+\.dependencies) ]]; then
            in_dep_table=1
            continue
        fi
        if [[ "$line" =~ ^\[ ]]; then
            in_dep_table=0
            continue
        fi
        if (( in_dep_table )); then
            if [[ "$line" =~ "optional = true" ]]; then
                continue
            fi
            local depname
            depname=$(printf '%s' "$line" | sed -nE 's/^[[:space:]]*([A-Za-z0-9_-]+)[[:space:]]*([.=].*)?$/\1/p')
            if [[ -n "$depname" ]] && [[ "$depname" =~ ^($forbidden_re)$ ]]; then
                errors+=("$crate: forbidden dep '$depname' at $cargo:$lineno")
            fi
        fi
    done < "$cargo"
}

check_forbidden_refs() {
    local crate="$1"
    local forbidden_re="$2"
    [[ -d "$crate" ]] || return 0
    local hits
    hits=$(rg -n "$forbidden_re" \
        "$crate/Cargo.toml" \
        "$crate/src" \
        "$crate/tests" \
        "$crate/README.md" \
        "$crate/ARCHITECTURE.md" \
        "$crate/CONFIG.md" 2>/dev/null || true)
    if [[ -n "$hits" ]]; then
        local filtered_hits
        filtered_hits=$(echo "$hits" | grep -vE 'Cargo.toml.*(optional = true|\[dev-dependencies\]|cuda = |wgpu = |spirv = |#.*requires CUDA|:[0-9]+:[[:space:]]*#)' | grep -vE '\.rs:.*(//|///|//!|/\*)' | grep -vE '(tests|gpu_tests|mod tests|test_)' | grep -vE '\.(md|txt):' | grep -vE '(resolve_family\.rs|any\.rs|vast_builder_oob_guard_regression\.rs|compile\.rs|bundle\.rs|launcher\.rs)' || true)
        if [[ -n "$filtered_hits" ]]; then
            errors+=("$crate: concrete driver reference outside owning driver crate:
$filtered_hits")
        fi
    fi
}

CONCRETE_DRIVER_RE='vyre-driver-wgpu|vyre_driver_wgpu|vyre-driver-cuda|vyre_driver_cuda|vyre-driver-spirv|vyre_driver_spirv|wgpu::|\bWgpu\b|\bCUDA\b|\bcudarc\b|feature = "wgpu"'

check_forbidden_dep vyre-foundation "vyre|vyre-driver|vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|vyre-runtime|vyre-libs|vyre-primitives|vyre-reference|vyre-intrinsics|vyre-aot|vyre-cc|wgpu|naga|cudarc"
check_forbidden_dep vyre-reference "vyre|vyre-driver|vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|vyre-runtime|vyre-libs|wgpu|naga|cudarc"
check_forbidden_dep vyre-driver "vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|wgpu|cudarc"
check_forbidden_dep vyre-driver-wgpu "vyre-driver-spirv|vyre-driver-cuda|cudarc"
check_forbidden_dep vyre-driver-spirv "vyre-driver-wgpu|vyre-driver-cuda|wgpu|cudarc"
check_forbidden_dep vyre-driver-cuda "vyre-driver-wgpu|vyre-driver-spirv|wgpu|naga"
check_forbidden_dep vyre-primitives "vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|vyre-runtime|vyre-libs|vyre-intrinsics|vyre-cc|vyre-aot|wgpu|naga|cudarc"
check_forbidden_dep vyre-libs "vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|vyre-runtime|vyre-cc|vyre-aot|wgpu|naga|cudarc"
check_forbidden_dep vyre-intrinsics "vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|vyre-runtime|vyre-libs|vyre-cc|vyre-aot|wgpu|naga|cudarc"
check_forbidden_dep vyre-aot "vyre-runtime|vyre-libs|vyre-cc|vyre-driver-wgpu|wgpu"
check_forbidden_dep vyre-harness "vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|vyre-runtime|vyre-cc|vyre-aot|vyre-libs|wgpu|naga|cudarc"
check_forbidden_dep vyre-core "vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|vyre-runtime|vyre-libs|vyre-cc|vyre-aot|wgpu|naga|cudarc"
check_forbidden_dep vyre-spec "vyre|vyre-foundation|vyre-driver|vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|vyre-runtime|vyre-libs|vyre-primitives|vyre-reference|vyre-intrinsics|vyre-aot|vyre-cc"
check_forbidden_dep vyre-macros "vyre|vyre-foundation|vyre-driver|vyre-driver-wgpu|vyre-driver-spirv|vyre-driver-cuda|vyre-runtime|vyre-libs|vyre-primitives|vyre-reference|vyre-intrinsics|vyre-aot|vyre-cc|vyre-spec"

for neutral_crate in \
    vyre-core \
    vyre-foundation \
    vyre-driver \
    vyre-primitives \
    vyre-reference \
    vyre-runtime \
    vyre-libs \
    vyre-intrinsics \
    vyre-aot \
    vyre-cc \
    vyre-harness \
    conform/vyre-test-harness \
    conform/vyre-conform-spec \
    conform/vyre-conform-generate \
    conform/vyre-conform-enforce
do
    check_forbidden_refs "$neutral_crate" "$CONCRETE_DRIVER_RE"
done

if (( ${#errors[@]} > 0 )); then
    echo "ownership-boundary gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: keep concrete backend code inside its concrete driver crate and route shared code through vyre-driver contracts." >&2
    exit 1
fi

echo "ownership-boundary gate: every workspace crate respects docs/OWNERSHIP.md."
