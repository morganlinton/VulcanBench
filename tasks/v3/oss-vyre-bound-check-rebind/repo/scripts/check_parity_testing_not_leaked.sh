#!/usr/bin/env bash
# P1.10: Ensure `parity-testing` is never enabled outside dev-dependencies.
#
# `WgpuBackend::probe_op` emits raw WGSL that bypasses vyre IR, validation,
# and the conform gate. It exists for the f32 transcendental parity oracle
# and MUST NOT be linked into production binaries. This gate greps every
# Cargo.toml in the workspace for any non-dev declaration of the
# `parity-testing` feature on `vyre-driver-wgpu`.
#
# Exits 0 if clean, non-zero with an actionable message otherwise.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
violations=()

while IFS= read -r -d '' manifest; do
    # Skip the driver-wgpu crate itself (it's allowed to declare the feature)
    # and the vyre-reference dev-dep case (it's the only legitimate consumer).
    crate_name="$(basename "$(dirname "$manifest")")"

    # We parse the manifest line-by-line tracking the current section. The
    # rule: `parity-testing` may only appear inside a section starting with
    # `[dev-dependencies]` or `[target.*.dev-dependencies]`. Any other
    # section enabling it is a violation.
    current_section=""
    while IFS= read -r line; do
        # Strip comments + leading/trailing whitespace.
        trimmed="${line%%#*}"
        trimmed="$(echo "$trimmed" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
        [ -z "$trimmed" ] && continue
        if [[ "$trimmed" =~ ^\[.*\]$ ]]; then
            current_section="$trimmed"
            continue
        fi
        if echo "$trimmed" | grep -q 'parity-testing'; then
            # Allow exactly two cases:
            #   (a) declaration inside vyre-driver-wgpu's [features] block
            #   (b) activation inside any [*dev-dependencies] block
            if [ "$crate_name" = "vyre-driver-wgpu" ] && [[ "$current_section" == "[features]" ]]; then
                continue
            fi
            if [[ "$current_section" =~ dev-dependencies ]]; then
                continue
            fi
            violations+=("$manifest: section '$current_section' enables parity-testing  -  move this to a dev-dependencies block")
        fi
    done <"$manifest"
done < <(find "$repo_root" -name "Cargo.toml" -not -path "*/target/*" -print0)

if [ "${#violations[@]}" -gt 0 ]; then
    echo "parity-testing leak detected:"
    for v in "${violations[@]}"; do
        echo "  $v"
    done
    echo
    echo "Fix: move the feature activation into the crate's [dev-dependencies]"
    echo "     or [target.'cfg(...)'.dev-dependencies] block. Production builds"
    echo "     must never link WgpuBackend::probe_op (bypasses vyre IR + conform)."
    exit 1
fi

echo "parity-testing feature is correctly isolated to dev-dependencies."
