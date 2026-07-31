#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

matches="$(
  rg -n 'program\.clone\(\)|Program::clone|clone_program' \
    vyre-foundation/src/optimizer \
    vyre-foundation/src/transform \
    vyre-foundation/src/lower \
    vyre-runtime/src/pipeline_cache.rs \
    vyre-driver-wgpu/src/lowering \
    vyre-driver-wgpu/src/pipeline.rs \
    --glob '*.rs' --glob '!*_tests.rs' || true
)"

if [[ -n "$matches" ]]; then
  echo "$matches"
  echo "Fix: remove Program::clone from optimizer/cache/lowering hot paths; use borrowed/COW rewrites or explicit test-only fixtures." >&2
  exit 1
fi

echo "Program clone hot-path check: 0 forbidden clones."
