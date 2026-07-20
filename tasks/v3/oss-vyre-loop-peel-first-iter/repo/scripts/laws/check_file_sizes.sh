#!/usr/bin/env bash
#
# Layout guideline  -  large-file review prompt (advisory).
#
# A .rs file under a vyre-* crate's src/ that grows past ADVISORY_LINES is
# *flagged for a split-by-responsibility review*: past that size a file has
# often picked up a second responsibility worth factoring into a named
# sub-module. This is a guideline, not a law, and never fails the build.
#
# The hard god-file ceiling (a real cap, ratcheted, with a per-file
# exception list) lives in `scripts/check_max_file_size.sh`. This script
# only surfaces the softer review prompt.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

ADVISORY_LINES=500

flagged=()
while IFS= read -r -d '' file; do
  lines=$(wc -l < "$file")
  if (( lines > ADVISORY_LINES )); then
    flagged+=("$lines $file")
  fi
done < <(find vyre-core vyre-foundation vyre-driver vyre-driver-wgpu vyre-driver-cuda vyre-driver-spirv vyre-runtime vyre-reference vyre-primitives vyre-macros vyre-spec vyre-libs vyre-aot vyre-cc \
  -type d \( -name target -o -name fuzz \) -prune -o \
  -type f -name "*.rs" -print0 2>/dev/null || true)

if [[ ${#flagged[@]} -gt 0 ]]; then
  printf 'Layout guideline: %d .rs file(s) over the %d-line review prompt (sorted descending):\n' \
    "${#flagged[@]}" "$ADVISORY_LINES" >&2
  printf '%s\n' "${flagged[@]}" | sort -rn | head -30 >&2
  printf '\n  Review: where a file has grown a second responsibility, split it\n' >&2
  printf '       so every module has one responsibility. `mod X` in `X.rs` is\n' >&2
  printf '       the canonical layout; factor cohesive sections into named\n' >&2
  printf '       sub-modules. Size alone is not a failure.\n' >&2
  echo '(advisory  -  this guideline never fails the build)' >&2
  exit 0
fi

echo "Layout guideline: no .rs file is over the ${ADVISORY_LINES}-line review prompt."
