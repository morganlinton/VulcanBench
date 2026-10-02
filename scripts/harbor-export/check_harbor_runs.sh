#!/usr/bin/env bash
# Scoring check for the Harbor export under a real `harbor run`: the oracle
# agent (gold solution) must score 1 on every task and the nop agent (no
# changes) must score 0 on every task. Builds every task image, so it loads
# the machine (linux/amd64 is emulated on Apple silicon).
#
# --wait holds until no sweep, chain launcher or gate is running, then for a
# further quiet period, because sweeps rank by wall clock and must not share
# the machine (docs/DECISIONS.md, 2026-09-13).
#
# --after-pid PID instead holds only until that process exits, then starts at
# once, even if another sweep is about to start (owner's call, 2026-10-02).
#
#   nohup bash scripts/harbor-export/check_harbor_runs.sh --wait \
#       > .harbor-checks/check.log 2>&1 &
#
# Output (gitignore it; the oracle copy contains the gold solutions):
#   .harbor-checks/frontier-v4-oracle/   export with solution/solve.sh
#   .harbor-checks/jobs/                 Harbor job directories
#   .harbor-checks/summary.txt           per-agent reward counts, PASS/FAIL
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT" || exit 1
OUT="$ROOT/.harbor-checks"
SUITE=tasks/coding-intelligence-index-v4
DATASET=exports/harbor/frontier-v4
QUIET_SEC=600
BUSY='vulcanbench run --suite|vulcanconduct run|run_.*_all_suites\.sh|run_.*_gate\.sh'
mkdir -p "$OUT"

stamp() { date '+%F %H:%M:%S'; }

if [ "${1:-}" = "--wait" ]; then
  echo "=== waiting for sweeps, chains and gates to finish $(stamp)"
  quiet=0
  while [ "$quiet" -lt "$QUIET_SEC" ]; do
    if pgrep -f "$BUSY" >/dev/null; then quiet=0; else quiet=$((quiet + 60)); fi
    sleep 60
  done
  echo "=== machine quiet for ${QUIET_SEC}s $(stamp)"
elif [ "${1:-}" = "--after-pid" ]; then
  echo "=== waiting for pid ${2:?--after-pid needs a pid} to exit $(stamp)"
  while kill -0 "$2" 2>/dev/null; do sleep 60; done
  echo "=== pid $2 exited $(stamp)"
fi

if ! docker info >/dev/null 2>&1; then
  echo "=== starting Docker Desktop $(stamp)"
  open -a Docker
  for _ in $(seq 1 60); do docker info >/dev/null 2>&1 && break; sleep 10; done
  docker info >/dev/null 2>&1 || { echo "Docker did not start" | tee "$OUT/summary.txt"; exit 1; }
fi

PY=python3
[ -x "$ROOT/.venv/bin/python" ] && PY="$ROOT/.venv/bin/python"
"$PY" scripts/harbor-export/export_suite.py --with-solution "$SUITE" "$OUT/frontier-v4-oracle" || exit 1

run_check() {
  local agent=$1 path=$2
  echo "=== harbor run -a $agent $(stamp)"
  harbor run -p "$path" -a "$agent" -n 1 -y -o "$OUT/jobs" \
    --job-name "frontier-v4-$agent-$(date +%Y%m%d-%H%M%S)" < /dev/null
  echo "=== $agent exit $? $(stamp)"
}
run_check oracle "$OUT/frontier-v4-oracle"
run_check nop "$DATASET"

# Rewards per trial, from each trial's result.json. Oracle passes only if every
# task scored 1, nop only if every task scored 0, with no errored trials.
"$PY" - "$OUT/jobs" <<'EOF' | tee "$OUT/summary.txt"
import json, sys
from pathlib import Path

expected = {"oracle": 1.0, "nop": 0.0}
latest = {}
for job in sorted(Path(sys.argv[1]).iterdir()):
    agent = job.name.split("-")[2]
    if agent in expected:
        latest[agent] = job
for agent, job in sorted(latest.items()):
    rows = []
    for result in sorted(job.glob("*/result.json")):
        r = json.loads(result.read_text())
        reward = ((r.get("verifier_result") or {}).get("rewards") or {}).get("reward")
        error = (r.get("exception_info") or {}).get("exception_type")
        rows.append((r.get("task_name"), reward, error))
    good = [t for t, rw, e in rows if rw == expected[agent] and not e]
    verdict = "PASS" if len(rows) == 23 and len(good) == 23 else "FAIL"
    print(f"{agent}: {verdict} {len(good)}/{len(rows)} trials scored {expected[agent]:g} ({job.name})")
    for t, rw, e in rows:
        if rw != expected[agent] or e:
            print(f"  {t}: reward={rw} error={e}")
for agent in expected:
    if agent not in latest:
        print(f"{agent}: FAIL no job directory")
EOF
echo "=== done $(stamp)"
