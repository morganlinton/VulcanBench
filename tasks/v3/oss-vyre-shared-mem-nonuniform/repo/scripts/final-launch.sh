#!/usr/bin/env bash
#
# Guarded public launch launcher for the configured Vyre / Weir release train.
#
# This script intentionally refuses to run unless the maintainer sets:
#   VYRE_RELEASE_APPROVED=<token derived by scripts/lib/release_train.sh>
#
# It performs the approval-gated publish and push, and records the launch
# verification that completes
# release/plans/paradigm-shift-100-concrete.md:
#   1. cargo_full publish in audited dependency order.
#   2. verify the configured public repository without changing the private repository.
#   3. push the release branch and product-scoped tags.

set -euo pipefail

PREFLIGHT=0
if [[ "${1:-}" == "--preflight" ]]; then
    PREFLIGHT=1
    shift
fi
if [[ "$#" -ne 0 ]]; then
    printf 'Fix: unknown final-launch argument(s): %s\n' "$*" >&2
    exit 2
fi

VYRE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$VYRE_ROOT"
source scripts/lib/cargo_runner.sh
source scripts/lib/repo_boundary.sh
source scripts/lib/release_train.sh
vyre_select_cargo_runner
vyre_load_repo_boundary
vyre_load_release_train

APPROVAL_TOKEN="$VYRE_RELEASE_LAUNCH_APPROVAL_TOKEN"
if [[ "$PREFLIGHT" != "1" && "${VYRE_RELEASE_APPROVED:-}" != "$APPROVAL_TOKEN" ]]; then
    printf 'Fix: refusing final launch without explicit approval.\n' >&2
    printf 'Set VYRE_RELEASE_APPROVED=%s only after maintainer approval for publish and git push. This script verifies %s is already public and does not change %s visibility.\n' "$APPROVAL_TOKEN" "$VYRE_RELEASE_PUBLIC_REPO" "$VYRE_RELEASE_PRIVATE_REPO" >&2
    exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
    printf 'Fix: jq is required to write launch completion evidence.\n' >&2
    exit 2
fi

if ! command -v gh >/dev/null 2>&1; then
    printf 'Fix: GitHub CLI `gh` is required for repository visibility verification.\n' >&2
    exit 2
fi

if ! gh auth status >/dev/null 2>&1; then
    printf 'Fix: GitHub CLI is not authenticated; run gh auth login before final launch.\n' >&2
    exit 2
fi

if ! git remote get-url origin >/dev/null 2>&1; then
    printf 'Fix: git remote `origin` is missing; refusing final launch.\n' >&2
    exit 2
fi

if ! release_branch="$(git symbolic-ref --quiet --short HEAD)"; then
    printf 'Fix: refusing final launch from a detached HEAD.\n' >&2
    exit 2
fi

if [[ -n "$(git status --porcelain)" && "$PREFLIGHT" != "1" ]]; then
    printf 'Fix: working tree has uncommitted or untracked changes; commit or intentionally clear them before final launch.\n' >&2
    exit 2
fi
if [[ "$PREFLIGHT" == "1" && -n "$(git status --porcelain)" ]]; then
    printf 'final-launch preflight note: working tree is dirty; real launch will refuse until committed or intentionally cleared.\n'
fi

for tag in "${VYRE_RELEASE_TAGS[@]}"; do
    if git rev-parse --verify "refs/tags/${tag}" >/dev/null 2>&1; then
        printf 'Fix: release tag %s already exists locally; refusing to risk stale tag target.\n' "$tag" >&2
        exit 2
    fi
    if git ls-remote --exit-code --tags origin "refs/tags/${tag}" >/dev/null 2>&1; then
        printf 'Fix: release tag %s already exists on origin; refusing to overwrite public release tags.\n' "$tag" >&2
        exit 2
    fi
done

repo_visibility="$(gh repo view "$VYRE_RELEASE_PUBLIC_REPO" --json visibility --jq '.visibility' 2>/dev/null || true)"
if [[ -z "$repo_visibility" ]]; then
    printf 'Fix: GitHub repository %s is not visible to gh; refusing final launch before publish.\n' "$VYRE_RELEASE_PUBLIC_REPO" >&2
    exit 2
fi
if [[ "${repo_visibility^^}" != "PUBLIC" ]]; then
    printf 'Fix: GitHub repository %s visibility is %s, expected PUBLIC. %s visibility is intentionally untouched.\n' "$VYRE_RELEASE_PUBLIC_REPO" "$repo_visibility" "$VYRE_RELEASE_PRIVATE_REPO" >&2
    exit 2
fi

if [[ "$PREFLIGHT" == "1" ]]; then
    bash scripts/publish-release.sh --preflight
    printf 'final-launch preflight passed; no publish, evidence commit, tag, or push performed.\n'
    exit 0
fi

export VYRE_RELEASE_BACKEND="${VYRE_RELEASE_BACKEND:-all}"
export VYRE_RELEASE_SHARDS="${VYRE_RELEASE_SHARDS:-64}"
export VYRE_RELEASE_FEATURES="${VYRE_RELEASE_FEATURES:-gpu}"
export VYRE_RELEASE_CERT_DIR="${VYRE_RELEASE_CERT_DIR:-.internals/certs/release-shards}"
release_conformance_certificate="$(scripts/prove-release-shards.sh)"
release_conformance_evidence="release/evidence/conformance/release-all-backends-certificate.json"
mkdir -p "$(dirname "$release_conformance_evidence")"
cp "$release_conformance_certificate" "$release_conformance_evidence"
if [[ ! -s "$release_conformance_evidence" ]]; then
    printf 'Fix: release conformance certificate evidence was not written: %s\n' "$release_conformance_evidence" >&2
    exit 1
fi

VYRE_RELEASE_APPROVED="$VYRE_RELEASE_PUBLISH_APPROVAL_TOKEN" bash scripts/publish-release.sh

printf 'verified GitHub repository is public: %s\n' "$VYRE_RELEASE_PUBLIC_REPO"

mkdir -p release/evidence/final
jq -n \
    --arg public_repo "$VYRE_RELEASE_PUBLIC_REPO" \
    --arg branch "$release_branch" \
    --arg conformance "$release_conformance_evidence" \
    --arg vyre_version "$VYRE_RELEASE_VYRE_VERSION" \
    --arg weir_version "$VYRE_RELEASE_WEIR_VERSION" \
    --arg verify_public_repo_action "$VYRE_RELEASE_VERIFY_PUBLIC_REPO_ACTION" \
    --arg vyre_tag "$VYRE_RELEASE_TAG_VYRE" \
    --arg weir_tag "$VYRE_RELEASE_TAG_WEIR" \
    --arg combined_tag "$VYRE_RELEASE_TAG_COMBINED" \
    '{
        schema_version: 1,
        release_train: {
            vyre: $vyre_version,
            weir: $weir_version
        },
        git: {
            branch: $branch,
            tags: [
                $vyre_tag,
                $weir_tag,
                $combined_tag
            ]
        },
        public_repository: $public_repo,
        external_actions: [
            {
                action: "prove sharded all-backend conformance certificate",
                status: "complete",
                evidence: $conformance
            },
            {
                action: "cargo_full publish approved crates in dependency order",
                status: "complete",
                evidence: "scripts/publish-release.sh"
            },
            {
                action: $verify_public_repo_action,
                status: "complete",
                evidence: ("gh repo view " + $public_repo + " --json visibility")
            },
            {
                action: "git push release branch and tags",
                status: "complete",
                evidence: ("git push origin release branch && git push origin " + $vyre_tag + " " + $weir_tag + " " + $combined_tag)
            }
        ],
        completion_status: "complete"
    }' > release/evidence/final/public-launch-completion.json

"$CARGO_RUNNER" run -j1 --manifest-path xtask/Cargo.toml --bin xtask -- launch-state --output release/evidence/final/public-launch-state.json
"$CARGO_RUNNER" run -j1 --manifest-path xtask/Cargo.toml --bin xtask -- release-completion-audit --output release/evidence/final/completion-audit.json
"$CARGO_RUNNER" run -j1 --manifest-path xtask/Cargo.toml --bin xtask -- vyre-weir-release-gate

git add \
    release/evidence/package/publish-readiness.json \
    release/evidence/conformance/release-all-backends-certificate.json \
    release/evidence/final/public-launch-completion.json \
    release/evidence/final/public-launch-state.json \
    release/evidence/final/completion-audit.json
git commit -m "Record ${VYRE_RELEASE_DISPLAY} public launch"

for tag in "${VYRE_RELEASE_TAGS[@]}"; do
    git tag -a "$tag" -m "$tag"
done

printf 'pushing release branch and product-scoped tags\n'
git push origin "$release_branch"
git push origin "${VYRE_RELEASE_TAGS[@]}"

printf '%s public launch actions completed.\n' "$VYRE_RELEASE_DISPLAY"
