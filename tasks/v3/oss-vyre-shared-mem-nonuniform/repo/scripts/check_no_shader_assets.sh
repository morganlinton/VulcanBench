#!/usr/bin/env bash
#
# Law B (extended): no shader asset files under src/ops/** or src/dialect/**.
#
# Hand-typed WGSL / SPIR-V / PTX / Metal text is forbidden anywhere an op is
# declared. Every op ships a naga::Module (or equivalent AST) builder in Rust;
# shader text emerges from naga::back::* emitters at build time, never lives
# as a checked-in asset.
#
# This script replaces the .rs-only string-WGSL check for op definitions. The
# underlying Law B text-emission check still applies to vyre-driver-wgpu lowering
# code that *consumes* the naga::Module (via `write_string`)  -  that is the
# only sanctioned text path.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

FORBIDDEN_EXT=(wgsl spv ptx metal msl)
SEARCH_ROOTS=(
  "vyre-core/src/ops"
  "vyre-core/src/dialect"
  "vyre-driver-wgpu/src/ops"
  "vyre-driver-wgpu/src/dialect"
  "vyre-foundation/src/ops"
  "vyre-foundation/src/dialect"
)

violations=0
for root in "${SEARCH_ROOTS[@]}"; do
  if [[ ! -d "$REPO_ROOT/$root" ]]; then
    continue
  fi
  for ext in "${FORBIDDEN_EXT[@]}"; do
    while IFS= read -r -d '' file; do
      echo "LAW B VIOLATION: shader asset file under $root" >&2
      echo "  $file" >&2
      echo "" >&2
      echo "  Hand-typed shader text is forbidden under op / dialect trees." >&2
      echo "  Every op ships a naga::Module builder function in Rust." >&2
      echo "  Shader text comes out of naga::back::<target>::write_string at runtime." >&2
      echo "" >&2
      violations=$((violations + 1))
    done < <(find "$root" -name "*.$ext" -type f -print0 2>/dev/null || true)
  done
done

if [[ "$violations" -gt 0 ]]; then
  echo "Law B (asset extension) failed: $violations shader asset file(s) under op / dialect trees." >&2
  echo "Fix: convert each to a naga::Module builder function and delete the asset." >&2
  exit 1
fi

echo "Law B (asset extension): no shader asset files under src/ops/** or src/dialect/**."
