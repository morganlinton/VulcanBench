"""Muse Wonder Full beside Claude Opus 5.5, GPT-6.1 Sol, Claude Fable 5.1 and GPT-6 Astra.

One card on VulcanBench Frontier v4 at the two effort levels Muse Wonder Full
has run so far: accuracy (functional score, the share of hidden behaviour
tests passed, mean with +/-1 se whiskers) and cost per task.

Accuracy is the functional score, not the combined score, because Muse Wonder
Full's code quality has not been judged; every model is compared on the same
measure, computed from its per-run rows.

Sources, nothing re-run or re-judged:

- Muse Wonder Full: run summaries in runs-effort-musewonder-full-v2 (Muse Code
  1.4.6, Contributor mode, skill reminder off). Meta publishes no API price
  for Wonder Full, so its cost is an estimate at the Muse Wonder 1.0 list
  price ($4.00 input, $0.20 cached input, $20.00 output per million tokens).
- Claude Opus 5.5 and GPT-6.1 Sol: per-run functional scores from their
  comparison.json; cost and minutes from the frontier leaders card table
  (docs/results/swe-v4-gpt6-vs-gpt56-2026-09/gpt61sol-opus55-astra-efforts.csv).
- Claude Fable 5.1 and GPT-6 Astra: per-run functional scores from
  docs/results/swe-v4-astra-fable51-2026-09/comparison.json; cost and minutes
  from that report's API-equivalent economics table.

Every per-level mean that also appears in a published table must match it, or
the card refuses to draw. A Muse Wonder Full level with fewer than 23 tasks is
marked preliminary.

    python scripts/cii-v4-board/make_musewonder_frontier_card.py
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

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness.retrospective_judging import digest, save  # noqa: E402

RESULTS = ROOT / "docs/results"
WONDER = ROOT / "runs-effort-musewonder-full-v2"
OPUS_SOL = RESULTS / "swe-v4-gpt6-vs-gpt56-2026-09/gpt61sol-opus55-astra-efforts.csv"
ASTRA_FABLE = RESULTS / "swe-v4-astra-fable51-2026-09/astra-vs-fable51-v34-economics-efforts.csv"
ROWS = {
    "opus55": RESULTS / "swe-v4-opus55-2026-09/comparison.json",
    "gpt61sol": RESULTS / "swe-v4-gpt61-sol-2026-09/comparison.json",
    "fable": RESULTS / "swe-v4-astra-fable51-2026-09/comparison.json",
    "astra": RESULTS / "swe-v4-astra-fable51-2026-09/comparison.json",
}
OUTPUT = RESULTS / "swe-v4-musewonder-full-2026-10"
LEVELS = ("low", "medium")
SUITE_TASKS = 23
MODELS = ("wonder", "opus55", "fable", "gpt61sol", "astra")
NAMES = {
    "wonder": "Muse Wonder Full",
    "opus55": "Claude Opus 5.5",
    "fable": "Claude Fable 5.1",
    "gpt61sol": "GPT-6.1 Sol",
    "astra": "GPT-6 Astra",
}
HARNESS = {
    "wonder": "Muse Code 1.4.6",
    "opus55": "Claude Code 2.1.280",
    "fable": "Claude Code",
    "gpt61sol": "Codex 0.159.0",
    "astra": "Codex",
}
# Lab colors (CLAUDE.md): Meta blue; Anthropic clay with a darker clay for the
# second Anthropic model; OpenAI greens as on the frontier leaders card.
# Markers, direct labels and the table carry identity as well as color.
COLORS = {
    "wonder": "#0866FF",
    "opus55": "#D97757",
    "fable": "#8C3A1F",
    "gpt61sol": "#0B3D2E",
    "astra": "#10A37F",
}
MARKERS = {"wonder": "o", "opus55": "s", "fable": "D", "gpt61sol": "^", "astra": "v"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
WONDER_RATES = (4.00, 0.20, 20.00)  # Muse Wonder 1.0 list price, per million tokens


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def mean_se(values):
    se = statistics.stdev(values) / math.sqrt(len(values)) if len(values) > 1 else 0.0
    return statistics.mean(values), se


def load_wonder():
    groups = {}
    for level in LEVELS:
        runs = [json.loads(p.read_text()) for p in sorted((WONDER / level).glob("*/summary.json"))]
        require(runs, f"no Muse Wonder Full runs at {level}")
        tasks = [r["task_id"] for r in runs]
        require(len(tasks) == len(set(tasks)), f"duplicate Muse Wonder Full tasks at {level}")
        usd, minutes, func = [], [], []
        for r in runs:
            require(
                r["model"] == "muse-code:muse-wonder-full-1.0-contributor", f"model {r['model']}"
            )
            require(r["effort"]["requested"] == level, f"effort {r['effort']}")
            cd = r["cost_detail"]
            output = r["total_tokens"] - cd["agent_input_tokens"]
            rin, rcached, rout = WONDER_RATES
            usd.append(
                (
                    cd["agent_uncached_input_tokens"] * rin
                    + cd["agent_cached_input_tokens"] * rcached
                    + output * rout
                )
                / 1e6
            )
            minutes.append(r["duration_s"] / 60)
            func.append(r["scores"]["functional"])
        m, se = mean_se(func)
        groups[level] = {
            "n": len(runs),
            "functional": 100 * m,
            "functional_se": 100 * se,
            "passed": sum(f >= 1 for f in func),
            "usd": statistics.mean(usd),
            "minutes": statistics.mean(minutes),
        }
    return groups


def per_run_functional(model):
    rows = json.loads(ROWS[model].read_text())["rows"]
    key = {"fable": "fable", "astra": "astra"}.get(model, model)
    out = {}
    for level in LEVELS:
        f = [r["functional"] for r in rows if r["model"] == key and r["effort"] == level]
        require(len(f) == SUITE_TASKS, f"{model} {level}: {len(f)} rows")
        out[level] = f
    return out


def load_published():
    pub = {}
    with OPUS_SOL.open() as fh:
        for r in csv.DictReader(fh):
            if r["model"] in ("opus55", "gpt61sol") and r["effort"] in LEVELS:
                pub[r["model"], r["effort"]] = {
                    "usd": float(r["mean_usd"]),
                    "minutes": float(r["mean_minutes"]),
                    "passed": int(r["passed"]),
                }
    with ASTRA_FABLE.open() as fh:
        for r in csv.DictReader(fh):
            if r["effort"] in LEVELS:
                pub[r["model"], r["effort"]] = {
                    "usd": float(r["mean_usd"]),
                    "minutes": float(r["mean_minutes"]),
                    "passed": None,
                }
    return pub


def load():
    groups = {"wonder": load_wonder()}
    pub = load_published()
    for model in MODELS[1:]:
        func = per_run_functional(model)
        groups[model] = {}
        for level in LEVELS:
            p = pub[model, level]
            m, se = mean_se(func[level])
            passed = sum(f >= 1 for f in func[level])
            if p["passed"] is not None:
                require(
                    passed == p["passed"],
                    f"{model} {level}: passed {passed} vs published {p['passed']}",
                )
            groups[model][level] = {
                "n": SUITE_TASKS,
                "functional": 100 * m,
                "functional_se": 100 * se,
                "passed": passed,
                "usd": p["usd"],
                "minutes": p["minutes"],
            }
    sources = {p.name: digest(p.read_bytes()) for p in {OPUS_SOL, ASTRA_FABLE, *ROWS.values()}}
    sources["wonder_summaries"] = {
        f"{lvl}/{p.parent.name}": digest(p.read_bytes())
        for lvl in LEVELS
        for p in sorted((WONDER / lvl).glob("*/summary.json"))
    }
    return groups, sources


def main():  # noqa: PLR0912, PLR0915, one linear figure
    groups, sources = load()
    final = all(groups["wonder"][lvl]["n"] == SUITE_TASKS for lvl in LEVELS)
    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 14.6
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
        s = s.replace("$", r"\$")  # plain dollar signs, not mathtext
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

    text(
        left,
        1.72,
        "VulcanBench Frontier v4: Muse Wonder Full beside the frontier",
        30,
        True,
        heading=True,
    )
    wl, wm = groups["wonder"]["low"], groups["wonder"]["medium"]
    star = "" if final else "*"
    text(
        left,
        2.22,
        f"Muse Wonder Full solves {wl['functional']:.1f}% of hidden tests at Low and {wm['functional']:.1f}% at Medium{star}, "
        f"for an estimated ${wl['usd']:.2f} and ${wm['usd']:.2f} per task.",
        15,
    )
    text(
        left,
        2.60,
        "Accuracy is the functional score: the share of hidden behaviour tests passed, the same measure for every model. "
        "Code quality is not judged yet.",
        13.5,
        color=MUTED,
    )
    x = left
    for m in MODELS:
        swatch(x + 0.004, 3.05, m)
        text(x + 0.016, 3.05, NAMES[m], 12.5, True)
        x += 0.177

    xs = list(range(len(LEVELS)))
    width = 0.15
    offsets = {m: (i - (len(MODELS) - 1) / 2) * width for i, m in enumerate(MODELS)}

    def axis(x0, w, top, h):
        ax = fig.add_axes([x0, yf(top + h), w, h / H], facecolor=PAPER)
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.tick_params(axis="x", length=0, labelsize=13, pad=8)
        ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
        ax.grid(axis="y", color=RULE, linewidth=0.5)
        ax.set_axisbelow(True)
        labels = []
        for lvl in LEVELS:
            n = groups["wonder"][lvl]["n"]
            labels.append(lvl.capitalize() + ("" if n == SUITE_TASKS else "*"))
        ax.set_xticks(xs, labels)
        ax.set_xlim(-0.5, len(LEVELS) - 0.5)
        return ax

    text(left, 3.62, "Accuracy", 20, True, heading=True)
    text(
        left,
        3.94,
        "functional score, %  ·  higher is better  ·  focused scale  ·  whiskers +/-1 se",
        12,
        color=MUTED,
    )
    ax = axis(0.085, 0.385, 4.25, 3.3)
    lo = min(
        groups[m][lvl]["functional"] - groups[m][lvl]["functional_se"]
        for m in MODELS
        for lvl in LEVELS
    )
    ax.set_ylim(max(0, math.floor(lo / 5) * 5 - 5), 101.5)
    for m in MODELS:
        ys = [groups[m][lvl]["functional"] for lvl in LEVELS]
        es = [groups[m][lvl]["functional_se"] for lvl in LEVELS]
        bx = [i + offsets[m] for i in xs]
        bars = ax.bar(
            bx,
            ys,
            width=width * 0.92,
            color=COLORS[m],
            edgecolor=INK,
            linewidth=0.5,
            zorder=2,
        )
        for patch, lvl in zip(bars, LEVELS, strict=True):
            if groups[m][lvl]["n"] < SUITE_TASKS:
                patch.set_hatch("//")  # preliminary: fewer than 23 tasks so far
        ax.errorbar(bx, ys, yerr=es, fmt="none", ecolor=INK, elinewidth=0.8, capsize=2.5, zorder=3)
        for xx, lvl in zip(bx, LEVELS, strict=True):
            ax.annotate(
                f"n={groups[m][lvl]['n']}",
                (xx, ax.get_ylim()[0]),
                xytext=(0, 4),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=7.5,
                fontfamily="IBM Plex Mono",
                color="#ffffff" if m in ("wonder", "fable", "gpt61sol") else INK,
                rotation=90,
            )

    text(0.53, 3.62, "Cost per task", 20, True, heading=True)
    text(
        0.53,
        3.94,
        "USD, API-equivalent at list prices  ·  lower is cheaper  ·  log scale  ·  bases in notes",
        12,
        color=MUTED,
    )
    ax = axis(0.555, 0.385, 4.25, 3.3)
    ax.set_yscale("log")
    vals = [groups[m][lvl]["usd"] for m in MODELS for lvl in LEVELS]
    ax.set_ylim(min(vals) / 1.8, max(vals) * 2.2)
    ax.yaxis.set_major_formatter(matplotlib.ticker.FuncFormatter(lambda v, _: f"${v:g}"))
    for m in MODELS:
        ys = [groups[m][lvl]["usd"] for lvl in LEVELS]
        bx = [i + offsets[m] for i in xs]
        ax.bar(
            bx,
            ys,
            width=width * 0.92,
            color=COLORS[m],
            edgecolor=INK,
            linewidth=0.5,
            zorder=2,
            hatch="//" if m == "wonder" else None,
        )
        for xx, y in zip(bx, ys, strict=True):
            ax.annotate(
                f"{y:.2f}",
                (xx, y),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=8.5,
                fontfamily="IBM Plex Mono",
                color=INK,
            )

    ty = 8.25
    text(left, ty, "Table 1  |  Low and Medium, all five models", 14.5, heading=True)
    cols = {
        ("low", "f"): 0.43,
        ("low", "p"): 0.50,
        ("low", "usd"): 0.57,
        ("low", "min"): 0.64,
        ("medium", "f"): 0.73,
        ("medium", "p"): 0.80,
        ("medium", "usd"): 0.87,
        ("medium", "min"): 0.94,
    }
    line(left, right, ty + 0.28, INK, 1.2)
    text(0.535, ty + 0.50, "Low", 12.5, True, ha="center")
    text(0.835, ty + 0.50, "Medium" + star, 12.5, True, ha="center")
    line(0.39, 0.645, ty + 0.66, RULE, 0.6)
    line(0.69, right, ty + 0.66, RULE, 0.6)
    text(left, ty + 0.86, "Model  ·  harness", 12, True)
    for lvl in LEVELS:
        for k, lab in (("f", "Functional"), ("p", "Passed"), ("usd", "$/task"), ("min", "Min")):
            text(cols[lvl, k], ty + 0.86, lab, 11, True, ha="right", color=MUTED)
    line(left, right, ty + 1.04, INK, 0.6)
    y = ty + 1.32
    for m in MODELS:
        swatch(left + 0.004, y, m, 7)
        text(left + 0.016, y, NAMES[m], 12.5, True)
        text(left + 0.016 + 0.115, y, HARNESS[m], 11, color=MUTED)
        for lvl in LEVELS:
            g = groups[m][lvl]
            pref = "~" if m == "wonder" else ""
            text(cols[lvl, "f"], y, f"{g['functional']:.1f}", 12.5, numeric=True, ha="right")
            text(cols[lvl, "p"], y, f"{g['passed']}/{g['n']}", 12.5, numeric=True, ha="right")
            text(cols[lvl, "usd"], y, f"{pref}{g['usd']:.2f}", 12.5, numeric=True, ha="right")
            text(cols[lvl, "min"], y, f"{g['minutes']:.1f}", 12.5, numeric=True, ha="right")
        y += 0.40
        line(left, right, y - 0.2, RULE, 0.6)
    line(left, right, y - 0.2, INK, 1.2)

    notes = [
        (
            f"* Muse Wonder Full Medium covers {wm['n']} of {SUITE_TASKS} tasks so far; the last two run after its Muse Code weekly limit resets. "
            "Hatched bars mark this preliminary level and the estimated costs."
            if not final
            else "All levels cover all 23 tasks."
        ),
        "Accuracy is the functional score (share of hidden behaviour tests passed), mean over runs with +/-1 se whiskers; Passed counts runs that "
        "passed every test. Combined scores with Code quality are on each model's own report; Muse Wonder Full has not been judged yet.",
        "Cost bases differ. Muse Wonder Full ran on a Muse Code subscription and has no API price; its cost (~) is an estimate from its token use at "
        "the Muse Wonder 1.0 list price ($4.00 input, $0.20 cached, $20.00 output per million). Claude Opus 5.5 is Claude Code's own list-price total; "
        "GPT-6.1 Sol is API-equivalent from its Codex token ledger; Claude Fable 5.1 and GPT-6 Astra are their report's central API-equivalent estimates.",
        "Muse Wonder Full ran in Contributor mode with Muse Code's skill reminder off (13 to 15 October 2026); 7 of the finished Medium tasks overlapped another "
        "local job, which can affect minutes but not scores. Other models are their published Frontier v4 runs, not re-run. Harnesses differ.",
    ]
    yy = y + 0.12
    for n_ in notes:
        for w in textwrap.wrap(n_, 200):
            text(left, yy, w, 10.5, color=MUTED)
            yy += 0.24
        yy += 0.05

    OUTPUT.mkdir(parents=True, exist_ok=True)
    suffix = "" if final else "-preliminary"
    out = OUTPUT / f"musewonder-full-vs-frontier{suffix}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"musewonder-full-vs-frontier{suffix}-efforts.csv"
    with table.open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(
            [
                "model",
                "effort",
                "n",
                "functional_pct",
                "functional_se_pct",
                "passed",
                "usd_per_task",
                "minutes",
                "cost_basis",
            ]
        )
        basis = {
            "wonder": "estimate at Muse Wonder 1.0 list price",
            "opus55": "Claude Code list-price total",
            "gpt61sol": "API-equivalent, Codex ledger",
            "fable": "API-equivalent central estimate",
            "astra": "API-equivalent central estimate",
        }
        for m in MODELS:
            for lvl in LEVELS:
                g = groups[m][lvl]
                w.writerow(
                    [
                        m,
                        lvl,
                        g["n"],
                        f"{g['functional']:.2f}",
                        f"{g['functional_se']:.2f}",
                        g["passed"],
                        f"{g['usd']:.4f}",
                        f"{g['minutes']:.2f}",
                        basis[m],
                    ]
                )
    save(
        out.with_suffix(".json"),
        {
            "final": final,
            "groups": groups,
            "sources": sources,
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
        },
    )
    print(out)
    for m in MODELS:
        print(
            f"  {NAMES[m]:<18}"
            + "  ".join(
                f"{lvl}: {groups[m][lvl]['functional']:.1f}% {groups[m][lvl]['passed']}/{groups[m][lvl]['n']} ${groups[m][lvl]['usd']:.2f}"
                for lvl in LEVELS
            )
        )


if __name__ == "__main__":
    main()
