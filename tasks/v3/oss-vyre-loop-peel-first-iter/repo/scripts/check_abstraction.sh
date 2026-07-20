#!/usr/bin/env bash
set -euo pipefail

source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner
"$CARGO_RUNNER" run -p xtask --bin xtask -- abstraction-gate
