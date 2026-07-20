#!/usr/bin/env bash
# P1 inventory #92  -  GPU-required tests fail loudly when probes are misconfigured.
#
# A GPU test that says `if no GPU { return; }` is a smoke alarm wired
# to nothing. The AGENTS.md rule states: "There is no environment in
# this fleet without one. If a probe says no GPU  -  surface it loudly,
# don't silently fall back to CPU."
#
# This gate scans Rust files for the silent-skip patterns that defeat
# that rule. The patterns it catches:
#   - `if .. is_err() { return Ok(()); }` after adapter acquisition
#   - `eprintln!("skipped` or `println!("no GPU` followed by early return
#   - `#[cfg_attr(not(any_gpu), ignore)]` without a paired loud `panic!`
#
# A test allowed to skip silently must declare `#[ignore = "reason"]`
# AND have a paired `_required_loud` test in the same file that
# exercises the same path with `WgpuBackend::acquire_or_panic`.
#
# Usage:
#   scripts/check_gpu_test_loudness.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SILENT_SKIP_PATTERNS=(
    'if .* is_err\(\)\s*\{\s*return Ok\(\(\)\);\s*\}'
    'if .* is_err\(\)\s*\{\s*return;\s*\}'
    'if let Err\([^=]+\)\s*=\s*.*\s*\{\s*return'
    'println!\("(skipped|no GPU|GPU unavailable)'
    'eprintln!\("(skipped|no GPU|GPU unavailable)'
    '#\\[cfg\\(not\\(.*gpu.*\\)\\)'
    '#\\[cfg_attr\\(not\\(feature = \"gpu\"\\),\\s*ignore'
    '#\\[cfg_attr\\(not\\(any\\(.*gpu.*\\)\\),\\s*ignore'
    'return; *// *no GPU'
    'return Ok\(\(\)\); *// *no GPU'
)

has_loud_abort() {
    local file="$1"
    local line_number="$2"
    local window_start="$((line_number > 10 ? line_number - 10 : 1))"
    local window_end="$((line_number + 20))"

    if sed -n "${window_start},${window_end}p" "$file" | \
        grep -qE 'acquire_or_panic|panic!\("(no adapter|adapter probe|GPU required|gpu required|headless backend)|assert!\(".*Fix:'; then
        return 0
    fi

    return 1
}

errors=()

while IFS= read -r f; do
    rel="${f#./}"
    for pat in "${SILENT_SKIP_PATTERNS[@]}"; do
        if hits=$(grep -nE "$pat" "$f" 2>/dev/null); then
            while IFS= read -r line; do
                [[ -z "$line" ]] && continue
                line_number="${line%%:*}"
                # Allow only when this skip site has a nearby loud paired test/error path.
                if has_loud_abort "$f" "$line_number"; then
                    continue
                fi
                errors+=("$rel:$line")
            done <<< "$hits"
        fi
    done
done < <(find . -type f -name '*.rs' -not -path '*/target/*' -not -path '*/target-*/*' 2>/dev/null)

if (( ${#errors[@]} > 0 )); then
    echo "gpu-test-loudness gate: ${#errors[@]} silent-skip sites." >&2
    for e in "${errors[@]}"; do
        echo "  $e" >&2
    done
    echo >&2
    echo "Fix: replace the silent skip with WgpuBackend::acquire_or_panic," >&2
    echo "or add a paired loud test that exercises the same path with the" >&2
    echo "panic. The AGENTS.md rule is: silent fallback to CPU is forbidden." >&2
    exit 1
fi

echo "gpu-test-loudness gate: no silent-skip sites."
exit 0
