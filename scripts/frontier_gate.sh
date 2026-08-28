#!/bin/zsh
# cii-v2 frontier admission gate: measure a candidate n=3 against both
# reference frontier models and print the admit/reject verdict.
#
# Usage: scripts/frontier_gate.sh <task-id> [tasks-root]
# Resumable: completed runs are counted, not repeated, so re-run this after a
# subscription-limit interruption and it fills only the gaps.
set -u
cd "$(dirname "$0")/.."
export PATH="$PWD/.venv/bin:$HOME/.local/bin:$HOME/.local/node/bin:$HOME/.cargo/bin:$HOME/.local/go/bin:$PATH"
[ -f pricing.local.json ] && export VULCANBENCH_PRICING="$PWD/pricing.local.json"

TASK="$1"
ROOT="${2:-tasks/cii-v2}"
MODELS=(codex:gpt-5.6-sol claude-code:claude-opus-5)
N=3

for model in "${MODELS[@]}"; do
  while true; do
    have=$(MODEL="$model" python3 - "$TASK" <<'PY'
import json, glob, os, sys
task, model = sys.argv[1], os.environ["MODEL"]
n = 0
for p in glob.glob(f"runs/{task}-*/summary.json"):
    s = json.load(open(p))
    if s.get("model") == model and (s.get("scores") or {}).get("functional") is not None:
        n += 1
print(n)
PY
)
    [ "$have" -ge "$N" ] && break
    best=$(python3 - "$TASK" <<'PY'
import json, glob, sys
task = sys.argv[1]
# effort axis: only the WEAKER reference's solves can decide early
n = 0
for p in glob.glob(f"runs/{task}-*/summary.json"):
    s = json.load(open(p))
    if s.get("model") == "codex:gpt-5.6-sol" and (s.get("scores") or {}).get("functional") == 1.0:
        n += 1
print(n)
PY
)
    if [ "$best" -ge 2 ]; then
      echo "[gate] early exit: the weaker reference already solved $best runs (effort bar needs <=1/3), verdict is REJECT regardless of remaining runs"
      break 2
    fi
    echo "[gate] $TASK $model run $((have+1))/$N $(date '+%H:%M:%S')"
    vulcanbench run --task "$TASK" --tasks-root "$ROOT" --model "$model" \
      --sandbox local --no-judges 2>&1 | tail -1
  done
done

python3 - "$TASK" <<'PY'
import json, glob, sys
task = sys.argv[1]
models = ["codex:gpt-5.6-sol", "claude-code:claude-opus-5"]
print(f"\n=== frontier gate verdict: {task} ===")
best = 0
results = {}
for m in models:
    vals = []
    for p in glob.glob(f"runs/{task}-*/summary.json"):
        s = json.load(open(p))
        if s.get("model") == m and (s.get("scores") or {}).get("functional") is not None:
            vals.append(round(s["scores"]["functional"], 3))
    vals = sorted(vals, reverse=True)[:3]
    solves = sum(1 for v in vals if v == 1.0)
    durs = []
    for p in glob.glob(f"runs/{task}-*/summary.json"):
        s = json.load(open(p))
        if s.get("model") == m and (s.get("scores") or {}).get("functional") is not None:
            durs.append(s.get("duration_s") or 0)
    med = sorted(durs)[len(durs) // 2] / 60 if durs else 0.0
    results[m] = (solves, med)
    print(f"  {m:28s} runs={vals}  solves={solves}/3  median={med:.1f} min")
weak_solves = results.get(models[0], (3, 0))[0]
strong_solves, strong_med = results.get(models[1], (3, 0.0))
strong_effort = strong_med >= 10.0 or strong_solves < 3
if weak_solves <= 1 and strong_effort:
    verdict = "ADMIT (effort axis: weaker ref <=1/3 and stronger ref median >=10 min or misses)"
elif not results.get(models[1]):
    verdict = f"REJECT (weaker reference solves {weak_solves}/3; effort bar requires <=1/3)"
else:
    verdict = f"REJECT (weaker ref {weak_solves}/3, stronger ref median {strong_med:.1f} min)"
print(f"  -> {verdict}")
PY
