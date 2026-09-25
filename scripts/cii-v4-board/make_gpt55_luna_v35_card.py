"""GPT-5.5 versus GPT-5.6 Luna card under the code-quality-maintenance-v3.5 protocol.

Reads the frozen v3.5 summary (neutral panel: Muse Spark 1.3 and Grok 4.6)
and the private manifest (functional, automated quality, security, runtime per
run). Combined score uses the locked profile: 50% functional, 8.5% automated
quality, 8.5% security, 33% Code quality. Until the measured-maintenance layer
exists, Code quality is 24 points reviewed panel plus 9 points intent recovery,
as the protocol pre-registers.

Coverage rules, applied by the script, not by hand:
- FINAL card: every submission has both panels' reviews and probes. Filename
  has no suffix.
- PRELIMINARY card: anything less. Rows use whatever layers both panels have
  completed (reviewed layer only where probes are missing), the headline strip
  says PRELIMINARY with coverage counts, and the filename carries the suffix.
No takeaway is hard-coded; the headline is computed from the data each run.
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

RUN = ROOT / "runs-code-quality-maintenance-v3.5"
OUTPUT = ROOT / "docs/results/swe-v4-gpt55-luna-2026-09"
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
COLORS = {"gpt55": "#6B6B66", "luna": "#10A37F"}
LEVELS_FOR = {"gpt55": ("low", "medium", "high", "extra-high"), "luna": tuple(LEVELS)}
NAMES = {"gpt55": "GPT-5.5", "luna": "GPT-5.6 Luna"}
HARNESS = {"gpt55": "Codex", "luna": "Codex"}
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
    if any(p["l1"] is None for p in panels):
        return None, None
    l1 = statistics.mean(p["l1"]["score"] for p in panels)
    l2_values = [p["l2"] for p in panels if p["l2"] is not None]
    probes_complete = all(p["l2"] is not None or p["l2_denominator"] == 0 for p in panels)
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
    require(summary["protocol"] == "code-quality-maintenance-v3.5", "wrong protocol")
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
                    entry["panels"][p]["l1"]["readability"] for p in PANELS
                ),
                "maintainability": statistics.mean(
                    entry["panels"][p]["l1"]["maintainability"] for p in PANELS
                ),
                "by_panel": {p: entry["panels"][p]["l1"]["score"] for p in PANELS},
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
                "by_panel": {p: mean_se(r["by_panel"][p] for r in rs) for p in PANELS},
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
    complete_efforts = [
        e for e in LEVELS if all((m, e) in groups and groups[m, e]["n"] > 0 for m in COLORS)
    ]
    require(complete_efforts, "no effort has rows for both models yet")
    best = {
        m: max(
            (groups[m, e] for e in LEVELS_FOR[m] if groups[m, e]["n"] > 0),
            key=lambda g: g["combined"]["mean"],
        )
        for m in COLORS
    }
    wins = {"combined": 0, "code_quality": 0, "runtime": 0}
    for e in complete_efforts:
        a, f = groups["gpt55", e], groups["luna", e]
        wins["combined"] += a["combined"]["mean"] > f["combined"]["mean"]
        wins["code_quality"] += a["code_quality"]["mean"] > f["code_quality"]["mean"]
        wins["runtime"] += a["minutes"]["mean"] < f["minutes"]["mean"]
    leader = max(COLORS, key=lambda m: best[m]["combined"]["mean"])
    other = "luna" if leader == "gpt55" else "gpt55"
    gap = best[leader]["combined"]["mean"] - best[other]["combined"]["mean"]
    cq_leader = max(COLORS, key=lambda m: best[m]["code_quality"]["mean"])
    faster = min(COLORS, key=lambda m: best[m]["minutes"]["mean"])
    ratio = max(best[m]["minutes"]["mean"] for m in COLORS) / max(
        1e-9, min(best[m]["minutes"]["mean"] for m in COLORS)
    )
    short = {"gpt55": "GPT-5.5", "luna": "Luna"}
    if abs(gap) < 1:
        headline = f"Similar scores. {short[cq_leader]} writes more maintainable code."
    else:
        headline = f"{short[leader]} leads by {gap:.1f} points at 33% Code quality."
    sub = f"{short[faster]} {ratio:.1f}× faster"  # noqa: RUF001

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    width_in, height_in = 16, 11.5
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
    text(left, 1.78, "VulcanBench Frontier v4: GPT-5.5 vs. GPT-5.6 Luna", 33, True, heading=True)
    text(
        left,
        2.22,
        "Combined score and runtime at every effort level. 23 tasks per model per effort; GPT-5.5 has no max level. "
        "Code quality judged by Muse Spark 1.3 and Grok 4.6.",
        15,
        color=MUTED,
    )

    # Legend
    markers = {"gpt55": "s", "luna": "o"}
    for model, x in (("gpt55", left), ("luna", 0.5)):
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
    text(right, 2.68, "n=23 at every effort", 13, ha="right", color=MUTED)

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
                    rival = "gpt55" if model == "luna" else "luna"
                    rival_y = (
                        groups[rival, LEVELS[i]]["combined"]["mean"]
                        if (rival, LEVELS[i]) in groups
                        else None
                    )
                    # The higher point at each effort labels above, the lower below, so labels never collide.
                    above = rival_y is None or y >= rival_y
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
            for model, shift in (("gpt55", -0.19), ("luna", 0.19)):
                xs = [LEVELS.index(e) for e in LEVELS_FOR[model]]
                ys = [groups[model, e]["minutes"]["mean"] for e in LEVELS_FOR[model]]
                es = [groups[model, e]["minutes"]["se"] or 0 for e in LEVELS_FOR[model]]
                ax.bar(
                    [i + shift for i in xs],
                    ys,
                    width=0.34,
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

    # Table
    a, f = best["gpt55"], best["luna"]
    text(
        left,
        7.55,
        "Table 1  |  Code quality at each model's best effort",
        14.5,
        False,
        heading=True,
    )
    col_a, col_f, col_d = 0.60, 0.78, 0.94
    line(left, right, 7.83, INK, 1.2)
    text(left, 8.07, "Component", 12.5, True)
    text(col_a, 8.07, f"GPT-5.5, {a['effort'].replace('-', ' ')}", 12.5, True, ha="right")
    text(col_f, 8.07, f"Luna, {f['effort'].replace('-', ' ')}", 12.5, True, ha="right")
    text(col_d, 8.07, "Difference", 12.5, True, ha="right")
    text(col_d, 8.33, "Luna minus GPT-5.5", 10, ha="right", color=MUTED)
    line(left, right, 8.5, INK, 0.6)

    def se(stat):
        return (
            {"mean": stat["se"], "se": None}
            if stat.get("se") is not None
            else {"mean": None, "se": None}
        )

    rows = [
        ("Code quality", a["code_quality"], f["code_quality"], 2, True, False),
        ("    Human readability", a["readability"], f["readability"], 1, False, False),
        ("    Maintainability", a["maintainability"], f["maintainability"], 1, False, False),
        *(
            [("    Intent recovery", a["l2"], f["l2"], 1, False, False)]
            if a["l2"]["mean"] is not None and f["l2"]["mean"] is not None
            else []
        ),
        (
            "    Rated by Muse Spark 1.3",
            a["by_panel"]["muse"],
            f["by_panel"]["muse"],
            1,
            False,
            False,
        ),
        ("    Rated by Grok 4.6", a["by_panel"]["grok"], f["by_panel"]["grok"], 1, False, False),
        (
            "Standard error of Code quality",
            se(a["code_quality"]),
            se(f["code_quality"]),
            2,
            False,
            True,
        ),
    ]
    step = 0.38
    y = 8.82
    for label, sa, sf, digits, emphasis, group_end in rows:
        quiet = label.startswith("Standard error")
        text(left, y, label, 13 if emphasis else 12.5, emphasis, color=MUTED if quiet else INK)
        for x, stat in ((col_a, sa), (col_f, sf)):
            if stat["mean"] is None:
                text(x, y, "pending", 12, numeric=True, ha="right", color=MUTED)
            else:
                text(
                    x,
                    y,
                    f"{stat['mean']:.{digits}f}",
                    14.5 if emphasis else 13.5,
                    emphasis,
                    numeric=True,
                    ha="right",
                    color=MUTED if quiet else INK,
                )
        if (
            sa["mean"] is not None
            and sf["mean"] is not None
            and not label.startswith("Standard error")
        ):
            delta = sf["mean"] - sa["mean"]
            shown = (
                (f"{delta:+.{max(digits, 1)}f}" if digits else f"{int(delta):+d}") if delta else "0"
            )
            text(col_d, y, shown, 14.5 if emphasis else 13.5, emphasis, numeric=True, ha="right")
        y += step
        if group_end:
            line(left, right, y - step / 2, RULE, 0.6)
    line(left, right, y - step / 2, INK, 1.2)

    suffix = "" if final else "-preliminary"
    out = OUTPUT / f"gpt55-vs-luna-v35{suffix}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"gpt55-vs-luna-v35{suffix}-efforts.csv"
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
            "selected": {
                m: {k: v for k, v in best[m].items() if k != "by_panel"}
                | {"by_panel": best[m]["by_panel"]}
                for m in COLORS
            },
            "matched_effort_leads_gpt55": wins,
            "complete_efforts": complete_efforts,
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
