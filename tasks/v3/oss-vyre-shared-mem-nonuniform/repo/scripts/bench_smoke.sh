#!/usr/bin/env bash
set -euo pipefail

# Run canonical vyre-bench smoke cases.
cd "$(dirname "$0")/.."
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner
"$CARGO_RUNNER" run -p vyre-bench -- run --suite smoke --format json "$@"
