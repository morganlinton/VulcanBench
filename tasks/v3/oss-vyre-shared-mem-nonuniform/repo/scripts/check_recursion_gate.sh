#!/usr/bin/env bash
# Enforce the recursion thesis from the release signoff path.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export RUSTC_WRAPPER=""
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" run -q -p xtask --bin xtask -- recursion-gate --strict
