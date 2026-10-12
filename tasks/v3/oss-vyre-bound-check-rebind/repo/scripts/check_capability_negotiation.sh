#!/usr/bin/env bash
# Law C enforcement: backends must advertise supported_ops; execute() must
# validate before dispatch.
#
# See THESIS.md + docs/memory-model.md. Capability negotiation is what
# makes an open IR safe: a frontend may emit an op the backend has never
# seen, and the validator catches that at compile time, not at GPU
# dispatch time. Every BackendRegistration is contractually obligated to
# supply a supported_ops function. Every Backend impl that dispatches
# programs must call validate_program before touching the substrate.
#
# This guard stays failing until Codex-FIX-ARCH FIX 2 lands
# capability-negotiation across the workspace; at that point it passes
# and holds the line against regressions.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

violations=0

# 1. Every `inventory::submit!(BackendRegistration { ... })` block must
#    include a `supported_ops: ...` field. A block without it is a
#    Backend that lies about capability  -  registers itself but cannot
#    declare what it can execute.
#
#    We scan Rust source files for `BackendRegistration {` opening
#    brace followed (within 20 lines) by the matching `}`  -  the block
#    must contain `supported_ops`.
while IFS= read -r file; do
  # Extract BackendRegistration { ... } construction blocks with awk.
  # Skip the type's own `impl BackendRegistration {` method block and
  # the `pub struct BackendRegistration {` definition (line 16 / 29 of
  # vyre-driver inventory_streams.rs)  -  those are not registration
  # construction sites; only `let x = BackendRegistration { … }`,
  # `inventory::submit!(BackendRegistration { … })`, and similar
  # value-construction expressions need to advertise supported_ops.
  missing="$(awk '
    /^[[:space:]]*(impl|pub[[:space:]]+struct|struct)[[:space:]]+BackendRegistration[[:space:]]*\{/ { next }
    /(^|[^a-zA-Z0-9_])BackendRegistration[[:space:]]*\{/     { inblock=1; depth=1; buf=$0 "\n"; next }
    inblock {
      buf = buf $0 "\n"
      n = gsub(/\{/, "{"); depth += n
      n = gsub(/\}/, "}"); depth -= n
      if (depth <= 0) {
        if (buf !~ /supported_ops/) {
          print "BLOCK_MISSING_supported_ops"
          print buf
          print "---"
        }
        inblock = 0; buf = ""
      }
    }
  ' "$file")"

  if [[ -n "$missing" ]]; then
    echo "LAW C VIOLATION: BackendRegistration in $file missing supported_ops." >&2
    echo "$missing" | head -20 | sed 's/^/    /' >&2
    echo "" >&2
    echo "  Every BackendRegistration must advertise \`supported_ops: fn() -> HashSet<OpId>\`" >&2
    echo "  so validate_program() can reject unknown ops before dispatch." >&2
    echo "  See THESIS.md + docs/memory-model.md for the capability contract." >&2
    echo "" >&2
    violations=$((violations + 1))
  fi
done < <(grep -rl 'BackendRegistration' --include='*.rs' "$REPO_ROOT" 2>/dev/null | grep -v '/target/')

# 2. Every Backend impl's `execute` method must call `validate_program`
#    before any substrate-specific work. Scanning is heuristic: look for
#    `fn execute(` in files that implement a Backend trait and grep for
#    a call to `validate_program` within the function body. If absent,
#    flag.
while IFS= read -r file; do
  if ! grep -q 'fn execute[[:space:]]*(' "$file"; then
    continue
  fi
  if ! grep -q '^impl.*Backend.*for' "$file" && ! grep -q '^impl.*Executable.*for' "$file"; then
    continue
  fi
  if ! grep -q 'validate_program' "$file"; then
    echo "LAW C VIOLATION: $file implements Backend::execute but does not call validate_program." >&2
    echo "  Open IR requires every dispatch path to validate that the backend" >&2
    echo "  supports every op in the Program. Skipping validation means an" >&2
    echo "  unknown op reaches the substrate and fails at GPU dispatch time." >&2
    echo "" >&2
    violations=$((violations + 1))
  fi
done < <(grep -rl 'fn execute' --include='*.rs' "$REPO_ROOT" 2>/dev/null | grep -E '(vyre-driver-wgpu|vyre-driver-cuda|vyre-driver-spirv|vyre-reference|conform)' | grep -v '/target/' | grep -v '/tests/')

if [[ "$violations" -gt 0 ]]; then
  echo "Law C failed: $violations capability-negotiation violation(s)." >&2
  echo "Open IR without validation is a promise that fails at GPU dispatch time." >&2
  exit 1
fi

echo "Law C: all backends advertise supported_ops and validate before dispatch."
