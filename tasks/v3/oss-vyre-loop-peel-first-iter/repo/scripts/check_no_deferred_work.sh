#!/usr/bin/env bash
# Enforce zero deferred-work markers in shipped Rust implementation paths.
#
# This gate scans production source comments for language that marks work as
# postponed or fake, and scans code for explicit unimplemented paths. It does
# not scan runtime error strings: messages such as "cert field is still TBD"
# are validators, not engineering excuses.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

paths=(
  "vyre-core/src"
  "vyre-foundation/src"
  "vyre-driver/src"
  "vyre-driver-wgpu/src"
  "vyre-driver-cuda/src"
  "vyre-driver-spirv/src"
  "vyre-primitives/src"
  "vyre-libs/src"
  "vyre-runtime/src"
  "vyre-reference/src"
  "vyre-harness/src"
  "vyre-aot/src"
  "vyre-cc/src"
  "conform/vyre-conform-enforce/src"
  "conform/vyre-conform-generate/src"
  "conform/vyre-conform-runner/src"
  "conform/vyre-conform-spec/src"
)

comment_pattern='(^[[:space:]]*(//|//!|///).*(TODO|FIXME|WIP|known limitation|future work|out[-_ ]of[-_ ]scope|not implemented|placeholder|stub|migration[[:space:]]+plan|deferred[[:space:]]+(to|behind)|ships[[:space:]]+for[[:space:]]+later|temporary[[:space:]]+(implementation|workaround|hack|placeholder|stub)))'
code_pattern='todo![[:space:]]*\(|unimplemented![[:space:]]*\(|panic![[:space:]]*\([[:space:]]*"not implemented'

failures=0

while IFS= read -r -d '' file; do
  while IFS=: read -r line text; do
    [[ -z "${line:-}" ]] && continue
    printf 'DEFERRED-WORK COMMENT: %s:%s:%s\n' "$file" "$line" "$text" >&2
    failures=$((failures + 1))
  done < <(grep -nEi "$comment_pattern" "$file" || true)

  while IFS=: read -r line text; do
    [[ -z "${line:-}" ]] && continue
    printf 'UNIMPLEMENTED CODE PATH: %s:%s:%s\n' "$file" "$line" "$text" >&2
    failures=$((failures + 1))
  done < <(grep -nE "$code_pattern" "$file" || true)
done < <(find "${paths[@]}" -type f -name '*.rs' -print0 2>/dev/null)

if [[ "$failures" -gt 0 ]]; then
  echo "FAIL: deferred-work markers found in shipped source. Fix the implementation or rewrite the comment to state the concrete contract." >&2
  exit 1
fi

echo "OK: no deferred-work markers in shipped source"
