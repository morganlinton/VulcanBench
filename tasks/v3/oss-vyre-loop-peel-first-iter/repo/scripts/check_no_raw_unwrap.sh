#!/usr/bin/env bash
# Law E enforcement: no raw unwrap/expect in production Rust.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_no_raw_unwrap.py "$ROOT"
