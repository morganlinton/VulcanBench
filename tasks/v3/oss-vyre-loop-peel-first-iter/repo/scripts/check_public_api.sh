#!/usr/bin/env bash
# Gap #6  -  public-API snapshot gate.
#
# Every vyre crate that publishes a public API has its surface frozen
# in `<crate>/PUBLIC_API.md`. Generated via `cargo_full public-api` and
# committed. This gate fails when any PR changes the public surface
# without a matching PUBLIC_API.md diff.
#
# Pass: `cargo_full public-api diff` against the checked-in snapshot is empty.
# Fail: public surface changed; either update PUBLIC_API.md explicitly
# or revert the breaking change.

set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v cargo-public-api >/dev/null 2>&1; then
    echo "gap #6: cargo-public-api not installed" >&2
    echo "  install: cargo_full install cargo-public-api --locked" >&2
    exit 1
fi

source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner
CRATES=(vyre-core vyre-foundation vyre-driver vyre-driver-wgpu vyre-primitives vyre-spec)
FAIL=0

for crate in "${CRATES[@]}"; do
    snapshot="$crate/PUBLIC_API.md"
    if [ ! -f "$snapshot" ]; then
        echo "gap #6: missing $snapshot" >&2
        echo "  fix: $CARGO_RUNNER public-api --manifest-path $crate/Cargo.toml > $snapshot" >&2
        FAIL=1
        continue
    fi
    current=$("$CARGO_RUNNER" public-api --manifest-path "$crate/Cargo.toml" 2>/dev/null || true)
    expected=$(cat "$snapshot")
    if [ "$current" != "$expected" ]; then
        echo "gap #6: $crate public API drifted from $snapshot" >&2
        diff <(echo "$expected") <(echo "$current") | head -40 >&2 || true
        FAIL=1
    fi
done

exit "$FAIL"
