#!/usr/bin/env bash
# P3.13: Every `unsafe` site in the workspace is on a pre-approved
# whitelist. New `unsafe fn`, `unsafe impl`, or `unsafe { … }` blocks
# outside the whitelist fail CI  -  forcing a deliberate review of
# every new unsafe site.
#
# Rationale: the 0.6 contract promises `#![forbid(unsafe_code)]` on
# the public Cat-A surface (`vyre-libs`) and audited `unsafe` only
# inside the driver + io_uring layers. Silent unsafe creep breaks
# the audit contract.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

# Whitelisted crates. Each entry is an absolute path fragment; any
# `unsafe` block under one of these paths is allowed. Every path
# here must have a corresponding SAFETY comment at the site (a
# stronger check than this gate; a separate lint enforces it).
whitelist=(
    "/vyre-pipeline/src/uring/"
    "/vyre-pipeline/src/lib.rs"
    "/vyre-foundation/src/ir_inner/model/arena.rs"
    "/vyre-driver-wgpu/src/runtime/shader/compile_compute_pipeline.rs"
    "/vyre-driver-wgpu/src/lib.rs"
    "/vyre-pipeline/tests/"
    "/xtask/"
    "/vyre-driver-wgpu/src/runtime/streaming_io_uring"
    "/conform/"
)

is_whitelisted() {
    local file="$1"
    for entry in "${whitelist[@]}"; do
        if [[ "$file" == *"$entry"* ]]; then
            return 0
        fi
    done
    return 1
}

violations=()

# Scan every .rs in the workspace that isn't under target/.
# Skip `//` comment lines before matching, so a `// mention of
# unsafe impl` in prose doesn't trigger the gate.
while IFS= read -r -d '' file; do
    if ! grep -v '^\s*//' "$file" \
        | grep -qE '(^|[^a-zA-Z_])unsafe\s+(impl|fn|\{)' 2>/dev/null; then
        continue
    fi
    if is_whitelisted "$file"; then
        continue
    fi
    violations+=("$file")
done < <(find "$repo_root" -type f -name "*.rs" -not -path "*/target/*" -print0)

if [ "${#violations[@]}" -gt 0 ]; then
    echo "Unsafe-code budget exceeded  -  new unsafe in:"
    for v in "${violations[@]}"; do
        echo "  $v"
    done
    echo
    echo "Fix: either (a) remove the unsafe, (b) wrap it in a safe"
    echo "     abstraction inside an already-whitelisted crate, or"
    echo "     (c) add a whitelist entry in scripts/check_unsafe_budget.sh"
    echo "     after a security review. Every site must have a SAFETY"
    echo "     comment naming the invariant the caller relies on."
    exit 1
fi

echo "Unsafe-code budget: no new unsafe sites outside the whitelist."
