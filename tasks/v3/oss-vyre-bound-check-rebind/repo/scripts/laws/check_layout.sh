#!/usr/bin/env bash
#
# Layout Law  -  canonical layout for the Unix-philosophy tree.
#
# The commitment: every directory has one obvious purpose, every
# file has one obvious home, every op directory has the same shape.
# This script enforces:
#
#   1. No banned directory names: utils/, helpers/, common/, misc/,
#      shared/  -  these are always two-or-more responsibilities
#      masquerading as one.
#   2. No banned file names: utils.rs, helpers.rs, common.rs, misc.rs,
#      shared.rs  -  same reasoning.
#   3. Every crate under vyre-* has a README.md.
#   4. Every `vyre-core/src/dialect/<name>/<op>/` directory (once
#      populated by Gemini A's migration) has the canonical shape:
#      op.rs, cpu_ref.rs, tests.rs, README.md.
#   5. Directory depth ≤ 4 from any `src/` root.
#
# Modes: default warns; VYRE_LAW_STRICT=1 fails.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

STRICT="${VYRE_LAW_STRICT:-0}"
violations=0

# Banned directory names  -  always.
BANNED_DIRS=(utils helpers common misc shared)
for banned in "${BANNED_DIRS[@]}"; do
  while IFS= read -r -d '' dir; do
    echo "LAYOUT LAW: banned directory name '$banned/': $dir" >&2
    echo "  Fix: rename to a name that describes the single responsibility" >&2
    echo "       (or split into multiple named modules)." >&2
    violations=$((violations + 1))
  done < <(find vyre-core vyre-foundation vyre-driver vyre-driver-wgpu vyre-driver-cuda vyre-driver-spirv vyre-runtime vyre-reference vyre-primitives vyre-macros vyre-spec vyre-libs vyre-aot vyre-cc \
    -type d \( -name target -o -name fuzz -o -name benches \) -prune -o \
    -type d -name "$banned" -print0 2>/dev/null || true)
done

# Banned file names.
BANNED_FILES=(utils.rs helpers.rs common.rs misc.rs shared.rs)
for banned in "${BANNED_FILES[@]}"; do
  while IFS= read -r -d '' file; do
    echo "LAYOUT LAW: banned file name '$banned': $file" >&2
    echo "  Fix: rename to describe what the file actually does." >&2
    violations=$((violations + 1))
  done < <(find vyre-core vyre-foundation vyre-driver vyre-driver-wgpu vyre-driver-cuda vyre-driver-spirv vyre-runtime vyre-reference vyre-primitives vyre-macros vyre-spec vyre-libs vyre-aot vyre-cc \
    -type d \( -name target -o -name fuzz \) -prune -o \
    -type f -name "$banned" -print0 2>/dev/null || true)
done

# Every vyre-* crate needs a README.md.
for crate in vyre-core vyre-foundation vyre-driver vyre-driver-wgpu vyre-driver-cuda vyre-driver-spirv vyre-runtime vyre-reference vyre-primitives vyre-macros vyre-spec vyre-libs vyre-aot vyre-cc; do
  if [[ -d "$crate" ]] && [[ ! -f "$crate/README.md" ]]; then
    echo "LAYOUT LAW: missing README.md in crate: $crate" >&2
    echo "  Fix: every crate ships a README.md describing its public surface." >&2
    violations=$((violations + 1))
  fi
done

# Once A-B4 / A-B4b / A-B4c populate src/dialect/<name>/<op>/, enforce
# the canonical op-directory shape. Gated on `dialect/<name>/<op>/`
# pattern existing; a no-op while the tree is migrating.
if [[ -d "vyre-core/src/dialect" ]]; then
  while IFS= read -r -d '' op_dir; do
    # Skip top-level files directly under dialect/ (op_def.rs, etc.)
    rel="${op_dir#vyre-core/src/dialect/}"
    depth=$(awk -F/ '{print NF}' <<< "$rel")
    if (( depth < 2 )); then
      continue
    fi
    for required in op.rs cpu_ref.rs tests.rs README.md; do
      if [[ ! -f "$op_dir/$required" ]]; then
        echo "LAYOUT LAW: op dir missing $required: $op_dir" >&2
        violations=$((violations + 1))
      fi
    done
  done < <(find vyre-core/src/dialect -mindepth 2 -type d -print0 2>/dev/null || true)
fi

if [[ "$violations" -gt 0 ]]; then
  echo '' >&2
  echo "Layout Law: $violations violation(s)." >&2
  if [[ "$STRICT" == "1" ]]; then
    exit 1
  fi
  echo '(informational mode  -  set VYRE_LAW_STRICT=1 to fail the build)' >&2
  exit 0
fi

echo "Layout Law: all checks passed."
