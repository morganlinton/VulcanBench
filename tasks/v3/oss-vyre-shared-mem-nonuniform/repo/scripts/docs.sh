#!/usr/bin/env bash
# Fast incremental documentation generator with changed-only filtering.
#
# Flags:
#   --changed-only    Only build documentation for crates containing modified files.
#                     Diffs against target branch (pull requests) or previous commit.
#
# By default, runs cargo doc --no-deps --workspace --keep-going.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

CHANGED_ONLY=0
for arg in "$@"; do
    if [[ "$arg" == "--changed-only" ]]; then
        CHANGED_ONLY=1
    fi
done

# Run cargo doc
if [[ "$CHANGED_ONLY" -eq 1 ]]; then
    echo "Doc CI: Checking for changed files..."
    
    # Try to find target/base commit for diffing
    BASE_COMMIT=""
    if [[ -n "${GITHUB_BASE_REF:-}" ]]; then
        BASE_COMMIT="origin/$GITHUB_BASE_REF"
    elif [[ -n "${GITHUB_EVENT_BEFORE:-}" && "${GITHUB_EVENT_BEFORE}" != "0000000000000000000000000000000000000000" ]]; then
        BASE_COMMIT="$GITHUB_EVENT_BEFORE"
    else
        # Local fallback: try origin/main, then origin/master, then HEAD~1
        if git rev-parse origin/main >/dev/null 2>&1; then
            BASE_COMMIT="origin/main"
        elif git rev-parse origin/master >/dev/null 2>&1; then
            BASE_COMMIT="origin/master"
        else
            BASE_COMMIT="HEAD~1"
        fi
    fi

    echo "Diffing against base commit: $BASE_COMMIT"
    
    if ! git rev-parse "$BASE_COMMIT" >/dev/null 2>&1; then
        echo "Base commit $BASE_COMMIT not found. Building documentation for all workspace crates..."
        "$CARGO_RUNNER" doc --no-deps --workspace --keep-going
        exit 0
    fi
    
    # Get list of changed files
    changed_files=$(git diff --name-only "$BASE_COMMIT" 2>/dev/null || true)
    
    if [[ -z "$changed_files" ]]; then
        echo "No changed files detected. Skipping docs build."
        exit 0
    fi
    
    # If workspace-level configs, root README, or root Cargo.toml changed, build everything
    if echo "$changed_files" | grep -qE '^(Cargo\.toml|Cargo\.lock|README\.md|docs/)'; then
        echo "Workspace-level changes detected. Building documentation for all workspace crates..."
        "$CARGO_RUNNER" doc --no-deps --workspace --keep-going
        exit 0
    fi
    
    # Determine which crates are affected
    affected_packages=()
    
    for file in $changed_files; do
        [[ ! -f "$file" ]] && continue
        # Find the nearest Cargo.toml up the directory structure
        dir=$(dirname "$file")
        while [[ "$dir" != "." && "$dir" != "/" ]]; do
            if [[ -f "$dir/Cargo.toml" ]]; then
                # Get package name from Cargo.toml
                pkg_name=$(grep -m 1 -E '^name\s*=\s*' "$dir/Cargo.toml" | cut -d'"' -f2 | cut -d"'" -f2 || true)
                if [[ -n "$pkg_name" ]]; then
                    affected_packages+=("$pkg_name")
                fi
                break
            fi
            dir=$(dirname "$dir")
        done
    done
    
    # Unique affected packages
    mapfile -t unique_packages < <(printf '%s\n' "${affected_packages[@]:-}" | sort -u | grep -v '^$')
    
    if [[ ${#unique_packages[@]} -eq 0 ]]; then
        echo "No crate-level changes detected. Skipping docs build."
        exit 0
    fi
    
    echo "Building documentation for affected packages: ${unique_packages[*]}"
    for pkg in "${unique_packages[@]}"; do
        echo "Building docs for $pkg..."
        "$CARGO_RUNNER" doc --no-deps -p "$pkg" --keep-going
    done
else
    echo "Building documentation for the entire workspace..."
    "$CARGO_RUNNER" doc --no-deps --workspace --keep-going
fi

echo "Documentation build complete."
