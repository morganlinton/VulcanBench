#!/usr/bin/env bash
# Gap #14  -  CI matrix.
#
# `.github/workflows/ci.yml` must declare a matrix that covers:
#   - os: ubuntu-latest + macos-latest + windows-latest
#   - rust: stable + nightly
# GPU parity is enforced by `.github/workflows/gpu-parity.yml` on the
# self-hosted GPU runner. The generic hosted matrix must not carry a
# no-GPU feature escape hatch.
#
# Fails today because the file does not exist OR does not declare
# the full matrix.

set -euo pipefail
cd "$(dirname "$0")/.."

CI=".github/workflows/ci.yml"
if [ ! -f "$CI" ]; then
    echo "gap #14: $CI does not exist" >&2
    exit 1
fi

FAIL=0
required_os=("ubuntu-latest" "macos-latest" "windows-latest")
for o in "${required_os[@]}"; do
    if ! grep -q "$o" "$CI"; then
        echo "gap #14: $CI missing OS '$o'" >&2
        FAIL=1
    fi
done

required_toolchains=("stable" "nightly")
for t in "${required_toolchains[@]}"; do
    if ! grep -q "$t" "$CI"; then
        echo "gap #14: $CI missing toolchain '$t'" >&2
        FAIL=1
    fi
done

if ! grep -q "matrix:" "$CI"; then
    echo "gap #14: $CI has no 'matrix:' key" >&2
    FAIL=1
fi

if grep -q "no-gpu\\|gpu-feature\\|vyre-driver-wgpu/no-gpu" "$CI"; then
    echo "gap #14: $CI contains a no-GPU escape hatch; GPU parity belongs in gpu-parity.yml" >&2
    FAIL=1
fi

if [ ! -f ".github/workflows/gpu-parity.yml" ]; then
    echo "gap #14: .github/workflows/gpu-parity.yml is missing" >&2
    FAIL=1
fi

exit "$FAIL"
