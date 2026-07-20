#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

legacy_op="Op""Spec"
legacy_entry="Op""Spec""Entry"
legacy_runtime="RUNTIME""_REGISTRY"
legacy_macro="register_op""_spec!"
pattern="${legacy_op}|${legacy_entry}|${legacy_runtime}|${legacy_macro}"

# Historical planning + audits + changelogs describe the removed shim
# legitimately. This script itself must also be excluded so its own
# documentation of what it forbids doesn'"'"'t self-trigger.
if rg -n -S --hidden \
    -g '!target' \
    -g '!.git' \
    -g '!.internals/planning/**' \
    -g '!.internals/plans/**' \
    -g '!.internals/audits/**' \
    -g '!.internals/archive/**' \
    -g '!.internals/release/**' \
    -g '!audits/**' \
    -g '!CHANGELOG.md' \
    -g '!scripts/check_no_opspec_tokens.sh' \
    -g '!scripts/check_release_signoff.sh' \
    -g '!.github/workflows/architectural-invariants.yml' \
    "$pattern" .; then
  echo "Legacy operation-registration shim token detected. Fix: migrate to dialect OpDefRegistration only." >&2
  exit 1
fi
