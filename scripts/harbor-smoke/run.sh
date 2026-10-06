#!/bin/bash
# Harbor plumbing smoke test for Frontier v5 (docs/frontier-v5/PHASE1.md).
#
# Runs scripts/harbor-smoke/smoke-task twice through the real Harbor CLI, once
# with the built-in oracle agent (expects reward 1) and once with the no-op
# agent (expects reward 0), and checks that the verifier phase had no network.
# Exercises: separate verifier built from tests/Dockerfile, the top-level
# artifacts handoff, reward.json metric parsing, and no-network enforcement.
# No model is called. Needs `uv tool install harbor` and a Docker daemon.
#
# Usage: scripts/harbor-smoke/run.sh [jobs-dir]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
jobs="$(cd "${1:-$here/jobs}" 2>/dev/null && pwd || { mkdir -p "${1:-$here/jobs}"; cd "${1:-$here/jobs}"; pwd; })"
export PATH="$HOME/.local/bin:$PATH"
command -v harbor >/dev/null || { echo "harbor CLI not found; run: uv tool install harbor" >&2; exit 2; }
fail=0
for agent in oracle nop; do
  rm -rf "$jobs/smoke-$agent"
  # -o must be absolute: a relative jobs dir is resolved against the compose
  # project directory and `docker compose cp` then fails (harbor 0.24.0).
  harbor run -p "$here/smoke-task" -a "$agent" -o "$jobs" --job-name "smoke-$agent" -n 1 -y -q >/dev/null 2>&1 || true
  trial="$(ls -d "$jobs/smoke-$agent"/smoke-task__* 2>/dev/null | head -1)"
  reward="$(python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print(int(d['reward']), int(d['network_blocked']))" "$trial/verifier/reward.json" 2>/dev/null || echo "missing missing")"
  case "$agent" in oracle) want=1 ;; nop) want=0 ;; esac
  echo "$agent: reward=${reward% *} (want $want) network_blocked=${reward#* } (want 1)"
  [ "$reward" = "$want 1" ] || fail=1
done
[ "$fail" = 0 ] && echo "harbor smoke: PASS" || { echo "harbor smoke: FAIL (see $jobs)" >&2; exit 1; }
