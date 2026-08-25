#!/usr/bin/env bash
# Phase 8 hard gate: reject remaining naga::front::wgsl::parse_str call sites.
#
# vyre-core must not parse WGSL source back into naga::Module. Every
# dialect lowering should build the naga::Module programmatically via
# the shared builder family in vyre-driver-wgpu.
#
# This gate intentionally matches WGSL parser calls, not identifiers that
# merely contain "parse_str" such as proc-macro string-list parsers or sparse
# C lexer names.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Files allowed to use parse_str (wgpu naga integration + tests).
allow='(vyre-driver-wgpu/|scripts/|tests/|benches/|docs/|\.md$)'

# Match qualified WGSL parser calls. Unqualified syn::parse_str and helper
# names like parse_string_array are not WGSL lowering debt.
mapfile -t offenders < <(
  grep -rEln '(^|[^[:alnum:]_])((naga::front::)?wgsl::parse_str|front::wgsl::parse_str)[[:space:]]*\(' --include='*.rs' --exclude-dir=target --exclude-dir=.git "$REPO_ROOT" 2>/dev/null \
    | grep -Ev "$allow" \
    | sort -u
)

count=${#offenders[@]}
echo "Phase 8 WGSL parse gate: $count production file(s) still call WGSL parse_str"
if [[ "$count" -gt 0 ]]; then
  printf '  %s\n' "${offenders[@]}"
  echo "Fix: replace WGSL source parsing with typed naga Module construction." >&2
  exit 1
fi

exit 0
