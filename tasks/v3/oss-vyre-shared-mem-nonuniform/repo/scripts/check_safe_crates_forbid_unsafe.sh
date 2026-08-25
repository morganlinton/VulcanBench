#!/usr/bin/env bash
# Audit-B  -  every safe crate must `#![forbid(unsafe_code)]`.
#
# Rule: a crate with zero `unsafe fn`, `unsafe impl`, `unsafe trait`,
# `unsafe extern`, or `unsafe { ... }` block in its `src/` tree MUST
# declare `#![forbid(unsafe_code)]` at the crate root. `deny` is too
# weak  -  `deny` can be locally overridden with `#[allow(unsafe_code)]`,
# `forbid` cannot.
#
# Crates with documented unsafe usage (FFI, arena lifetimes, io_uring
# bindings) are explicitly allow-listed below with the rationale that
# justifies the `#![allow(unsafe_code)]` policy. Adding a new entry
# requires the same reviewer scrutiny as bumping any other ratchet.
#
# Usage:
#   scripts/check_safe_crates_forbid_unsafe.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Crates with sanctioned unsafe usage. Each entry pins both the crate
# directory and the policy reason for the audit trail.
declare -A UNSAFE_ALLOWED
UNSAFE_ALLOWED["vyre-foundation"]="IR arena lifetime extension in ir_inner::model::arena (2 sites)."
UNSAFE_ALLOWED["vyre-driver-wgpu"]="wgpu FFI helpers; deny + per-site allow is the policy."
UNSAFE_ALLOWED["vyre-driver-cuda"]="cudarc CUDA driver FFI is inherently unsafe (32 sites)."
UNSAFE_ALLOWED["vyre-runtime"]="io_uring zero-copy ingest FFI + persistent ring (48 sites)."
UNSAFE_ALLOWED["vyre-driver-spirv"]="FFI Vulkan/SPIR-V cross compiler integration (47 sites)."

# Workspace lib crates the gate scans. Excludes binaries-only crates.
CRATES=(
    "vyre-core"
    "vyre-foundation"
    "vyre-driver"
    "vyre-driver-wgpu"
    "vyre-driver-spirv"
    "vyre-driver-cuda"
    "vyre-reference"
    "vyre-spec"
    "vyre-macros"
    "vyre-primitives"
    "vyre-runtime"
    "vyre-libs"
    "vyre-intrinsics"
    "vyre-frontend-c"
    "vyre-aot"
    "vyre-harness"
)

errors=()

for crate in "${CRATES[@]}"; do
    src="$crate/src"
    [[ ! -d "$src" ]] && { errors+=("$crate: missing src/"); continue; }

    ucount=$( { grep -rE 'unsafe[[:space:]]+(fn|impl|trait|\{|extern)' "$src" 2>/dev/null || true; } | wc -l | tr -d ' ')
    policy=$(grep -E '#!\[(forbid|deny|allow)\(unsafe_code' "$crate/src/lib.rs" 2>/dev/null | head -1 || true)

    if (( ucount == 0 )); then
        # Safe crate  -  must forbid.
        if [[ ! "$policy" =~ forbid\(unsafe_code ]]; then
            errors+=("$crate: 0 unsafe sites but policy is '$policy' (require #![forbid(unsafe_code)])")
        fi
    else
        # Crate uses unsafe  -  must be on the allow-list AND must NOT forbid.
        if [[ -z "${UNSAFE_ALLOWED[$crate]:-}" ]]; then
            errors+=("$crate: $ucount unsafe sites but not on the allow-list (add a documented entry)")
        fi
        if [[ "$policy" =~ forbid\(unsafe_code ]]; then
            errors+=("$crate: $ucount unsafe sites but policy is forbid; the build won't compile")
        fi
    fi
done

if (( ${#errors[@]} > 0 )); then
    echo "safe-crates-forbid-unsafe gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: a crate with no unsafe usage must declare" >&2
    echo "#![forbid(unsafe_code)]. A crate with documented unsafe must be" >&2
    echo "on the allow-list in scripts/check_safe_crates_forbid_unsafe.sh" >&2
    echo "with the policy reason written next to it." >&2
    exit 1
fi

echo "safe-crates-forbid-unsafe gate: ${#CRATES[@]} crates classified."
exit 0
