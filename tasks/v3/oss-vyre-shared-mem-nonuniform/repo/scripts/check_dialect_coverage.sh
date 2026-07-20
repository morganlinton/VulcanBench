#!/usr/bin/env bash
#
# Dialect coverage law: every `Category::Intrinsic` (intrinsic) op must declare at
# least one backend lowering in its `LoweringTable` (naga_wgsl | naga_spv |
# ptx | metal_ir). Category::Intrinsic ops lower through composition inlining via the
# DialectRegistry + OpDef::compose pipeline  -  a direct backend lowering is
# optional for them.
#
# A Cat C op with no backend lowering is a silent stub: it registers, it
# passes validation, and it fails at dispatch with a "backend doesn't support
# this op" error that lies about the cause. The op was never implemented, not
# unsupported.
#
# This script scans every OpDef instantiation under the dialect tree and
# fails the PR for any Cat C OpDef whose LoweringTable has no `Some(_)`
# backend-lowering field.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

SEARCH_ROOTS=(
  "vyre-core/src/dialect"
)
for dir in vyre-dialect-*; do
  [[ -d "$dir" ]] && SEARCH_ROOTS+=("$dir/src")
done

violations=0
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

for root in "${SEARCH_ROOTS[@]}"; do
  if [[ ! -d "$REPO_ROOT/$root" ]]; then
    continue
  fi
  while IFS= read -r -d '' file; do
    case "$file" in
      */dialect/lowering.rs) continue ;;
      */dialect/op_def.rs) continue ;;
    esac
    awk '
      /OpDef[[:space:]]*\{/ && $0 !~ /pub[[:space:]]+struct/ {
        in_opdef = 1
        opdef_start = NR
        opdef_text = ""
        depth = 0
      }
      in_opdef {
        opdef_text = opdef_text $0 "\n"
        for (i = 1; i <= length($0); i++) {
          c = substr($0, i, 1)
          if (c == "{") depth++
          else if (c == "}") {
            depth--
            if (depth == 0) {
              in_opdef = 0
              is_cat_c = (opdef_text ~ /category[[:space:]]*:[[:space:]]*Category::Intrinsic\b/)
              has_backend = (opdef_text ~ /(naga_wgsl|naga_spv|ptx|metal_ir|metal)[[:space:]]*:[[:space:]]*Some[[:space:]]*\(/)
              if (is_cat_c && !has_backend) {
                print FILENAME ":" opdef_start ":" opdef_text
              }
              opdef_text = ""
            }
          }
        }
      }
    ' "$file" >> "$tmp"
  done < <(find "$root" -name "*.rs" -type f -print0 2>/dev/null || true)
done

if [[ -s "$tmp" ]]; then
  echo "DIALECT COVERAGE VIOLATION: one or more Category::Intrinsic OpDef entries have no backend lowering." >&2
  echo "" >&2
  cat "$tmp" >&2
  echo "" >&2
  echo "Fix: populate at least one of naga_wgsl / naga_spv / ptx / metal on every Cat C LoweringTable." >&2
  echo "     Category::Intrinsic ops may inline through composition  -  Cat C intrinsics must declare a backend." >&2
  exit 1
fi

echo "Dialect coverage: every Category::Intrinsic OpDef has at least one backend lowering."
