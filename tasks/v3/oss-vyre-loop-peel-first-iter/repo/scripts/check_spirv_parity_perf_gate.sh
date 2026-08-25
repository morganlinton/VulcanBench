#!/usr/bin/env bash
# P1 inventory #53  -  SPIR-V parity/performance must be a first-class gate.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

"$CARGO_RUNNER" test -p vyre-driver-spirv --test spirv_parity -- --nocapture
