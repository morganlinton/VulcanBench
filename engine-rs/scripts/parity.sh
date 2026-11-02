#!/usr/bin/env bash
# Acceptance test for the Rust engine spike: both engines on hello-world,
# normalised artifacts diffed. See parity.py for the normalisation rules.
set -euo pipefail
exec python3 "$(dirname "$0")/parity.py" "$@"
