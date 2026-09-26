"""Claude Opus 5.5, GPT-6 Astra and Claude Fable 5.1 on VulcanBench Frontier v4.

One card: combined score and cost per task at every effort level, and a table
with tasks passed and the refusal-fallback share for each Claude column.

Sources, nothing re-judged:

- GPT-6 Astra and Claude Fable 5.1: the published Code quality v3.4 rows and
  their cost ledger, loaded by ``load_judged`` below, copied from the
  loader that published them.
- Claude Opus 5.5: the Code quality v3.15 summary and manifest. Cost per task
  is Claude Code's own list-price total (``cli_reported_cost_usd``), because
  the harness's api_equivalent_cost_usd omits fallback-model usage. High
  depotcore is not judged (empty patch after a classifier stop): it counts in
  tasks passed, runtime and cost, and the high combined score is over 22.

Both Claude columns ran with Claude Code's refusal fallback on, and both are
counted as run (the Artificial Analysis convention, docs/DECISIONS.md
2026-09-24). The card publishes each level's share of replies written by
claude-opus-4-8, counted from each run's stream. Astra ran in Codex, which has
no refusal fallback.

    python scripts/cii-v4-board/make_opus55_astra_fable_card.py
"""

from __future__ import annotations

import collections
import csv
import importlib.util
import json
import math
import statistics
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness.evaluator.reviewed_score import WEIGHTS_V3  # noqa: E402
from harness.retrospective_judging import LEVELS, digest, save  # noqa: E402


def _load(name, file):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(file))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


family = _load("openai_family_cards", "make_openai_family_cards.py")
JUDGED = ROOT / "runs-code-quality-maintenance-v3.4"
LEDGER = ROOT / "docs/results/swe-v4-astra-fable51-2026-09/api-equivalent-costs.json"

OPUS_RUN = ROOT / "runs-code-quality-maintenance-v3.15"
OUTPUT = ROOT / "docs/results/swe-v4-opus55-2026-09"
MODELS = ("opus55", "astra", "fable")
NAMES = {"opus55": "Claude Opus 5.5", "astra": "GPT-6 Astra", "fable": "Claude Fable 5.1"}
HARNESS = {"opus55": "Claude Code 2.1.280", "astra": "Codex", "fable": "Claude Code 2.1.259 to 2.1.261"}
# Validated with the dataviz palette checker on #f7f5f0: CVD separation sits in
# the 6 to 8 floor band and contrast is under 3:1, both legal only with
# secondary encoding, which the distinct markers, direct labels and table give.
COLORS = {"opus55": "#8C3A1F", "astra": "#10A37F", "fable": "#D97757"}
MARKERS = {"opus55": "D", "astra": "o", "fable": "s"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
CLAUDE_MAIN = {"opus55": "claude-opus-5-5", "fable": "claude-fable-5-1"}


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def mean_se(values):
    values = [v for v in values if v is not None]
    if not values:
        return {"n": 0, "mean": None, "se": None}
    se = statistics.stdev(values) / math.sqrt(len(values)) if len(values) > 1 else 0.0
    return {"n": len(values), "mean": statistics.mean(values), "se": se}


def replies(run_dir: Path) -> collections.Counter:
    counts = collections.Counter()
    for line in (run_dir / "cli-agent-stream.jsonl").open(errors="replace"):
        if '"assistant"' not in line:
            continue
        event = json.loads(line)
        if event.get("type") == "assistant":
            counts[event["message"].get("model")] += 1
    return counts


def load_judged():
    """Astra and Fable: published v3.4 rows with their ledger costs (the loader that published them)."""
    summary = json.loads((JUDGED / "summary.json").read_text())
    protocol = json.loads((JUDGED / "protocol.json").read_text())
    require(summary["protocol"] == "code-quality-maintenance-v3.4", "wrong protocol")
    require(summary["ready_for_publication"], "v3.4 summary not final")
    require(set(summary["passing_panels"]) == set(family.PANELS), "v3.4 panels")
    require(
        protocol["weights"]["functional"] == WEIGHTS_V3["functional"]
        and protocol["weights"]["code_quality"]["total"] == WEIGHTS_V3["human_like"],
        "weights drift",
    )
    manifest = {r["id"]: r for r in json.loads((JUDGED / "private-manifest.json").read_text())}
    ledger = json.loads(LEDGER.read_text())
    costs = {
        (r["model"], r["run_id"]): (r["estimated_usd"], r.get("long_context_upper_usd"))
        for r in ledger["rows"]
    }
    rows = []
    for entry in summary["rows"]:
        if entry["model"] not in ("astra", "fable") or not entry.get("published"):
            continue
        run = manifest[entry["id"]]
        cq, _, _ = family.code_quality(entry)
        receipt = run["solver_receipt"]
        require(type(receipt.get("raw_tokens")) is int, f"no receipt for {run['run_id']}")
        usd, upper = costs[entry["model"], run["run_id"]]
        rows.append(
            {
                "model": entry["model"],
                "effort": entry["effort"],
                "task": entry["task"],
                "run_id": run["run_id"],
                "functional": run["functional"],
                "minutes": run["duration_s"] / 60,
                "code_quality": cq,
                "combined": family.composite(run, cq, WEIGHTS_V3["human_like"]),
                "usd": usd,
                "usd_upper": upper if upper is not None else usd,
                "tokens": receipt["raw_tokens"],
            }
        )
    hashes = {
        "summary": digest((JUDGED / "summary.json").read_bytes()),
        "protocol": digest((JUDGED / "protocol.json").read_bytes()),
        "ledger": digest(LEDGER.read_bytes()),
    }
    return rows, hashes, ledger


def load_opus55():
    summary = json.loads((OPUS_RUN / "summary.json").read_text())
    protocol = json.loads((OPUS_RUN / "protocol.json").read_text())
    require(summary["protocol"] == "code-quality-maintenance-v3.15", "wrong Opus 5.5 protocol")
    require(summary["ready_for_publication"], "v3.15 summary not final")
    require(set(summary["passing_panels"]) == set(family.PANELS), "v3.15 panels")
    require(protocol["weights"]["code_quality"]["total"] == WEIGHTS_V3["human_like"], "weights drift")
    manifest = {r["id"]: r for r in json.loads((OPUS_RUN / "private-manifest.json").read_text())}
    rows = []
    for entry in summary["rows"]:
        run = manifest[entry["id"]]
        cq, _, _ = family.code_quality(entry)
        src = Path(run["source_directory"])
        s = json.loads((src / "summary.json").read_text())
        rows.append({"model": "opus55", "effort": entry["effort"], "task": entry["task"], "run_dir": src,
                     "functional": run["functional"], "minutes": run["duration_s"] / 60, "code_quality": cq,
                     "combined": family.composite(run, cq, WEIGHTS_V3["human_like"]),
                     "usd": s["economics"]["cli_reported_cost_usd"]})
    comparison = json.loads((ROOT / "docs/results/swe-v4-opus55-2026-09/comparison.json").read_text())
    for ex in comparison["excluded"]:  # not judged, but it ran: counts in passed, time and cost
        src = ROOT / "runs-effort-opus55" / ex["effort"] / ex["run_id"]
        s = json.loads((src / "summary.json").read_text())
        rows.append({"model": "opus55", "effort": ex["effort"], "task": ex["task"], "run_dir": src,
                     "functional": s["scores"]["functional"], "minutes": s["duration_s"] / 60,
                     "code_quality": None, "combined": None, "usd": s["economics"]["cli_reported_cost_usd"]})
    return rows, {"summary": digest((OPUS_RUN / "summary.json").read_bytes()),
                  "protocol": digest((OPUS_RUN / "protocol.json").read_bytes())}


def main():  # noqa: PLR0915, one linear figure
    judged, v34_hashes, _ledger = load_judged()
    man34 = {r["run_id"]: r for r in json.loads((JUDGED / "private-manifest.json").read_text())}
    for r in judged:
        r["run_dir"] = Path(man34[r["run_id"]]["source_directory"])
    opus, v315_hashes = load_opus55()
    rows = judged + opus

    groups = {}
    for m in MODELS:
        for e in LEVELS:
            rs = [r for r in rows if r["model"] == m and r["effort"] == e]
            require(len(rs) == 23, f"{m} {e}: {len(rs)} rows")
            g = {"n": len(rs), "combined": mean_se(r["combined"] for r in rs),
                 "code_quality": mean_se(r["code_quality"] for r in rs),
                 "passed": sum(r["functional"] == 1 for r in rs), "usd": mean_se(r["usd"] for r in rs),
                 "minutes": mean_se(r["minutes"] for r in rs), "fb_runs": 0, "fb_share": None}
            if m in CLAUDE_MAIN:
                counts = [replies(r["run_dir"]) for r in rs]
                other = [sum(v for k, v in c.items() if k not in (CLAUDE_MAIN[m], "<synthetic>")) for c in counts]
                g["fb_runs"] = sum(o > 0 for o in other)
                g["fb_share"] = 100 * sum(other) / max(1, sum(sum(c.values()) for c in counts))
            groups[m, e] = g

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 15.9
    fig = plt.figure(figsize=(W, H), dpi=150, facecolor=PAPER)
    yf = lambda i: 1 - i / H  # noqa: E731

    def text(x, y, s, size=13, bold=False, heading=False, numeric=False, ha="left", color=INK):
        fam = "IBM Plex Mono" if numeric else ("Chakra Petch SemiBold" if bold else "Chakra Petch Medium") if heading else "Geist"
        wt = (500 if bold else 400) if numeric else ((600 if bold else 500) if heading else (700 if bold else 400))
        return fig.text(x, yf(y), s, fontsize=size, fontfamily=fam, weight=wt, ha=ha, va="center", color=color)

    def line(x1, x2, y, color=RULE, width=0.8):
        fig.add_artist(plt.Line2D([x1, x2], [yf(y), yf(y)], transform=fig.transFigure, color=color, lw=width))

    left, right = 0.06, 0.94
    logo = fig.add_axes([left, yf(0.86), 0.42 / W, 0.42 / H])
    mark = logo.imshow(plt.imread(ROOT / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(FancyBboxPatch((0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes))
    logo.axis("off")
    text(left + 0.038, 0.65, "VulcanBench", 20, True, heading=True)
    text(right, 0.65, "September 2026", 14, ha="right", color=MUTED)
    line(left, right, 1.05, INK, 1.2)

    best = {m: max(LEVELS, key=lambda e: groups[m, e]["combined"]["mean"]) for m in MODELS}
    text(left, 1.72, "VulcanBench Frontier v4: Claude Opus 5.5, GPT-6 Astra and Claude Fable 5.1", 30, True, heading=True)
    order = sorted(MODELS, key=lambda m: -groups[m, best[m]]["combined"]["mean"])
    head = "  ·  ".join(f"{NAMES[m]} {groups[m, best[m]]['combined']['mean']:.2f} at {best[m].replace('-', ' ')}" for m in order)
    text(left, 2.22, f"Best combined score per model: {head}.", 15)
    text(left, 2.60, "23 hard legacy-reconstruction tasks per effort level. Code quality judged by Muse Spark 1.3 and Grok 4.6.", 13.5, color=MUTED)

    x = left
    for m in MODELS:
        fig.add_artist(plt.Line2D([x + 0.004], [yf(3.05)], transform=fig.transFigure, marker=MARKERS[m], color=COLORS[m],
                                  markersize=9, markeredgecolor=INK, markeredgewidth=0.6, linestyle="none"))
        label = f"{NAMES[m]}{' (with fallback)' if m in CLAUDE_MAIN else ''}"
        text(x + 0.016, 3.05, label, 13, True)
        x += {"opus55": 0.30, "astra": 0.16, "fable": 0}[m]

    def axis(x0, w, top, h):
        ax = fig.add_axes([x0, yf(top + h), w, h / H], facecolor=PAPER)
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.tick_params(axis="x", length=0, labelsize=12, pad=8)
        ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
        ax.grid(axis="y", color=RULE, linewidth=0.5)
        ax.set_axisbelow(True)
        ax.set_xticks(range(len(LEVELS)), [e.replace("-", " ").capitalize() for e in LEVELS])
        return ax

    text(left, 3.62, "Combined score", 20, True, heading=True)
    text(left, 3.94, "/100  ·  higher is better  ·  focused scale  ·  whiskers +/-1 se", 12, color=MUTED)
    ax = axis(0.085, 0.40, 4.25, 3.2)
    vals = [groups[m, e]["combined"] for m in MODELS for e in LEVELS]
    lo = math.floor(min(v["mean"] - v["se"] for v in vals)) - 1
    hi = math.ceil(max(v["mean"] + v["se"] for v in vals)) + 1
    ax.set_ylim(lo, hi)
    ax.set_xlim(-0.35, len(LEVELS) - 0.35)
    for m in MODELS:
        ys = [groups[m, e]["combined"]["mean"] for e in LEVELS]
        es = [groups[m, e]["combined"]["se"] for e in LEVELS]
        ax.plot(range(5), ys, color=COLORS[m], lw=2, zorder=2)
        ax.errorbar(range(5), ys, yerr=es, fmt=MARKERS[m], color=COLORS[m], markersize=8, markeredgecolor=INK,
                    markeredgewidth=0.6, ecolor=INK, elinewidth=0.8, capsize=3, zorder=3)
        ax.annotate(NAMES[m].replace("Claude ", "").replace("GPT-6 ", ""), (4, ys[-1]), xytext=(9, 0),
                    textcoords="offset points", va="center", fontsize=10.5, color=INK, fontfamily="Geist", weight=700)

    text(0.56, 3.62, "Cost per task", 20, True, heading=True)
    text(0.56, 3.94, "USD, API-equivalent at list prices  ·  every serving model included", 12, color=MUTED)
    ax = axis(0.585, 0.355, 4.25, 3.2)
    shifts = {"opus55": -0.26, "astra": 0.0, "fable": 0.26}
    top = max(groups[m, e]["usd"]["mean"] for m in MODELS for e in LEVELS)
    ax.set_ylim(0, top * 1.22)
    for m in MODELS:
        ys = [groups[m, e]["usd"]["mean"] for e in LEVELS]
        ax.bar([i + shifts[m] for i in range(5)], ys, width=0.25, color=COLORS[m], edgecolor=INK, linewidth=0.5, zorder=2)
        for i, y in enumerate(ys):
            ax.annotate(f"{y:.1f}", (i + shifts[m], y), xytext=(0, 3), textcoords="offset points", ha="center",
                        va="bottom", fontsize=8.5, fontfamily="IBM Plex Mono", color=INK)

    ty = 8.35
    text(left, ty, "Table 1  |  Every effort level", 14.5, heading=True)
    cols = dict(zip(LEVELS, (0.54, 0.64, 0.74, 0.84, 0.94), strict=True))
    line(left, right, ty + 0.28, INK, 1.2)
    text(left, ty + 0.52, "Model and measure", 12.5, True)
    for e in LEVELS:
        text(cols[e], ty + 0.52, e.replace("-", " ").capitalize(), 12.5, True, ha="right")
    line(left, right, ty + 0.72, INK, 0.6)
    y = ty + 0.98
    for m in MODELS:
        text(left, y, NAMES[m], 13, True)
        fig.add_artist(plt.Line2D([left - 0.012], [yf(y)], transform=fig.transFigure, marker=MARKERS[m], color=COLORS[m],
                                  markersize=7, markeredgecolor=INK, markeredgewidth=0.5, linestyle="none"))
        measures = [("Combined score", lambda g: f"{g['combined']['mean']:.2f}" + ("*" if g["combined"]["n"] < 23 else ""), True),
                    ("Tasks passed of 23", lambda g: str(g["passed"]), False),
                    ("Cost per task, USD", lambda g: f"{g['usd']['mean']:.2f}", False)]
        if m in CLAUDE_MAIN:
            measures.append(("Opus 4.8 share of replies (runs)", lambda g: f"{g['fb_share']:.1f}% ({g['fb_runs']})", False))
        for label, fn, strong in measures:
            y += 0.30
            text(left + 0.012, y, label, 12, color=INK if strong else MUTED)
            for e in LEVELS:
                g = groups[m, e]
                emph = strong and e == best[m]
                text(cols[e], y, fn(g), 13 if strong else 12, emph, numeric=True, ha="right", color=INK)
        y += 0.40
        line(left, right, y - 0.2, RULE, 0.6)
    line(left, right, y - 0.2, INK, 1.2)

    notes = [
        "* Opus 5.5 high: combined over the 22 judged runs. On depotcore a safeguard classifier stop cut off the turn that would have "
        "written the module, so there is no code to review; the run counts in tasks passed and cost. Bold marks each model's best level.",
        "Protocols: Astra and Fable 5.1 are the published Code quality v3.4 rows; Opus 5.5 is v3.15. Same rubric, controls, weights and "
        "pinned judges, judged in separate sessions.",
        "Both Claude columns ran with Claude Code's refusal fallback on and count every run, following Artificial Analysis ('Default "
        "Fallback'). The share row counts replies written by claude-opus-4-8. Astra ran in Codex, which has no refusal fallback.",
        "Cost: Astra and Fable 5.1 from the published per-model cost ledger; Opus 5.5 from Claude Code's own list-price totals. Both "
        "Claude bases include the fallback model's usage. Opus 5.5 ran on a newer Claude Code than Fable 5.1, so small gaps between "
        "them are harness confounded (Opus 5.5 on 2.1.280, Fable 5.1 on 2.1.259 to 2.1.261).",
    ]
    import textwrap
    yy = y + 0.12
    for n in notes:
        for w in textwrap.wrap(n, 200):
            text(left, yy, w, 10.5, color=MUTED)
            yy += 0.24
        yy += 0.05

    OUTPUT.mkdir(parents=True, exist_ok=True)
    out = OUTPUT / "opus55-astra-fable.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / "opus55-astra-fable-efforts.csv"
    with table.open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(["model", "effort", "n", "judged", "combined", "combined_se", "code_quality", "passed",
                    "usd_per_task", "minutes", "opus48_reply_share_pct", "fallback_runs"])
        for (m, e), g in groups.items():
            w.writerow([m, e, g["n"], g["combined"]["n"], f"{g['combined']['mean']:.4f}", f"{g['combined']['se']:.4f}",
                        f"{g['code_quality']['mean']:.4f}", g["passed"], f"{g['usd']['mean']:.4f}",
                        f"{g['minutes']['mean']:.4f}", "" if g["fb_share"] is None else f"{g['fb_share']:.2f}", g["fb_runs"]])
    save(out.with_suffix(".json"), {"sources": {"v3.4": v34_hashes, "v3.15": v315_hashes}, "best": best,
                                    "png_sha256": digest(out.read_bytes()), "table": table.name})
    print(out)
    for m in order:
        print(f"  {NAMES[m]:<18} best {best[m]:<10} {groups[m, best[m]]['combined']['mean']:.2f}")


if __name__ == "__main__":
    main()
