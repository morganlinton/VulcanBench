#!/usr/bin/env bash
# Audit-J  -  standardize lib.rs headers across every workspace crate.
#
# Requires every workspace crate's `src/lib.rs` to declare:
#   1. A crate-level doc comment (`//! ...`).
#   2. An explicit unsafe-code policy: exactly one of
#      `#![forbid(unsafe_code)]`, `#![deny(unsafe_code)]`, or
#      `#![allow(unsafe_code)]`. Inheriting the workspace lint is fine,
#      but the lib.rs must restate the choice so a reader can tell the
#      policy from the file alone (`forbid` is preferred; `allow` is
#      reserved for crates with documented FFI / driver bindings).
#
# `missing_docs` is enforced at the workspace lint floor
# (`[workspace.lints.rust] missing_docs = "deny"`) and inherited through
# `[lints] workspace = true` on every member, so this gate does NOT
# re-check it. The metadata-normalization gate keeps that inheritance
# in place.
#
# Usage:
#   scripts/check_lib_rs_headers.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

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
    "conform/vyre-conform-spec"
    "conform/vyre-conform-generate"
    "conform/vyre-conform-enforce"
    "conform/vyre-conform-runner"
    "conform/vyre-test-harness"
)

errors=()

for crate in "${CRATES[@]}"; do
    lib="$crate/src/lib.rs"
    if [[ ! -f "$lib" ]]; then
        # Some conform members are bin-only (vyre-conform-runner has src/main.rs).
        if [[ -f "$crate/src/main.rs" ]]; then
            lib="$crate/src/main.rs"
        else
            errors+=("$crate: missing src/lib.rs and src/main.rs")
            continue
        fi
    fi
    head_block=$(head -120 "$lib")
    if ! grep -qE '^//!' <<< "$head_block"; then
        errors+=("$crate ($lib): missing crate-level doc comment (//!)")
    fi
    unsafe_lines=$(grep -cE '^#!\[(forbid|deny|allow)\(unsafe_code\)' <<< "$head_block" || true)
    if (( unsafe_lines == 0 )); then
        errors+=("$crate ($lib): missing explicit unsafe-code policy (#![forbid(unsafe_code)] preferred)")
    elif (( unsafe_lines > 1 )); then
        errors+=("$crate ($lib): multiple unsafe-code policy lines; pick exactly one")
    fi
done

if (( ${#errors[@]} > 0 )); then
    echo "lib-rs-headers gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: every src/lib.rs starts with a //! crate-doc-comment and" >&2
    echo "exactly one of #![forbid(unsafe_code)] / #![deny(unsafe_code)] /" >&2
    echo "#![allow(unsafe_code)]. Prefer forbid; allow only for documented" >&2
    echo "FFI/driver crates (e.g. vyre-driver-cuda)." >&2
    exit 1
fi

echo "lib-rs-headers gate: ${#CRATES[@]} crates conform."
exit 0
