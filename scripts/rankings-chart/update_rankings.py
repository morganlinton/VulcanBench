"""Rebuild v3_rankings.json.

Non-DeepSeek columns come from suite.json aggregates (one suite run each).
DeepSeek columns aggregate ALL fresh runs per effort across repeat sweeps:
pass@1 = mean of per-task success rates, stderr = std of per-task means /
sqrt(n_tasks). The mislabeled "medium" run (API-coerced to high) is excluded.
"""

import glob
import json
import math
import statistics
from pathlib import Path

from harness.leaderboard import current_task_hashes

HERE = Path(__file__).resolve().parent
REPO = str(HERE.parents[1])
CURRENT_V3_HASHES = current_task_hashes(Path(REPO) / "tasks" / "v3")

# Models with repeat sweeps: aggregate from individual run summaries
# (per-task mean pass@1) instead of a single suite.json.
REPEAT_MODELS = {
    "deepseek:deepseek-v4-flash",
    "zai:glm-5.3",
    "deepseek:deepseek-v4-pro",
    "openai:grok-4.5",
    "openai:gpt-5.6-luna",
    "openai:gpt-5.6-terra",
    "qwen:qwen3.8-max",
}

best = {}
for p in glob.glob(f"{REPO}/runs*/suite-*/suite.json"):
    try:
        with open(p) as f:
            s = json.load(f)
    except Exception:
        continue
    if s.get("suite") != "v3" or not s.get("aggregate") or s["model"] in REPEAT_MODELS:
        continue
    if s["model"].startswith(("zcode:", "ollama:")):
        # Subscription-harness and local-inference suites never join the board.
        continue
    key = (s["model"], s.get("effort") or "")
    a = s["aggregate"][0]
    cand = dict(
        model=s["model"],
        effort=s.get("effort") or "",
        n_tasks=a["n_tasks"],
        n_runs=a["n_runs"],
        pass1=a["pass_at_1"],
        se=a.get("pass_at_1_stderr"),
        avg_total=a.get("avg_total"),
        cost=a.get("total_cost"),
        avg_duration_s=a.get("avg_duration_s"),
        fin=s.get("finished_at", ""),
    )
    cur = best.get(key)
    if cur is None or (cand["n_runs"], cand["fin"]) > (cur["n_runs"], cur["fin"]):
        best[key] = cand

agg: dict[tuple, dict[str, list]] = {}
repeat_summary_paths = glob.glob(f"{REPO}/runs/*/summary.json")
repeat_summary_paths.extend(glob.glob(f"{REPO}/runs/deepseek-v4-pro-high-r3/*/summary.json"))
for p in repeat_summary_paths:
    with open(p) as f:
        s = json.load(f)
    model = str(s.get("model", ""))
    if model not in REPEAT_MODELS or s.get("suite") != "v3":
        continue
    task_id = str(s.get("task_id", ""))
    if s.get("task_hash") != CURRENT_V3_HASHES.get(task_id):
        continue
    eff = (s.get("effort") or {}).get("requested", "")
    # DeepSeek "medium" = duplicate high run (API coercion), excluded.
    if model.startswith("deepseek") and eff == "medium":
        continue
    agg.setdefault((model, eff), {}).setdefault(task_id, []).append(s)

for (model, eff), tasks in agg.items():
    per_task = []
    cost = 0.0
    n_runs = 0
    tot = 0.0
    dur = 0.0
    all_durs: list[float] = []
    for runs in tasks.values():
        wins = sum(1 for r in runs if r["scores"].get("functional") == 1.0)
        per_task.append(wins / len(runs))
        cost += sum(r.get("cost_usd") or 0 for r in runs)
        tot += sum(r["scores"].get("total") or 0 for r in runs)
        dur += sum(r.get("duration_s") or 0 for r in runs)
        all_durs.extend(r.get("duration_s") or 0 for r in runs)
        n_runs += len(runs)
    n = len(per_task)
    p1 = sum(per_task) / n
    var = sum((x - p1) ** 2 for x in per_task) / (n - 1)
    best[(model, eff)] = dict(
        model=model,
        effort=eff,
        n_tasks=n,
        n_runs=n_runs,
        pass1=round(p1, 4),
        se=round(math.sqrt(var / n), 4),
        avg_total=round(tot / n_runs, 4),
        cost=round(cost, 4),
        # Single-pass columns joined via Reports 17/18 publish MEDIAN times
        # (long-tail timeouts distort a 23-run mean); keep the board consistent.
        avg_duration_s=round(
            statistics.median(all_durs) if model == "zai:glm-5.3" else dur / n_runs,
            1,
        ),
        fin="",
    )

# Claude Opus 5 on v3, from vulcanbench.com Report 10 (2026-07-26, single
# runs; raw run dirs not in this checkout). se = sqrt(p(1-p)/(n-1)) to match
# the harness's single-run pass_at_1_stderr.
for eff, p1, cost, dur_s in (
    ("low", 20 / 23, 14.07, 312.0),
    ("medium", 19 / 23, 24.09, 456.0),
    ("high", 18 / 23, 43.60, 816.0),
):
    best[("anthropic:claude-opus-5", eff)] = dict(
        model="anthropic:claude-opus-5",
        effort=eff,
        n_tasks=23,
        n_runs=23,
        pass1=round(p1, 4),
        se=round(math.sqrt(p1 * (1 - p1) / 22), 4),
        avg_total=None,
        cost=cost,
        avg_duration_s=dur_s,
        fin="2026-07-26",
    )

# Qwen3.8-27B on v3, from Report 17 / the deployed board. The local runs dir
# holds extra non-column sweeps (68/37/22 runs per effort), so the published
# column values are authoritative here; times are medians per that report.
for eff, p1, n_runs, cost_run, med_min in (
    ("low", 0.768, 69, 0.71, 7.1),
    ("medium", 0.826, 23, 0.48, 6.8),
    ("extra-high", 0.739, 23, 0.69, 17.1),
):
    best[("qwen:qwen3.8-27b", eff)] = dict(
        model="qwen:qwen3.8-27b",
        effort=eff,
        n_tasks=23,
        n_runs=n_runs,
        pass1=p1,
        se=0.081 if eff == "medium" else round(math.sqrt(p1 * (1 - p1) / 22), 4),
        avg_total=None,
        cost=round(cost_run * n_runs, 4),
        avg_duration_s=round(med_min * 60.0, 1),
        fin="2026-08-21",
    )

rows = sorted(best.values(), key=lambda r: -r["pass1"])
for r in rows:
    se = f"±{r['se']:.3f}" if r.get("se") is not None else ""
    print(
        f"{r['model']:32} {r['effort']:10} pass@1={r['pass1']:.4f}{se} "
        f"runs={r['n_runs']}/{r['n_tasks']}t cost=${(r['cost'] or 0):.2f}"
    )
with open(HERE / "v3_rankings.json", "w") as f:
    json.dump(rows, f, indent=1)
