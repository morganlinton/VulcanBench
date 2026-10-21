#!/usr/bin/env bash
# Audit-D  -  no `#![allow(missing_docs)]` overrides.
#
# `[workspace.lints.rust] missing_docs = "deny"` is the workspace lint
# floor. Every member inherits it via `[lints] workspace = true`. A
# crate-level `#![allow(missing_docs)]` defeats that floor and lets
# undocumented public surface land. This gate forbids the override.
#
# Module-scoped `#[allow(missing_docs)]` on a generated module (auto-
# emitted op wrappers, etc.) is allowed; the gate only flags the
# top-level inner attribute that disables the lint for the whole
# crate.
#
# Usage:
#   scripts/check_no_missing_docs_override.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

errors=()

while IFS= read -r lib; do
    [[ -z "$lib" ]] && continue
    rel="${lib#./}"
    # Pure inner attribute on the crate root: `#![allow(missing_docs)]`
    # without surrounding context. Module-level `#[allow(missing_docs)]`
    # is fine.
    if grep -nE '^[[:space:]]*#!\[allow\([^)]*missing_docs' "$lib" >/dev/null 2>&1; then
        line=$(grep -nE '^[[:space:]]*#!\[allow\([^)]*missing_docs' "$lib" | head -1)
        errors+=("$rel: crate-root #![allow(missing_docs)] override  -  $line")
    fi
done < <(find . -maxdepth 4 -name 'lib.rs' -path '*/src/lib.rs' -not -path '*/target*' -not -path '*/target-*/*' 2>/dev/null)

if (( ${#errors[@]} > 0 )); then
    echo "no-missing-docs-override gate: ${#errors[@]} violations." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: remove the inner-attribute override and document the public" >&2
    echo "items the lint surfaces. The workspace deny floor is the contract;" >&2
    echo "individual crates do not opt out." >&2
    exit 1
fi

echo "no-missing-docs-override gate: every workspace lib.rs respects the deny floor."
exit 0
