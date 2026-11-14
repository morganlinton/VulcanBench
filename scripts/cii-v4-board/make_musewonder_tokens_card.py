"""Muse Wonder Full, GPT-6.1 Sol and Claude Opus 5.5: token use at Low on VulcanBench Frontier v4.

One card: mean tokens per task with +/-1 se whiskers, every shared task as a
dot plot, and a table with raw and effective tokens, cache-read share,
functional score, tasks passed and minutes.

Tokens are compared in one unit, "effective tokens": uncached input + 0.1 x
cache reads + 1.25 x cache writes + output. That is how the Claude Code adapter
stored Opus 5.5's totals (harness/agent/cli_agents.py, _fold_usage), and the
raw Opus counts were not kept. Muse and Codex record raw input with cached
input included, so their effective totals are rebuilt from the cached-token
count in each run's receipt.

Sources, nothing re-run or re-judged:

- Muse Wonder Full: run summaries in runs-effort-musewonder-full/low (bench
  tag bench/2026-10-13-musewonder-full-frontier-v4). The sweep is still
  running, so the card is preliminary until all 23 Low tasks finish.
- GPT-6.1 Sol and Claude Opus 5.5: the published per-task rows in
  docs/results/swe-v4-gpt61-sol-2026-09/comparison.json and
  docs/results/swe-v4-opus55-2026-09/comparison.json.

Only tasks present for all three models are compared.

    python scripts/cii-v4-board/make_musewonder_tokens_card.py
"""

from __future__ import annotations

import csv
import json
import math
import statistics
import sys
import textwrap
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch
from matplotlib.ticker import FuncFormatter

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness.retrospective_judging import digest, save  # noqa: E402

WONDER = ROOT / "runs-effort-musewonder-full/low"
SOL = ROOT / "docs/results/swe-v4-gpt61-sol-2026-09/comparison.json"
OPUS = ROOT / "docs/results/swe-v4-opus55-2026-09/comparison.json"
OUTPUT = ROOT / "docs/results/swe-v4-musewonder-full-2026-10"
SUITE_TASKS = 23
MODELS = ("wonder", "gpt61sol", "opus55")
NAMES = {"wonder": "Muse Wonder Full", "gpt61sol": "GPT-6.1 Sol", "opus55": "Claude Opus 5.5"}
HARNESS = {
    "wonder": "Muse Code 1.4.6, Contributor",
    "gpt61sol": "Codex 0.159.0",
    "opus55": "Claude Code 2.1.280",
}
# Lab colors from CLAUDE.md (Meta blue from scripts/rankings-chart/_common.py).
# The palette validator passes all checks; clay vs green sits in the 6 to 8 CVD
# band, so markers, direct labels and the table carry identity as well.
COLORS = {"wonder": "#0866ff", "gpt61sol": "#10A37F", "opus55": "#D97757"}
MARKERS = {"wonder": "o", "gpt61sol": "^", "opus55": "D"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
CACHE_READ, CACHE_WRITE = 0.1, 1.25


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def effective(raw, cached, written=0):
    """Raw tokens (cached input included) folded to Claude Code's effective unit."""
    return raw - cached - written + CACHE_READ * cached + CACHE_WRITE * written


def load_wonder():
    rows = {}
    for path in sorted(WONDER.glob("*/summary.json")):
        r = json.loads(path.read_text())
        require(r["model"] == "muse-code:muse-wonder-full-1.0-contributor", f"model {r['model']}")
        require(r["effort"]["requested"] == "low", f"effort {r['effort']}")
        require(r["task_id"] not in rows, f"duplicate {r['task_id']}")
        cli = r["cli_agent"]
        raw, cached = r["total_tokens"], cli["cached_input_tokens"]
        require(0 <= cached <= raw, f"cached tokens out of range for {r['task_id']}")
        rows[r["task_id"]] = {
            "raw": raw,
            "cached": cached,
            "effective": effective(raw, cached),
            "functional": r["scores"]["functional"],
            "minutes": r["duration_s"] / 60,
        }
    return rows


def load_published(path, model):
    rows = {}
    for r in json.loads(path.read_text())["rows"]:
        if r["model"] != model or r["effort"] != "low":
            continue
        task = r["run_id"].rsplit("-", 1)[0]
        require(task not in rows, f"duplicate {model} {task}")
        usage = (r.get("solver_receipt") or {}).get("usage")
        row = {"functional": r["functional"], "minutes": r["duration_s"] / 60}
        if usage:
            # Codex: reported tokens are raw, cached input included.
            cached = usage["cached_input_tokens"]
            written = usage.get("cache_write_input_tokens", 0)
            require(r["reported_tokens"] == usage["input_tokens"] + usage["output_tokens"], task)
            row |= {
                "raw": r["reported_tokens"],
                "cached": cached,
                "effective": effective(r["reported_tokens"], cached, written),
            }
        else:
            # Claude Code stored effective tokens only; raw counts were not kept.
            row |= {"raw": None, "cached": None, "effective": r["reported_tokens"]}
        rows[task] = row
    require(len(rows) == SUITE_TASKS, f"{model} has {len(rows)} Low rows")
    return rows


def mean_se(values):
    se = statistics.stdev(values) / math.sqrt(len(values)) if len(values) > 1 else 0.0
    return statistics.mean(values), se


def compact(n):
    return f"{n / 1e6:.1f}M" if n >= 1e6 else f"{n / 1e3:.0f}k"


def main():  # noqa: PLR0912, PLR0915, one linear figure
    data = {"wonder": load_wonder(), "gpt61sol": load_published(SOL, "gpt61sol")}
    data["opus55"] = load_published(OPUS, "opus55")
    common = sorted(set.intersection(*(set(d) for d in data.values())))
    require(common, "no shared tasks")
    n = len(common)
    final = n == SUITE_TASKS
    stats = {}
    for m in MODELS:
        rows = [data[m][t] for t in common]
        eff_mean, eff_se = mean_se([r["effective"] for r in rows])
        raw = [r["raw"] for r in rows]
        stats[m] = {
            "eff_mean": eff_mean,
            "eff_se": eff_se,
            "eff_median": statistics.median(r["effective"] for r in rows),
            "eff_total": sum(r["effective"] for r in rows),
            "raw_mean": None if None in raw else statistics.mean(raw),
            "cached_share": None if None in raw else sum(r["cached"] for r in rows) / sum(raw),
            "functional": statistics.mean(r["functional"] for r in rows),
            "passed": sum(r["functional"] >= 1 for r in rows),
            "minutes": statistics.mean(r["minutes"] for r in rows),
        }
    ratio = {m: stats["wonder"]["eff_total"] / stats[m]["eff_total"] for m in MODELS}
    heaviest = sum(
        all(data["wonder"][t]["effective"] > data[m][t]["effective"] for m in MODELS[1:])
        for t in common
    )

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 14.75
    fig = plt.figure(figsize=(W, H), dpi=150, facecolor=PAPER)
    yf = lambda i: 1 - i / H  # noqa: E731

    def text(x, y, s, size=13, bold=False, heading=False, numeric=False, ha="left", color=INK):
        fam = (
            "IBM Plex Mono"
            if numeric
            else ("Chakra Petch SemiBold" if bold else "Chakra Petch Medium")
            if heading
            else "Geist"
        )
        wt = (
            (500 if bold else 400)
            if numeric
            else ((600 if bold else 500) if heading else (700 if bold else 400))
        )
        return fig.text(
            x, yf(y), s, fontsize=size, fontfamily=fam, weight=wt, ha=ha, va="center", color=color
        )

    def line(x1, x2, y, color=RULE, width=0.8):
        fig.add_artist(
            plt.Line2D([x1, x2], [yf(y), yf(y)], transform=fig.transFigure, color=color, lw=width)
        )

    def swatch(x, y, m, size=9):
        fig.add_artist(
            plt.Line2D(
                [x],
                [yf(y)],
                transform=fig.transFigure,
                marker=MARKERS[m],
                color=COLORS[m],
                markersize=size,
                markeredgecolor=INK,
                markeredgewidth=0.6,
                linestyle="none",
            )
        )

    left, right = 0.06, 0.94
    logo = fig.add_axes([left, yf(0.86), 0.42 / W, 0.42 / H])
    mark = logo.imshow(plt.imread(ROOT / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    text(left + 0.038, 0.65, "VulcanBench", 20, True, heading=True)
    text(right, 0.65, "October 2026", 14, ha="right", color=MUTED)
    line(left, right, 1.05, INK, 1.2)

    text(left, 1.72, "VulcanBench Frontier v4: token use at Low effort", 30, True, heading=True)
    text(
        left,
        2.22,
        f"Muse Wonder Full used {ratio['gpt61sol']:.1f}x GPT-6.1 Sol's tokens and "
        f"{ratio['opus55']:.1f}x Claude Opus 5.5's on the same {n} tasks, and the most on "
        f"{heaviest} of {n}.",
        15,
    )
    text(
        left,
        2.60,
        ("" if final else f"Preliminary: {n} of {SUITE_TASKS} tasks so far.  ")
        + "Hard legacy-reconstruction tasks, one run per task. Tokens in one unit: "
        "uncached input + 0.1 x cache reads + output (notes).",
        13.5,
        color=MUTED,
    )
    x = left
    for m in MODELS:
        swatch(x + 0.004, 3.05, m)
        text(x + 0.016, 3.05, NAMES[m], 13, True)
        x += 0.22

    text(left, 3.62, "Mean tokens per task", 20, True, heading=True)
    text(
        left,
        3.94,
        f"effective tokens  ·  lower is leaner  ·  whiskers +/-1 se  ·  n={n} each",
        12,
        color=MUTED,
    )
    ax = fig.add_axes([0.13, yf(7.35), 0.33, 3.1 / H], facecolor=PAPER)
    order = sorted(MODELS, key=lambda m: stats[m]["eff_mean"])
    ys = range(len(order))
    for i, m in zip(ys, order, strict=True):
        s = stats[m]
        ax.barh(
            i, s["eff_mean"], height=0.5, color=COLORS[m], edgecolor=INK, linewidth=0.5, zorder=2
        )
        ax.errorbar(
            s["eff_mean"],
            i,
            xerr=s["eff_se"],
            fmt="none",
            ecolor=INK,
            elinewidth=0.9,
            capsize=4,
            zorder=3,
        )
        ax.annotate(
            f"{compact(s['eff_mean'])}  n={n}",
            (s["eff_mean"] + s["eff_se"], i),
            xytext=(8, 0),
            textcoords="offset points",
            va="center",
            fontsize=11,
            fontfamily="IBM Plex Mono",
            color=INK,
        )
    ax.set_yticks(list(ys), [NAMES[m] for m in order])
    ax.set_xlim(0, max(stats[m]["eff_mean"] + stats[m]["eff_se"] for m in MODELS) * 1.45)
    ax.xaxis.set_major_formatter(FuncFormatter(lambda v, _: compact(v) if v else "0"))
    ax.spines[["top", "right"]].set_visible(False)
    ax.spines[["left", "bottom"]].set_color(RULE)
    ax.tick_params(axis="y", length=0, labelsize=12.5, pad=8)
    ax.tick_params(axis="x", length=0, labelsize=10.5, pad=6)
    ax.grid(axis="x", color=RULE, linewidth=0.5)
    ax.set_axisbelow(True)

    text(0.56, 3.62, "Every shared task", 20, True, heading=True)
    text(
        0.56,
        3.94,
        "effective tokens per task  ·  log scale  ·  sorted by Muse Wonder Full",
        12,
        color=MUTED,
    )
    ax = fig.add_axes([0.70, yf(7.35), 0.24, 3.1 / H], facecolor=PAPER)
    tasks = sorted(common, key=lambda t: data["wonder"][t]["effective"])
    for i, t in enumerate(tasks):
        vals = [data[m][t]["effective"] for m in MODELS]
        ax.plot([min(vals), max(vals)], [i, i], color=RULE, lw=1, zorder=1)
        for m in MODELS:
            ax.plot(
                data[m][t]["effective"],
                i,
                MARKERS[m],
                color=COLORS[m],
                markersize=6.5,
                markeredgecolor=INK,
                markeredgewidth=0.4,
                zorder=3,
            )
    ax.set_xscale("log")
    ax.xaxis.set_major_formatter(FuncFormatter(lambda v, _: compact(v)))
    ax.set_yticks(
        range(len(tasks)),
        [
            t.removeprefix("legacy-").replace("-binary-parity", "").replace("-parity", "")
            for t in tasks
        ],
    )
    ax.set_ylim(-0.7, len(tasks) - 0.3)
    ax.spines[["top", "right"]].set_visible(False)
    ax.spines[["left", "bottom"]].set_color(RULE)
    ax.tick_params(axis="y", length=0, labelsize=9, pad=4)
    ax.tick_params(axis="x", length=0, labelsize=10, pad=6)
    ax.grid(axis="x", which="major", color=RULE, linewidth=0.5)
    ax.set_axisbelow(True)

    ty = 8.15
    star = "" if final else "*"
    text(left, ty, f"Table 1  |  Low effort, the same {n} tasks{star}", 14.5, heading=True)
    cols = dict(zip(MODELS, (0.62, 0.78, 0.94), strict=True))
    line(left, right, ty + 0.28, INK, 1.2)
    text(left, ty + 0.52, "Measure", 12.5, True)
    for m in MODELS:
        swatch(cols[m] - 0.135, ty + 0.52, m, 7)
        text(cols[m], ty + 0.52, NAMES[m], 12.5, True, ha="right")
    line(left, right, ty + 0.72, INK, 0.6)
    measures = [
        (
            "Effective tokens per task, mean (+/-1 se)",
            lambda s: f"{compact(s['eff_mean'])} ({compact(s['eff_se'])})",
            True,
        ),
        ("Effective tokens per task, median", lambda s: compact(s["eff_median"]), False),
        (
            "Muse Wonder Full used this many times as many",
            lambda s: (
                "-"
                if s is stats["wonder"]
                else f"{stats['wonder']['eff_total'] / s['eff_total']:.1f}x"
            ),
            True,
        ),
        (
            "Raw tokens per task, mean",
            lambda s: "not kept" if s["raw_mean"] is None else compact(s["raw_mean"]),
            False,
        ),
        (
            "Cache reads, share of raw tokens",
            lambda s: (
                "not kept" if s["cached_share"] is None else f"{100 * s['cached_share']:.0f}%"
            ),
            False,
        ),
        ("Functional score, mean", lambda s: f"{s['functional']:.3f}", False),
        (f"Tasks passed, of {n}", lambda s: str(s["passed"]), False),
        ("Minutes per task, mean", lambda s: f"{s['minutes']:.1f}", False),
        ("Harness", None, False),
    ]
    y = ty + 0.98
    for label, fn, strong in measures:
        text(left, y, label, 12.5, color=INK if strong else MUTED)
        for m in MODELS:
            value = HARNESS[m] if fn is None else fn(stats[m])
            text(cols[m], y, value, 12.5 if fn else 11, strong, numeric=fn is not None, ha="right")
        y += 0.32
    line(left, right, y - 0.14, INK, 1.2)

    notes = [
        (
            f"* Preliminary: Muse Wonder Full has finished {n} of {SUITE_TASKS} Low tasks; its sweep paused on the Muse Code "
            "weekly quota and resumes after it resets. Only tasks all three finished are compared."
            if not final
            else f"All {SUITE_TASKS} Low tasks for all three models."
        ),
        "Effective tokens = uncached input + 0.1 x cache reads + 1.25 x cache writes + output. Claude Code stored Opus 5.5's "
        "totals only in this unit and its raw counts were not kept; Muse and Codex record raw counts, rebuilt here from each "
        "run's cached-token receipt. The weights follow Anthropic's cache pricing: a common unit, not each vendor's price. "
        f"In raw tokens, Muse Wonder Full used {stats['wonder']['raw_mean'] / stats['gpt61sol']['raw_mean']:.1f}x as many as GPT-6.1 Sol.",
        "Muse Wonder Full ran in Contributor mode through Muse Code 1.4.6 (pinned binary) on the subscription; it has no API "
        "price, so no cost is shown. GPT-6.1 Sol and Opus 5.5 are their published Frontier v4 rows, not re-run. Harnesses "
        "differ, so tool loops and context handling differ too. Code quality is not judged yet, so no combined score is shown.",
    ]
    yy = y + 0.12
    for note in notes:
        for w in textwrap.wrap(note, 190):
            text(left, yy, w, 10.5, color=MUTED)
            yy += 0.24
        yy += 0.05

    OUTPUT.mkdir(parents=True, exist_ok=True)
    suffix = "" if final else "-preliminary"
    out = OUTPUT / f"musewonder-full-low-tokens{suffix}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"musewonder-full-low-tokens{suffix}-tasks.csv"
    with table.open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(
            [
                "task",
                "model",
                "effective_tokens",
                "raw_tokens",
                "cached_input_tokens",
                "functional",
                "minutes",
            ]
        )
        for t in common:
            for m in MODELS:
                r = data[m][t]
                w.writerow(
                    [
                        t,
                        m,
                        round(r["effective"]),
                        r["raw"] if r["raw"] is not None else "",
                        r["cached"] if r["cached"] is not None else "",
                        r["functional"],
                        f"{r['minutes']:.2f}",
                    ]
                )
    save(
        out.with_suffix(".json"),
        {
            "final": final,
            "tasks": common,
            "stats": stats,
            "wonder_ratio": ratio,
            "wonder_heaviest_tasks": heaviest,
            "unit": "effective tokens: uncached input + 0.1 x cache reads + 1.25 x cache writes + output",
            "sources": {
                "wonder_summaries": {
                    p.parent.name: digest(p.read_bytes())
                    for p in sorted(WONDER.glob("*/summary.json"))
                },
                "published": {
                    str(p.relative_to(ROOT)): digest(p.read_bytes()) for p in (SOL, OPUS)
                },
            },
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
        },
    )
    print(out)
    for m in MODELS:
        s = stats[m]
        print(
            f"  {NAMES[m]:<18} {compact(s['eff_mean']):>6} effective/task  functional {s['functional']:.3f}"
        )


if __name__ == "__main__":
    main()
