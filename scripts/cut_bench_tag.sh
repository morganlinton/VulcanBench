#!/usr/bin/env bash
# Cut an annotated "bench" tag: the exact commit a sweep or judging round runs.
#
# Refuses unless you are on main, the tree is clean and main matches
# origin/main. Suite locks are checked by CI on main and again in the fresh run
# worktree (an edited checkout can hold untracked files inside task dirs, which
# task_hash counts unless they are local byproducts like __pycache__). The tag
# is what a run worktree checks out (scripts/run_worktree.sh) and what every run
# summary's source.harness.describe points back to. See docs/HOW_WE_WORK.md.
#
#   bash scripts/cut_bench_tag.sh grok47-safety          # bench/2026-10-05-grok47-safety
#   bash scripts/cut_bench_tag.sh grok47-safety --push   # also push the tag
set -euo pipefail
cd "$(dirname "$0")/.."

label=${1:-}
push=${2:-}
if [[ -z "$label" || ! "$label" =~ ^[a-z0-9][a-z0-9.-]*$ ]]; then
  echo "usage: $0 <label: lowercase, digits, dots, dashes> [--push]" >&2
  exit 2
fi

branch=$(git rev-parse --abbrev-ref HEAD)
if [[ "$branch" != "main" ]]; then
  echo "refusing: bench tags are cut from main (you are on $branch)" >&2
  exit 1
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "refusing: working tree is not clean (git status)" >&2
  exit 1
fi
git fetch --quiet origin main
if [[ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]]; then
  echo "refusing: local main differs from origin/main (pull or push first)" >&2
  exit 1
fi

tag="bench/$(date +%F)-$label"
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  echo "refusing: tag $tag already exists" >&2
  exit 1
fi
git tag -a "$tag" -m "Bench snapshot $tag at $(git rev-parse --short HEAD)"
echo "tagged $tag at $(git rev-parse --short HEAD)"

if [[ "$push" == "--push" ]]; then
  git push origin "refs/tags/$tag"
else
  echo "push it with: git push origin refs/tags/$tag"
fi
echo "next: make run-worktree TAG=$tag"
