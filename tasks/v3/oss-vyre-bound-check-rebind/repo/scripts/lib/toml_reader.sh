#!/usr/bin/env bash
# Shared TOML value reader for release shell helpers.

vyre_read_toml_values() {
    if [[ "$#" -lt 4 ]]; then
        printf 'Fix: vyre_read_toml_values requires MANIFEST, LABEL, EXPECTED_COUNT, and at least one key.\n' >&2
        return 2
    fi
    local manifest="$1"
    local label="$2"
    local expected_count="$3"
    shift 3
    if [[ "$#" -ne "$expected_count" ]]; then
        printf 'Fix: %s requested %s TOML key(s), expected %s.\n' "$label" "$#" "$expected_count" >&2
        return 2
    fi
    local output
    if ! command -v python3 >/dev/null 2>&1; then
        printf 'Fix: python3 with tomllib is required to read %s.\n' "$manifest" >&2
        return 2
    fi
    if ! output="$(python3 - "$manifest" "$label" "$@" <<'PY'
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:
    print("Fix: python3 tomllib is required; use Python 3.11+.", file=sys.stderr)
    sys.exit(2)

path = Path(sys.argv[1])
label = sys.argv[2]
keys = sys.argv[3:]
try:
    with path.open("rb") as handle:
        data = tomllib.load(handle)
except Exception as error:
    print(f"Fix: failed to read {path}: {error}", file=sys.stderr)
    sys.exit(2)

for key in keys:
    current = data
    for part in key.split("."):
        if not isinstance(current, dict) or part not in current:
            print(f"Fix: {path} is missing required {label} key {key}.", file=sys.stderr)
            sys.exit(2)
        current = current[part]
    if isinstance(current, bool):
        print("true" if current else "false")
    elif isinstance(current, (str, int, float)):
        print(current)
    else:
        print(f"Fix: {path} {label} key {key} must be a scalar value.", file=sys.stderr)
        sys.exit(2)
PY
    )"; then
        return 2
    fi
    mapfile -t VYRE_TOML_VALUES <<< "$output"
    if [[ "${#VYRE_TOML_VALUES[@]}" -ne "$expected_count" ]]; then
        printf 'Fix: %s produced %s %s value(s), expected %s.\n' "$manifest" "${#VYRE_TOML_VALUES[@]}" "$label" "$expected_count" >&2
        return 2
    fi
}
