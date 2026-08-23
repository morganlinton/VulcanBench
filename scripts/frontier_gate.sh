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
counts = {}
for p in glob.glob(f"runs/{task}-*/summary.json"):
    s = json.load(open(p))
    if (s.get("scores") or {}).get("functional") == 1.0:
        counts[s.get("model")] = counts.get(s.get("model"), 0) + 1
print(max(counts.values(), default=0))
PY
)
    if [ "$best" -ge 3 ]; then
      echo "[gate] early exit: a model already solved $best runs — verdict is REJECT regardless of remaining runs"
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
for m in models:
    vals = []
    for p in glob.glob(f"runs/{task}-*/summary.json"):
        s = json.load(open(p))
        if s.get("model") == m and (s.get("scores") or {}).get("functional") is not None:
            vals.append(round(s["scores"]["functional"], 3))
    vals = sorted(vals, reverse=True)[:3]
    solves = sum(1 for v in vals if v == 1.0)
    best = max(best, solves)
    print(f"  {m:28s} runs={vals}  solves={solves}/3")
verdict = "ADMIT (hard band: best model <=2/3)" if best <= 2 else f"REJECT (best model solves {best}/3; consider for v1)"
print(f"  -> {verdict}")
PY
