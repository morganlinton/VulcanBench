#!/usr/bin/env bash
#
# Layout Law  -  every crate + every dialect + every op has a README.md.
#
# A READMEless directory is a directory nobody can navigate into cold.
# Every published unit of work ships its own README that answers:
# "what does this do, who uses it, how do I extend it?"
#
# Gates checked:
#   1. Every vyre-* crate has README.md.
#   2. Every directory under `vyre-core/src/dialect/` at depth 1
#      (each dialect) has README.md.
#   3. Every op directory under `vyre-core/src/dialect/<name>/`
#      (depth 2) has README.md.
#
# Modes: default warns; VYRE_LAW_STRICT=1 fails.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

STRICT="${VYRE_LAW_STRICT:-0}"
violations=0

CRATES=(vyre-core vyre-foundation vyre-driver vyre-driver-wgpu vyre-driver-cuda vyre-driver-spirv vyre-runtime vyre-reference vyre-primitives vyre-macros vyre-spec vyre-libs vyre-aot vyre-cc)
for crate in "${CRATES[@]}"; do
  if [[ -d "$crate" ]] && [[ ! -f "$crate/README.md" ]]; then
    echo "LAYOUT LAW: missing README.md: $crate" >&2
    violations=$((violations + 1))
  fi
done

if [[ -d "vyre-core/src/dialect" ]]; then
  # Depth 1: stdlib dialect roots.
  while IFS= read -r -d '' dir; do
    if [[ ! -f "$dir/README.md" ]]; then
      echo "LAYOUT LAW: missing README.md in dialect: $dir" >&2
      violations=$((violations + 1))
    fi
  done < <(find vyre-core/src/dialect -mindepth 1 -maxdepth 1 -type d -print0 2>/dev/null || true)

  # Depth 2+: op directories.
  while IFS= read -r -d '' dir; do
    if [[ ! -f "$dir/README.md" ]]; then
      echo "LAYOUT LAW: missing README.md in op: $dir" >&2
      violations=$((violations + 1))
    fi
  done < <(find vyre-core/src/dialect -mindepth 2 -type d -print0 2>/dev/null || true)
fi

if [[ "$violations" -gt 0 ]]; then
  echo '' >&2
  echo "Layout Law: $violations README(s) missing." >&2
  echo "  Fix: write a short (≤ 100 line) README.md per the canonical template." >&2
  if [[ "$STRICT" == "1" ]]; then
    exit 1
  fi
  echo '(informational mode  -  set VYRE_LAW_STRICT=1 to fail the build)' >&2
  exit 0
fi

echo "Layout Law: every tracked directory has a README.md."
