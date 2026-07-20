#!/usr/bin/env bash
# Law B enforcement: no string-based shader emission.
#
# See ARCHITECTURE.md "Absolute architectural laws". Shader emission
# must go through the naga AST → validator → writer pipeline. String
# concatenation produces shaders that only fail at GPU dispatch time,
# bypassing every upstream correctness check.
#
# WGSL knowledge is restricted to vyre-driver-wgpu. This script fails the PR
# if WGSL-syntax-token substrings appear inside a push_str/format!/write!
# call site in any file outside vyre-driver-wgpu/, or if any file inside
# vyre-driver-wgpu/ constructs WGSL via string concat rather than naga AST.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Files that are allowed to contain WGSL syntax tokens because they are
# .wgsl source files on disk (now that ORG-07 extracts embedded shaders
# to real files) or naga integration shims that construct naga IR
# structurally.
allow_path_regex='(\.wgsl$|/tests/|/benches/|/docs/|vyre-driver-wgpu/src/|vyre-driver-wgpu/shaders/|vyre-ops/src/.*/wgsl\.rs|vyre-ops/src/.*/kernel\.rs|scripts/check_no_string_wgsl\.sh|ARCHITECTURE\.md|THESIS\.md|VISION\.md|docs/|benches/shaders/|\.internals/|_findings\.json$|vyre-foundation/src/transform/compiler/)'

# WGSL-syntax tokens that are dead giveaways of shader construction. A
# file outside vyre-driver-wgpu that has any of these in proximity to a
# `push_str`, `format_args!`, or `write!` call is almost certainly
# building WGSL as a string.
wgsl_tokens=(
  '@compute'
  '@workgroup_size'
  '@group('
  '@binding('
  'var<storage'
  'var<uniform'
  'var<workgroup'
  '-> @location'
)

violations=0

for token in "${wgsl_tokens[@]}"; do
  # Find every file containing this token. Skip allowed paths.
  while IFS= read -r file; do
    # Skip the allowlisted paths.
    if [[ "$file" =~ $allow_path_regex ]]; then
      continue
    fi
    # Only flag if the token appears inside a string literal context
    # (surrounded by quotes or in a raw string r#"..."#). A bare
    # grep match for "@compute" in a comment is fine  -  we only
    # care about shader construction.
    if grep -qE "(push_str|format_args|format!|write!|writeln!|r#\")" "$file"; then
      # Get the matching line numbers for richer reporting.
      matches="$(grep -n -F "$token" "$file" | grep -vE '^[0-9]+:[[:space:]]*(//|/\*|\*)' || true)"
      if [[ -n "$matches" ]]; then
        echo "LAW B VIOLATION: WGSL token '$token' in a file that builds strings." >&2
        echo "  File: $file" >&2
        echo "$matches" | head -5 | sed 's/^/    /' >&2
        echo "" >&2
        echo "  WGSL construction belongs inside vyre-driver-wgpu via the naga AST" >&2
        echo "  pipeline. See ARCHITECTURE.md Law B." >&2
        echo "" >&2
        violations=$((violations + 1))
      fi
    fi
  done < <(grep -rl -F "$token" --include='*.rs' --exclude-dir=target --exclude-dir=.git "$REPO_ROOT" 2>/dev/null || true)
done

# Additional check: even inside vyre-driver-wgpu, we refuse any file whose name
# suggests shader emission but that reaches for String. The naga pipeline
# returns a String from write_string at the very end; that is allowed.
# But multiple push_str sites inside a file named like "emit_wgsl.rs" is
# a smell.
if [[ -d "$REPO_ROOT/vyre-driver-wgpu/src" ]]; then
  while IFS= read -r file; do

    # Count push_str occurrences that look like WGSL construction.
    count="$(grep -c 'push_str' "$file" || true)"
    if [[ "$count" -gt 0 ]]; then
      # Does the file also reference @compute or var<storage?
      if grep -qE '@compute|var<storage|@workgroup_size' "$file"; then
        echo "LAW B VIOLATION: $file has $count push_str calls and WGSL tokens," >&2
        echo "  but does not route through naga::back::wgsl. This is string-" >&2
        echo "  based emission inside vyre-driver-wgpu, which Law B forbids." >&2
        echo "" >&2
        violations=$((violations + 1))
      fi
    fi
  done < <(find "$REPO_ROOT/vyre-driver-wgpu/src" -type f -name '*.rs' 2>/dev/null)
fi

# Law B ratchet: cap the current violation count. A regression fails CI.
# Lower HIGHWATER_LAW_B whenever a file migrates from string-WGSL to native
# naga AST emission. 0 means the contract is fully enforced.
HIGHWATER_LAW_B=0

if [[ "$violations" -gt "$HIGHWATER_LAW_B" ]]; then
  echo "Law B regression: $violations string-based WGSL violation(s), cap is $HIGHWATER_LAW_B." >&2
  echo "Shader emission must go through the naga AST pipeline." >&2
  exit 1
fi
if [[ "$violations" -lt "$HIGHWATER_LAW_B" ]]; then
  echo "Law B progress: $violations violations (cap $HIGHWATER_LAW_B). Lower HIGHWATER_LAW_B to ratchet."
fi
echo "Law B: $violations string-based WGSL violation(s) (cap: $HIGHWATER_LAW_B)."

# --- §8 ratchet: cap `naga::front::wgsl::parse_str` site count --------------
#
# Vision §8: every lowering emits `naga::Module` natively. Today many op
# lowerings parse a static WGSL file via `naga::front::wgsl::parse_str`.
# That is a transitional step (the WGSL is not built from Rust strings), but
# it still violates the long-term contract. This ratchet fixes the current
# count and fails CI if the count ever increases. Lower the cap whenever a
# file migrates to native AST emission.

HIGHWATER_PARSE_STR=0
CURRENT_PARSE_STR="$(grep -rln 'naga::front::wgsl::parse_str' vyre-driver-wgpu/src vyre-foundation/src 2>/dev/null | wc -l | tr -d ' ' || true)"

if [[ "$CURRENT_PARSE_STR" -gt "$HIGHWATER_PARSE_STR" ]]; then
    printf '§8 regression: naga::front::wgsl::parse_str call-site files rose from %s to %s.\n' "$HIGHWATER_PARSE_STR" "$CURRENT_PARSE_STR" >&2
    printf 'Fix: emit naga::Module AST directly; do not add another parse_str site.\n' >&2
    exit 1
fi
if [[ "$CURRENT_PARSE_STR" -lt "$HIGHWATER_PARSE_STR" ]]; then
    printf '§8 progress: parse_str files dropped from %s to %s. Lower HIGHWATER_PARSE_STR in this script to ratchet the contract.\n' "$HIGHWATER_PARSE_STR" "$CURRENT_PARSE_STR"
fi

echo "§8 string-shader file count: $CURRENT_PARSE_STR (cap: $HIGHWATER_PARSE_STR)"
