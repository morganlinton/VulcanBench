#!/usr/bin/env bash
# Create a run worktree: a separate checkout pinned to a tag, with its own venv.
#
# Sweeps and judging rounds run from here, never from the checkout you edit, so
# main can keep moving while a multi-day sweep runs. The worktree is detached at
# the tag; nobody commits in it. See docs/HOW_WE_WORK.md.
#
#   bash scripts/run_worktree.sh bench/2026-10-05-grok47-safety
#   # -> ../VulcanBench-runs/bench-2026-10-05-grok47-safety
#
# Override the parent directory with RUN_WORKTREES=/some/dir.
#
# The worktree gets its own .venv (an editable install of ITS harness/). A venv
# from another checkout would import that checkout's code while reading this
# one's tasks; run summaries would show it as two different source commits.
set -euo pipefail
cd "$(dirname "$0")/.."
# The primary checkout, even when this script is invoked from another worktree.
main_checkout=$(cd "$(git rev-parse --git-common-dir)/.." && pwd)

tag=${1:-}
if [[ -z "$tag" ]]; then
  echo "usage: $0 <tag>   (cut one with: bash scripts/cut_bench_tag.sh <label>)" >&2
  exit 2
fi
git fetch --quiet origin "refs/tags/$tag:refs/tags/$tag" 2>/dev/null || true
if ! git rev-parse -q --verify "refs/tags/$tag^{commit}" >/dev/null; then
  echo "refusing: no tag $tag (run worktrees are pinned to tags, not branches)" >&2
  exit 1
fi

parent=${RUN_WORKTREES:-$(cd .. && pwd)/VulcanBench-runs}
dir="$parent/${tag//\//-}"
if [[ -e "$dir" ]]; then
  echo "exists: $dir (remove with: git worktree remove $dir)" >&2
  exit 1
fi
mkdir -p "$parent"
git worktree add --detach "$dir" "$tag"

cd "$dir"
if git lfs version >/dev/null 2>&1; then
  git lfs pull
fi
make setup >/dev/null
.venv/bin/python -m harness.suite_lock check

cat <<EOF

Run worktree ready at $dir (detached at $tag).

Launch sweeps from it, writing results OUTSIDE it (absolute OUTROOT) so they
outlive the worktree. The main checkout's runs-*/ dirs are gitignored and are
where the card and population scripts already look:
  cd $dir
  OUTROOT=$main_checkout/runs-effort-<model> MODEL=... PROBE=... \\
    bash scripts/cii-v4-board/run_effort_sweep.sh

When the sweep is archived: git worktree remove $dir
EOF
