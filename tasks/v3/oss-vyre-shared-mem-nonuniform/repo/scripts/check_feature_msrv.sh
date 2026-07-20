#!/usr/bin/env bash
# P1 inventory #110  -  feature-MSRV gate.
#
# For every advertised feature combination across the workspace, ensure
# `cargo_full check --features <combo>` passes on the workspace MSRV
# (`rust-toolchain.toml` / `[workspace.package].rust-version`).
#
# The gate runs a small, opinionated matrix:
#   - default features per crate (always)  -  implicit, runs in main CI.
#   - explicit feature combinations the workspace docs advertise:
#       vyre-libs       : math, nn, matching, crypto, decode (each alone)
#       vyre-primitives : default, all-lego
#       vyre-runtime    : default, remote-cache, uring-cmd-nvme
#       vyre-aot        : ptx, spirv
#       vyre-driver-cuda: default
#       vyre-driver-spirv: default
#
# Why per-crate matrix instead of a global all-features run: feature
# unification at the workspace level masks broken individual feature
# sets (a feature passes because something else turns its prerequisites
# on). The per-crate run pins each combination on its own.
#
# The gate is OPT-IN by default (it shells out to cargo_full). Run it
# locally before publishing or with `--ci` in CI. Without `--ci`, the
# gate emits the matrix and exits 0 so the release signoff stays
# fast.
#
# Usage:
#   scripts/check_feature_msrv.sh           # list matrix, exit 0
#   scripts/check_feature_msrv.sh --ci      # run cargo_full check on each combo

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

MSRV="$(grep -E '^rust-version' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"
if [[ -z "$MSRV" ]]; then
    echo "feature-MSRV gate: cannot determine MSRV from Cargo.toml." >&2
    exit 1
fi

# matrix entries: "<crate>|<feature-spec>"
# An empty <feature-spec> means default features.
MATRIX=(
    "vyre-libs|"
    "vyre-libs|--no-default-features"
    "vyre-libs|--no-default-features --features math"
    "vyre-libs|--no-default-features --features nn"
    "vyre-libs|--no-default-features --features matching"
    "vyre-libs|--no-default-features --features crypto"
    "vyre-libs|--no-default-features --features decode"
    "vyre-libs|--no-default-features --features c-parser"
    "vyre-primitives|"
    "vyre-primitives|--features all-lego"
    "vyre-primitives|--no-default-features --features bitset"
    "vyre-primitives|--no-default-features --features reduce"
    "vyre-runtime|"
    "vyre-runtime|--features remote-cache"
    "vyre-runtime|--features uring-cmd-nvme"
    "vyre-aot|--no-default-features --features ptx"
    "vyre-aot|--no-default-features --features spirv"
    "vyre-driver-cuda|"
    "vyre-driver-spirv|"
)

mode="${1:-list}"

if [[ "$mode" == "list" ]]; then
    echo "feature-MSRV gate: workspace MSRV = $MSRV"
    echo "Matrix (run with --ci to execute cargo_full check on each):"
    for entry in "${MATRIX[@]}"; do
        IFS='|' read -r crate spec <<< "$entry"
        echo "  - $crate $spec"
    done
    echo
    echo "Note: --ci runs cargo_full check serially against the workspace MSRV"
    echo "and is intended for the publish-readiness sweep, not the release"
    echo "signoff fast path."
    exit 0
fi

if [[ "$mode" != "--ci" ]]; then
    echo "Unknown mode: $mode (use --ci or no args)" >&2
    exit 2
fi

failed=()
for entry in "${MATRIX[@]}"; do
    IFS='|' read -r crate spec <<< "$entry"
    label="$crate ${spec:-(default)}"
    echo "▶ $label"
    if ! "$CARGO_RUNNER" +"$MSRV" check -p "$crate" $spec 2>&1 | tail -3; then
        failed+=("$label")
    fi
done

if (( ${#failed[@]} > 0 )); then
    echo "feature-MSRV gate: ${#failed[@]} matrix entries failed on $MSRV." >&2
    for f in "${failed[@]}"; do
        echo "  ✗ $f" >&2
    done
    exit 1
fi
echo "feature-MSRV gate: all ${#MATRIX[@]} matrix entries pass on $MSRV."
exit 0
