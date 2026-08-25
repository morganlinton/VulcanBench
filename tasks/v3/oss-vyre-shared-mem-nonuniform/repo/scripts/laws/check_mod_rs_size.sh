#!/usr/bin/env bash
#
# Layout Law  -  mod.rs size cap.
#
# `mod.rs` (and the `<name>.rs` mirror-files the vyre convention
# uses) must be trivial: `pub mod X;` declarations + re-exports only.
# ≤ 80 lines total. If a mod.rs grows real logic, move that logic
# into a named sub-module and leave mod.rs as pure declarations.
#
# Modes: default warns; VYRE_LAW_STRICT=1 fails.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

MAX_LINES=80
STRICT="${VYRE_LAW_STRICT:-0}"

violations=()
while IFS= read -r -d '' file; do
  lines=$(wc -l < "$file")
  if (( lines > MAX_LINES )); then
    violations+=("$lines $file")
  fi
done < <(find vyre-core vyre-foundation vyre-driver vyre-driver-wgpu vyre-driver-cuda vyre-driver-spirv vyre-runtime vyre-reference vyre-primitives vyre-macros vyre-spec vyre-libs vyre-aot vyre-cc \
  -type d \( -name target -o -name fuzz \) -prune -o \
  -type f -name "mod.rs" -print0 2>/dev/null || true)

if [[ ${#violations[@]} -gt 0 ]]; then
  printf 'Layout Law: %d mod.rs file(s) exceed %d lines:\n' \
    "${#violations[@]}" "$MAX_LINES" >&2
  printf '%s\n' "${violations[@]}" | sort -rn | head -20 >&2
  printf '\n  Fix: mod.rs must contain `pub mod ...;` declarations and re-exports only.\n' >&2
  printf '       Move real logic into a named sub-module.\n' >&2
  if [[ "$STRICT" == "1" ]]; then
    exit 1
  fi
  echo '(informational mode  -  set VYRE_LAW_STRICT=1 to fail the build)' >&2
  exit 0
fi

echo "Layout Law: every mod.rs ≤ ${MAX_LINES} lines."
