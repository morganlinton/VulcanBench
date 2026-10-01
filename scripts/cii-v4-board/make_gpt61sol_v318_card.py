"""GPT-6.1 Sol across effort levels under the code-quality-maintenance-v3.18 protocol.

Derived from the GPT-6 Sol v3.17 card: same layout, same combined score (50%
functional, 8.5% automated quality, 8.5% security, 33% Code quality, with Code
quality = 24 points reviewed panel plus 9 points intent recovery), same
coverage rules (FINAL vs PRELIMINARY is computed, never hand-set). All 115 runs
are published. One submission (medium paddockcore, submission-054) has no valid
Grok primary review (invalidate_unrecoverable_primary, v3.14 owner precedent);
the frozen summary publishes its Code quality from Muse alone (reviewed layer
and intent recovery), and this card does the same and checks every row's
figure against the summary's.

    python scripts/cii-v4-board/make_gpt61sol_v318_card.py
"""

from __future__ import annotations

import csv
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

RUN = ROOT / "runs-code-quality-maintenance-v3.18"
OUTPUT = ROOT / "docs/results/swe-v4-gpt61-sol-2026-09"
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
COLORS = {"gpt61sol": "#10A37F"}
LEVELS_FOR = {"gpt61sol": tuple(LEVELS)}
NAMES = {"gpt61sol": "GPT-6.1 Sol"}
HARNESS = {"gpt61sol": "Codex 0.159.0"}
# Cells short of 23 judged rows; empty unless a probe is ruled invalid.
EXPECTED = {}  # every cell judged on 23
INVALID_MARKER = "operator-invalid.json"
PANELS = ("muse", "grok")
PANEL_NAMES = {"muse": "Muse Spark 1.3 (Meta)", "grok": "Grok 4.6 (xAI)"}
SPLIT_WITHOUT_L3 = {"l1": 0.24, "l2": 0.09}


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def mean_se(values):
    values = list(values)
    if not values:
        return {"n": 0, "mean": None, "se": None}
    return {
        "n": len(values),
        "mean": statistics.mean(values),
        "se": statistics.stdev(values) / math.sqrt(len(values)) if len(values) > 1 else 0.0,
    }


def code_quality(row):
    """Per-row Code quality on 0 to 100 from both neutral panels, and which layers it used."""
    panels = [row["panels"][p] for p in PANELS]
    reviewed = [p for p in panels if p["l1"] is not None]
    if not reviewed:
        return None, None
    if len(reviewed) < len(panels) and not (row.get("published") or {}).get("code_quality"):
        return (
            None,
            None,
        )  # a missing review counts only once the frozen summary publishes the row without it
    l1 = statistics.mean(p["l1"]["score"] for p in reviewed)
    l2_values = [p["l2"] for p in reviewed if p["l2"] is not None]
    probes_complete = all(p["l2"] is not None or p["l2_denominator"] == 0 for p in reviewed)
    if probes_complete and l2_values:
        return (
            SPLIT_WITHOUT_L3["l1"] * l1 + SPLIT_WITHOUT_L3["l2"] * statistics.mean(l2_values)
        ) / WEIGHTS_V3["human_like"], "l1+l2"
    if probes_complete:
        return l1, "l1 (no scored quirks)"
    return l1, "l1 only"


def composite(run, quality, weight):
    other = (0.5 - weight) / 2
    return 100 * (
        0.5 * run["functional"]
        + other * run["quality"]
        + other * run["security"]
        + weight * quality / 100
    )


def load():
    summary = json.loads((RUN / "summary.json").read_text())
    manifest = {r["id"]: r for r in json.loads((RUN / "private-manifest.json").read_text())}
    protocol = json.loads((RUN / "protocol.json").read_text())
    require(summary["protocol"] == "code-quality-maintenance-v3.18", "wrong protocol")
    invalid = {p.parent.name for p in (RUN / "calls").glob(f"*/probe/*/{INVALID_MARKER}")}
    unpublished = {r["id"] for r in summary["rows"] if not r.get("published")}
    require(
        unpublished == invalid, f"unpublished {sorted(unpublished)} vs invalid {sorted(invalid)}"
    )
    require(
        summary["published_submissions"] + len(invalid) == summary["expected_submissions"],
        "publication count",
    )
    summary["rows"] = [r for r in summary["rows"] if r["id"] not in invalid]
    invalid_rows = [
        {"id": i, **{k: manifest[i][k] for k in ("model", "effort", "task")}}
        for i in sorted(invalid)
    ]
    require(
        set(summary["passing_panels"]) == set(PANELS), f"passing panels {summary['passing_panels']}"
    )
    require(
        protocol["weights"]["functional"] == WEIGHTS_V3["functional"]
        and protocol["weights"]["code_quality"]["total"] == WEIGHTS_V3["human_like"],
        "weights drift",
    )
    rows = []
    for entry in summary["rows"]:
        run = manifest[entry["id"]]
        cq, layers = code_quality(entry)
        if cq is None:
            continue
        published = (entry.get("published") or {}).get("code_quality")
        require(
            published is None or abs(published - cq) < 1e-6,
            f"{entry['id']}: Code quality {cq} differs from the frozen summary's {published}",
        )
        rows.append(
            {
                "id": entry["id"],
                "model": entry["model"],
                "effort": entry["effort"],
                "task": entry["task"],
                "fallback": bool(run.get("fallback")),
                "functional": run["functional"],
                "minutes": run["duration_s"] / 60,
                "code_quality": cq,
                "layers": layers,
                "readability": statistics.mean(
                    entry["panels"][p]["l1"]["readability"]
                    for p in PANELS
                    if entry["panels"][p]["l1"]
                ),
                "maintainability": statistics.mean(
                    entry["panels"][p]["l1"]["maintainability"]
                    for p in PANELS
                    if entry["panels"][p]["l1"]
                ),
                "by_panel": {
                    p: entry["panels"][p]["l1"]["score"] for p in PANELS if entry["panels"][p]["l1"]
                },
                # Intent recovery is shown only once it is scored, i.e. both panels' probes are complete for the row.
                "l2": statistics.mean(
                    [
                        entry["panels"][p]["l2"]
                        for p in PANELS
                        if entry["panels"][p]["l2"] is not None
                    ]
                )
                if layers == "l1+l2"
                else None,
                "combined_v3": composite(run, cq, WEIGHTS_V3["human_like"]),
                "combined_20pct": composite(run, cq, 0.20),
            }
        )
    total = len(summary["rows"])
    final = len(rows) == total and all(r["layers"] != "l1 only" for r in rows)
    coverage = {
        "submissions_with_both_reviews": len(rows),
        "submissions_total": total,
        "submissions_with_probes": sum(r["layers"] != "l1 only" for r in rows),
        "per_panel_reviews": {
            p: sum(1 for e in summary["rows"] if e["panels"][p]["l1"]) for p in PANELS
        },
        "per_panel_probes": {
            p: sum(1 for e in summary["rows"] if e["panels"][p]["l2"] is not None) for p in PANELS
        },
    }
    coverage["unpublished_invalid_probe"] = invalid_rows
    return summary, protocol, rows, final, coverage


def aggregate(rows):
    groups = {}
    for model in COLORS:
        for effort in LEVELS_FOR[model]:
            rs = [r for r in rows if r["model"] == model and r["effort"] == effort]
            groups[model, effort] = {
                "model": model,
                "effort": effort,
                "n": len(rs),
                "combined": mean_se(r["combined_v3"] for r in rs),
                "combined_20pct": mean_se(r["combined_20pct"] for r in rs),
                "code_quality": mean_se(r["code_quality"] for r in rs),
                "readability": mean_se(r["readability"] for r in rs),
                "maintainability": mean_se(r["maintainability"] for r in rs),
                "l2": mean_se(r["l2"] for r in rs if r["l2"] is not None),
                "by_panel": {
                    p: mean_se(r["by_panel"][p] for r in rs if p in r["by_panel"]) for p in PANELS
                },
                "minutes": mean_se(r["minutes"] for r in rs),
                "passed": sum(r["functional"] == 1 for r in rs),
                "fallbacks": sum(r["fallback"] for r in rs),
            }
    return groups


def fmt(stat, digits=2):
    return "n/a" if stat["mean"] is None else f"{stat['mean']:.{digits}f}"


def main():  # noqa: PLR0912, PLR0915, one linear figure
    _summary, _protocol, rows, final, coverage = load()
    groups = aggregate(rows)
    for (model, effort), g in groups.items():
        require(
            g["n"] == EXPECTED.get(f"{model}/{effort}", 23),
            f"{model} {effort} has {g['n']} rows",
        )
    best = max(groups.values(), key=lambda g: g["combined"]["mean"])
    low, high = groups["gpt61sol", "low"], groups["gpt61sol", "max"]
    headline = f"Combined score {low['combined']['mean']:.2f} to {high['combined']['mean']:.2f} from low to max."
    sub = f"Best at {best['effort']}"

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    width_in, height_in = 16, 12.45
    fig = plt.figure(figsize=(width_in, height_in), dpi=150, facecolor=PAPER)

    def yf(inches):
        """Figure fraction for a position measured in inches from the top edge."""
        return 1 - inches / height_in

    def text(
        x, y_in, label, size=16, bold=False, heading=False, numeric=False, ha="left", color=INK
    ):
        family = (
            "IBM Plex Mono"
            if numeric
            else ("Chakra Petch SemiBold" if bold else "Chakra Petch Medium")
            if heading
            else "Geist"
        )
        weight = (
            (500 if bold else 400)
            if numeric
            else ((600 if bold else 500) if heading else (700 if bold else 400))
        )
        return fig.text(
            x,
            yf(y_in),
            label,
            fontsize=size,
            fontfamily=family,
            weight=weight,
            ha=ha,
            va="center",
            color=color,
        )

    def line(x1, x2, y_in, color=RULE, width=0.8):
        fig.add_artist(
            plt.Line2D(
                [x1, x2], [yf(y_in), yf(y_in)], transform=fig.transFigure, color=color, lw=width
            )
        )

    def box(x, top_in, w, h_in, face, edge, lw=0.8, radius=0.006):
        fig.patches.append(
            FancyBboxPatch(
                (x, yf(top_in + h_in)),
                w,
                h_in / height_in,
                boxstyle=f"round,pad=0,rounding_size={radius}",
                transform=fig.transFigure,
                facecolor=face,
                edgecolor=edge,
                linewidth=lw,
            )
        )

    left, right = 0.06, 0.94
    # Masthead
    logo_h = 0.42 / height_in
    logo = fig.add_axes([left, yf(0.86), 0.42 / width_in, logo_h])
    mark = logo.imshow(plt.imread(ROOT / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    text(left + 0.038, 0.65, "VulcanBench", 20, True, heading=True)
    text(right, 0.65, "September 2026", 14, ha="right", color=MUTED)
    line(left, right, 1.05, INK, 1.2)

    # Title
    text(
        left,
        1.78,
        "VulcanBench Frontier v4: GPT-6.1 Sol across effort levels",
        33,
        True,
        heading=True,
    )
    text(
        left,
        2.22,
        "Combined score and runtime at every effort level, 23 tasks per effort. "
        "Code quality judged by Muse Spark 1.3 and Grok 4.6.",
        15,
        color=MUTED,
    )

    # Legend
    markers = {"gpt61sol": "o"}
    for model, x in (("gpt61sol", left),):
        fig.add_artist(
            plt.Line2D(
                [x + 0.004],
                [yf(2.68)],
                transform=fig.transFigure,
                marker=markers[model],
                color=COLORS[model],
                markersize=8,
                markeredgecolor=INK,
                markeredgewidth=0.6,
                linestyle="none",
            )
        )
        text(x + 0.018, 2.68, f"{NAMES[model]}  ·  {HARNESS[model]}", 15, True)
    text(
        right,
        2.68,
        "n=23 at every effort" if not EXPECTED else "n=23 at every effort, n=22 judged at medium",
        13,
        ha="right",
        color=MUTED,
    )

    # Charts: combined score by effort (lines) and runtime by effort (bars)
    chart_top, chart_h = 3.75, 3.0
    for panel, (x0, w) in (("combined", (0.085, 0.405)), ("minutes", (0.565, 0.39))):
        tx = left if panel == "combined" else x0 - 0.04
        text(
            tx,
            3.1,
            "Combined score" if panel == "combined" else "Mean runtime",
            21,
            True,
            heading=True,
        )
        text(
            tx,
            3.42,
            "/100  ·  higher is better  ·  focused scale"
            if panel == "combined"
            else "Minutes per task  ·  lower is better",
            12,
            color=MUTED,
        )
        ax = fig.add_axes([x0, yf(chart_top + chart_h), w, chart_h / height_in], facecolor=PAPER)
        ax.set_xticks(range(len(LEVELS)), [e.replace("-", " ").capitalize() for e in LEVELS])
        ax.tick_params(axis="x", length=0, labelsize=12, pad=8)
        ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.grid(axis="y", color=RULE, linewidth=0.5)
        ax.set_axisbelow(True)
        if panel == "combined":
            values = [groups[m, e]["combined"] for m in COLORS for e in LEVELS_FOR[m]]
            lo = math.floor(min(v["mean"] - (v["se"] or 0) for v in values)) - 3
            hi = math.ceil(max(v["mean"] + (v["se"] or 0) for v in values)) + 0.5
            ax.set_ylim(lo, hi)
            step = 1 if hi - lo <= 12 else (5 if hi - lo <= 40 else 10)
            ax.set_yticks(range(math.ceil(lo / step) * step, math.floor(hi) + 1, step))
            ax.set_xlim(-0.45, len(LEVELS) - 0.55)
            for model in COLORS:  # noqa: PLC0206, keys only
                xs = [LEVELS.index(e) for e in LEVELS_FOR[model]]
                ys = [groups[model, e]["combined"]["mean"] for e in LEVELS_FOR[model]]
                es = [groups[model, e]["combined"]["se"] or 0 for e in LEVELS_FOR[model]]
                ax.plot(xs, ys, color=COLORS[model], linewidth=2, zorder=2)
                ax.errorbar(
                    xs,
                    ys,
                    yerr=es,
                    fmt=markers[model],
                    color=COLORS[model],
                    markersize=8,
                    markeredgecolor=INK,
                    markeredgewidth=0.6,
                    ecolor=INK,
                    elinewidth=0.9,
                    capsize=3,
                    zorder=3,
                )
                for i, (y, e) in zip(xs, zip(ys, es, strict=True), strict=True):
                    above = True
                    ax.annotate(
                        f"{y:.2f}",
                        (i, y + e if above else y - e),
                        xytext=(0, 7 if above else -7),
                        textcoords="offset points",
                        ha="center",
                        va="bottom" if above else "top",
                        fontsize=10.5,
                        fontfamily="IBM Plex Mono",
                        color=INK,
                    )
        else:
            top = (
                10
                * math.ceil(
                    max(
                        groups[m, e]["minutes"]["mean"] + (groups[m, e]["minutes"]["se"] or 0)
                        for m in COLORS
                        for e in LEVELS_FOR[m]
                    )
                    / 10
                )
                + 5
            )
            ax.set_ylim(0, top)
            ax.set_yticks(range(0, top + 1, 10))
            ax.set_xlim(-0.6, len(LEVELS) - 0.4)
            for model, shift in (("gpt61sol", 0.0),):
                xs = [LEVELS.index(e) for e in LEVELS_FOR[model]]
                ys = [groups[model, e]["minutes"]["mean"] for e in LEVELS_FOR[model]]
                es = [groups[model, e]["minutes"]["se"] or 0 for e in LEVELS_FOR[model]]
                ax.bar(
                    [i + shift for i in xs],
                    ys,
                    width=0.5,
                    color=COLORS[model],
                    edgecolor=INK,
                    linewidth=0.5,
                    yerr=es,
                    error_kw={"ecolor": INK, "elinewidth": 0.9, "capsize": 3},
                    zorder=2,
                )
                for i, (y, e) in zip(xs, zip(ys, es, strict=True), strict=True):
                    ax.annotate(
                        f"{y:.1f}",
                        (i + shift, y + e),
                        xytext=(0, 5),
                        textcoords="offset points",
                        ha="center",
                        va="bottom",
                        fontsize=10.5,
                        fontfamily="IBM Plex Mono",
                        color=INK,
                    )

    # Table: Code quality components at every effort level
    text(left, 7.55, "Table 1  |  Code quality at each effort level", 14.5, False, heading=True)
    cols = {e: x for e, x in zip(LEVELS, (0.50, 0.61, 0.72, 0.83, 0.94), strict=True)}
    line(left, right, 7.83, INK, 1.2)
    text(left, 8.07, "Component", 12.5, True)
    for effort in LEVELS:
        text(cols[effort], 8.07, effort.replace("-", " ").capitalize(), 12.5, True, ha="right")
    text(cols["max"], 8.33, "/100, mean of both judges", 10, ha="right", color=MUTED)
    line(left, right, 8.5, INK, 0.6)

    def se(stat):
        return {"mean": stat["se"], "se": None} if stat.get("se") is not None else {"mean": None}

    table_rows = [
        ("Code quality", "code_quality", 2, True, False),
        ("    Human readability", "readability", 1, False, False),
        ("    Maintainability", "maintainability", 1, False, False),
        ("    Intent recovery", "l2", 1, False, False),
        ("    Rated by Muse Spark 1.3", ("by_panel", "muse"), 1, False, False),
        ("    Rated by Grok 4.6", ("by_panel", "grok"), 1, False, False),
        ("Standard error of Code quality", "se", 2, False, True),
    ]
    step = 0.35
    y = 8.74
    for label, key, digits, emphasis, group_end in table_rows:
        quiet = label.startswith("Standard error")
        text(left, y, label, 13 if emphasis else 12.5, emphasis, color=MUTED if quiet else INK)
        for effort in LEVELS:
            g = groups["gpt61sol", effort]
            if key == "se":
                stat = se(g["code_quality"])
            elif isinstance(key, tuple):
                stat = g[key[0]][key[1]]
            else:
                stat = g[key]
            if stat["mean"] is None:
                text(cols[effort], y, "pending", 12, numeric=True, ha="right", color=MUTED)
            else:
                text(
                    cols[effort],
                    y,
                    f"{stat['mean']:.{digits}f}",
                    14.5 if emphasis else 13.5,
                    emphasis,
                    numeric=True,
                    ha="right",
                    color=MUTED if quiet else INK,
                )
        y += step
        if group_end:
            line(left, right, y - step / 2, RULE, 0.6)
    line(left, right, y - step / 2, INK, 1.2)
    notes = [
        "All 115 runs finished inside the flat 3-hour task bound (longest 80 minutes); every cell is judged on 23 runs.",
        "Medium paddockcore has no valid Grok 4.6 review (both attempts quoted a changed line), so, as the protocol does for any",
        "such run, its Code quality is Muse Spark 1.3's alone (reviewed layer and intent recovery). High codeccore's Muse probe",
        "was recovered after a JSON escaping slip. Ran on Codex CLI 0.159.0, the first release that serves GPT-6.1 Sol on a ChatGPT plan.",
    ]
    for i, note in enumerate(notes):
        text(left, y - step / 2 + 0.18 + 0.24 * i, note, 11, color=MUTED)

    suffix = "" if final else "-preliminary"
    out = OUTPUT / f"gpt61-sol-v318{suffix}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"gpt61-sol-v318{suffix}-efforts.csv"
    with table.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(
            [
                "model",
                "effort",
                "n",
                "combined_v3",
                "combined_v3_se",
                "combined_20pct",
                "code_quality",
                "code_quality_se",
                "readability",
                "maintainability",
                "intent_recovery",
                "muse_l1",
                "grok_l1",
                "minutes",
                "passed",
                "fallbacks",
            ]
        )
        for (model, effort), g in groups.items():
            writer.writerow(
                [
                    model,
                    effort,
                    g["n"],
                    fmt(g["combined"], 4),
                    fmt({"mean": g["combined"]["se"]}, 4),
                    fmt(g["combined_20pct"], 4),
                    fmt(g["code_quality"], 4),
                    fmt({"mean": g["code_quality"]["se"]}, 4),
                    fmt(g["readability"], 4),
                    fmt(g["maintainability"], 4),
                    fmt(g["l2"], 4),
                    fmt(g["by_panel"]["muse"], 4),
                    fmt(g["by_panel"]["grok"], 4),
                    fmt(g["minutes"], 4),
                    g["passed"],
                    g["fallbacks"],
                ]
            )
    save(
        out.with_suffix(".json"),
        {
            "final": final,
            "coverage": coverage,
            "headline": headline,
            "subtitle": sub,
            "best_effort": best["effort"],
            "expected_rows": {
                f"{m}/{e}": EXPECTED.get(f"{m}/{e}", 23) for m in COLORS for e in LEVELS_FOR[m]
            },
            "weights": {
                "functional": 0.5,
                "quality": 0.085,
                "security": 0.085,
                "code_quality": 0.33,
                "code_quality_split": SPLIT_WITHOUT_L3,
            },
            "summary_sha256": digest((RUN / "summary.json").read_bytes()),
            "protocol_sha256": digest((RUN / "protocol.json").read_bytes()),
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table.name,
            "visual_qa": "Requires visual inspection after rendering",
        },
    )
    print(out)


if __name__ == "__main__":
    main()
