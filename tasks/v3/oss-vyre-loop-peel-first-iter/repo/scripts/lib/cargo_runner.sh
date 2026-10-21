#!/usr/bin/env bash
# Shared cargo runner selection for release, CI, and benchmark gates.
#
# The workspace prefers `./cargo_full` when it is available, but release
# scripts must still be executable in checkouts where the wrapper is absent.
# In that case they fall back to `cargo` while forcing single-job builds to
# preserve the OOM protection that cargo_full normally provides.

vyre_select_cargo_runner() {
    export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
    if [[ -n "${VYRE_CARGO_RUNNER:-}" ]]; then
        CARGO_RUNNER="$VYRE_CARGO_RUNNER"
    elif [[ -x ./cargo_full ]]; then
        CARGO_RUNNER="./cargo_full"
    else
        CARGO_RUNNER="cargo"
    fi
    export CARGO_RUNNER
}
