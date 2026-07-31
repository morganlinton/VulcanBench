#!/usr/bin/env bash
# Proves the external-IR extension demo is not just present but buildable as
# its own crate, outside the Vyre workspace.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

demo="examples/external_ir_extension"
manifest="$demo/Cargo.toml"

if [[ ! -f "$manifest" ]]; then
    echo "external-ir-extension-ci gate: missing $manifest." >&2
    echo "Fix: restore the standalone extension demonstrator crate." >&2
    exit 1
fi

demo_loc=$(find "$demo" -name '*.rs' -exec cat {} + | wc -l | tr -d ' ')
if [[ "$demo_loc" -gt 200 ]]; then
    echo "external-ir-extension-ci gate: $demo_loc Rust LOC exceeds the 200 LOC cap." >&2
    echo "Fix: keep external extension onboarding trivial enough to audit in one screen." >&2
    exit 1
fi

if ! grep -q '^\[workspace\]$' "$manifest"; then
    echo "external-ir-extension-ci gate: $manifest does not declare its own workspace." >&2
    echo "Fix: keep the demo isolated so it cannot depend on unintentional workspace state." >&2
    exit 1
fi

CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" check --manifest-path "$manifest" --locked -q

echo "external-ir-extension-ci gate: demo builds as an isolated crate at ${demo_loc} LOC."
