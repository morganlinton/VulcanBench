#!/usr/bin/env bash
# Wave 1.1  -  P0 performance inventory contract tests.
# See audits/WAVE_EXECUTION.md
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

echo "==> wave1.1: vyre-foundation optimizer reference parity (P0 #34 seed)"
"$CARGO_RUNNER" test -p vyre-foundation --test optimizer_reference_parity_smoke

echo "==> wave1.1: vyre-driver-wgpu dispatch allocation contracts (P0 #10)"
"$CARGO_RUNNER" test -p vyre-driver-wgpu --test dispatch_allocation_contract

echo "==> performance inventory wave 1.1: OK"
