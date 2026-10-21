#!/usr/bin/env bash
# Repo hygiene and contribution discipline.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

failed=0

require_file() {
    local path="$1"
    if [[ ! -f "$path" ]]; then
        echo "FAIL: required repository file missing: $path" >&2
        failed=1
    else
        echo "  ✓ $path"
    fi
}

require_dir_file_count() {
    local dir="$1"
    local glob="$2"
    local min_count="$3"
    local count
    count="$(find "$dir" -maxdepth 1 -type f -name "$glob" 2>/dev/null | wc -l | tr -d ' ')"
    if [[ "$count" -lt "$min_count" ]]; then
        echo "FAIL: $dir needs at least $min_count '$glob' files; found $count" >&2
        failed=1
    else
        echo "  ✓ $dir has $count '$glob' files"
    fi
}

require_file README.md
require_file CONTRIBUTING.md
require_file CODE_OF_CONDUCT.md
require_file SECURITY.md
require_file CHANGELOG.md
require_file LICENSE-APACHE
require_file LICENSE-MIT
require_file CODEOWNERS
require_file .github/CODEOWNERS
require_file .github/PULL_REQUEST_TEMPLATE.md
require_file .github/dependabot.yml
require_file .github/workflows/ci.yml
require_file .github/workflows/gpu-parity.yml
require_file .github/workflows/architectural-invariants.yml
require_dir_file_count .github/ISSUE_TEMPLATE '*.md' 3

for redirect in CLAUDE.md GEMINI.md; do
    if ! grep -q 'compatibility redirect' "$redirect" || ! grep -q 'AGENTS.md' "$redirect"; then
        echo "FAIL: $redirect must be a compatibility redirect to AGENTS.md, not a separate policy file" >&2
        failed=1
    elif [[ "$(wc -l < "$redirect" | tr -d ' ')" -gt 8 ]]; then
        echo "FAIL: $redirect is too large for a redirect stub" >&2
        failed=1
    else
        echo "  ✓ $redirect redirects to AGENTS.md"
    fi
done

dev_artifacts="$(
    find . \
        -path './target' -prune -o \
        -path '*/target' -prune -o \
        -path './.git' -prune -o \
        \( -name '.pytest_cache' -o -name '.cursor' -o -name '__pycache__' \) \
        -print 2>/dev/null
)"
if [[ -n "$dev_artifacts" ]]; then
    echo "FAIL: developer cache artifacts present in repository tree:" >&2
    echo "$dev_artifacts" >&2
    echo "Fix: remove generated caches and keep them covered by .gitignore." >&2
    failed=1
else
    echo "  ✓ no developer cache artifacts"
fi

if grep -RInE 'no-gpu|gpu-feature|vyre-driver-wgpu/no-gpu' \
    --include='*.yml' --include='*.yaml' --include='Cargo.toml' \
    --exclude-dir=target --exclude-dir=.git \
    .github vyre-driver-wgpu/Cargo.toml 2>/dev/null; then
    echo "FAIL: no-GPU feature escape hatch found in CI or WGPU manifest" >&2
    failed=1
else
    echo "  ✓ no no-GPU CI or manifest escape hatch"
fi

FORBIDDEN_EXTS='\.(rlib|so|dylib|exe|o|a|bin|dll|lib|pdb|pyd|whl|tgz|tar\.gz|zip|old|backup|orig|bak)$'
binary_artifacts="$(
    find . \
        -path './target' -prune -o \
        -path '*/target' -prune -o \
        -path '*/target-*' -prune -o \
        -path '*/.cargo-target' -prune -o \
        -path '*/.cargo-target-*' -prune -o \
        -path './.git' -prune -o \
        -path '*/tests/corpus/*' -prune -o \
        -path '*/tests/fixtures/*' -prune -o \
        -type f -regextype posix-extended -regex ".*${FORBIDDEN_EXTS}" \
        -print 2>/dev/null
)"
if [[ -n "$binary_artifacts" ]]; then
    echo "FAIL: binary/backup artifacts present in repository tree:" >&2
    echo "$binary_artifacts" | head -10 >&2
    failed=1
else
    echo "  ✓ no binary/backup artifacts"
fi

build_artifacts="$(
    find . \
        -path './target' -prune -o \
        -path '*/target' -prune -o \
        -path '*/target-*' -prune -o \
        -path '*/.cargo-target' -prune -o \
        -path '*/.cargo-target-*' -prune -o \
        -path './.git' -prune -o \
        \( -name 'node_modules' -o -name '.venv' -o -name '.next' -o -name 'dist' \) \
        -print 2>/dev/null
)"
if [[ -n "$build_artifacts" ]]; then
    echo "FAIL: build-output artifacts present in repository tree:" >&2
    echo "$build_artifacts" | head -5 >&2
    failed=1
else
    echo "  ✓ no build-output artifacts"
fi

if [[ -f .github/workflows-paused/gpu-parity.yml ]]; then
    echo "FAIL: GPU parity workflow is paused; it must be active under .github/workflows/" >&2
    failed=1
else
    echo "  ✓ GPU parity workflow is active"
fi

silent_gpu_skips="$(
    grep -RInE 'no GPU.*skipp|skipp.*no GPU|adapter missing.*skipp|skipp.*adapter missing' \
        --include='*.rs' \
        --exclude-dir=target \
        --exclude-dir=.git \
        . 2>/dev/null || true
)"
if [[ -n "$silent_gpu_skips" ]]; then
    echo "FAIL: silent GPU skip language found:" >&2
    echo "$silent_gpu_skips" >&2
    failed=1
else
    echo "  ✓ no silent GPU skip language"
fi

if [[ "$failed" -ne 0 ]]; then
    exit 1
fi

exit 0
