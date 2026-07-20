#!/usr/bin/env bash
# Enforce that platform crate docs/comments do not name downstream consumers.
#
# Code-level dependency gates catch `use` edges. This gate catches semantic
# coupling through Rust comments, crate docs, and crate-local markdown.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PLATFORM_CRATES=(
  "vyre-core"
  "vyre-spec"
  "vyre-macros"
  "vyre-foundation"
  "vyre-primitives"
  "vyre-intrinsics"
  "vyre-libs"
  "vyre-reference"
  "vyre-driver"
  "vyre-driver-cuda"
  "vyre-driver-wgpu"
  "vyre-driver-spirv"
  "vyre-runtime"
)

PLATFORM_MARKDOWN_FILES=(
  "README.md"
  "docs/ARCHITECTURE.md"
  "docs/HOT_PATH_PROOFS.md"
  "docs/MATH_PRIMITIVES_PLACEMENT.md"
  "docs/PARSING_EXECUTION_PLAN.md"
  "docs/PREDICATE_EXPR_DUALITY.md"
  "docs/ERROR_SURFACE.md"
  "docs/RELEASE.md"
  "docs/RELEASE_CHECKLIST.md"
  "docs/TESTING_PROGRAM.md"
  "docs/RECURSION_THESIS.md"
  "docs/RUNTIME_PIPELINE.md"
  "docs/library-tiers.md"
  "docs/megakernel-wiring.md"
  "docs/ops-catalog.md"
  "docs/parsing-and-frontends.md"
  "docs/region-chain.md"
  "docs/consumer-integration.md"
)

PLATFORM_TEXT_FILES=(
  "docs/optimization/OP_MATRIX.toml"
)

SELF_SUBSTRATE_PLATFORM_DIRS=(
  "analysis"
  "data"
  "graph"
  "hardware"
  "logic"
  "math"
  "optimization"
  "optimizer"
  "scheduling"
  "telemetry"
)

FORBIDDEN_RE='(^|[^A-Za-z0-9_])(weir|surgec|gossan|keyhog)([^A-Za-z0-9_]|$)'
violations=()

scan_rust_comments() {
  local file="$1"
  local line_no=0
  local line
  while IFS= read -r line; do
    line_no=$((line_no + 1))
    case "$line" in
      [[:space:]]*"//!"*|[[:space:]]*"///"*|[[:space:]]*"// "*|*"/*"*|[[:space:]]*"*"*)
        if [[ "${line,,}" =~ $FORBIDDEN_RE ]]; then
          violations+=("$file:$line_no:$line")
        fi
        ;;
    esac
  done < "$file"
}

scan_markdown() {
  local file="$1"
  local line_no=0
  local line
  while IFS= read -r line; do
    line_no=$((line_no + 1))
    if [[ "${line,,}" =~ $FORBIDDEN_RE ]]; then
      violations+=("$file:$line_no:$line")
    fi
  done < "$file"
}

for crate in "${PLATFORM_CRATES[@]}"; do
  [[ -d "$crate" ]] || continue
  while IFS= read -r file; do
    scan_rust_comments "$file"
  done < <(find "$crate/src" -type f -name '*.rs' 2>/dev/null | sort)

  for doc in "$crate/README.md" "$crate/ARCHITECTURE.md" "$crate/CONFIG.md"; do
    [[ -f "$doc" ]] && scan_markdown "$doc"
  done
done

for doc in "${PLATFORM_MARKDOWN_FILES[@]}"; do
  [[ -f "$doc" ]] && scan_markdown "$doc"
done

for doc in "${PLATFORM_TEXT_FILES[@]}"; do
  [[ -f "$doc" ]] && scan_markdown "$doc"
done

if [[ -d "vyre-self-substrate/src" ]]; then
  [[ -f "vyre-self-substrate/src/lib.rs" ]] && scan_rust_comments "vyre-self-substrate/src/lib.rs"
  for dir in "${SELF_SUBSTRATE_PLATFORM_DIRS[@]}"; do
    [[ -d "vyre-self-substrate/src/$dir" ]] || continue
    while IFS= read -r file; do
      scan_rust_comments "$file"
    done < <(find "vyre-self-substrate/src/$dir" -type f -name '*.rs' 2>/dev/null | sort)
  done
fi

if (( ${#violations[@]} > 0 )); then
  printf 'platform consumer-doc boundary: %d violations.\n' "${#violations[@]}" >&2
  printf '%s\n' "${violations[@]}" >&2
  printf '\nFix: platform crate docs/comments must describe capabilities generically. Consumer-specific names belong in consumer crates or explicit release-integration evidence.\n' >&2
  exit 1
fi

printf 'platform consumer-doc boundary: platform crate docs/comments are consumer-neutral.\n'
