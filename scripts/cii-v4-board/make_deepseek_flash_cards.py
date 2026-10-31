"""X cards (1600x900) for DeepSeek V4.1 Flash on VulcanBench Frontier v4 beside Claude Opus 5.5 and GPT-6.1 Sol.

Levels are the three DeepSeek's API offers: Low, High and Max (it has no medium).
Flash's combined score is the v3.27 summary (runs-code-quality-maintenance-v3.27,
judged by Muse Spark 1.3 and GPT-6.1 Sol); Opus 5.5's and GPT-6.1 Sol's come from
their published rows in docs/results/swe-v4-gpt6-vs-gpt56-2026-09/gpt61sol-opus55-astra-efforts.csv,
judged by Muse Spark 1.3 and Grok 4.6. The panels differ, and the cards say so.

Flash's minutes, passes and cost come from its run summaries in
runs-effort-deepseek-flash-dsh: cost at DeepSeek's off-peak list price (the price
table's rate), with the amount actually billed after peak-hour doubling alongside.
The card refuses to draw unless every Flash level has 23 judged runs that match
the run summaries.

    python scripts/cii-v4-board/make_deepseek_flash_cards.py --judging <v3.27 run directory> --out <dir>
"""

from __future__ import annotations

import argparse
import csv
import json
import statistics
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch

ROOT = Path(__file__).resolve().parents[2]
RUNS = ROOT / "runs-effort-deepseek-flash-dsh"
COMPARATORS = ROOT / "docs/results/swe-v4-gpt6-vs-gpt56-2026-09/gpt61sol-opus55-astra-efforts.csv"
LEVELS = ("low", "high", "max")
LABEL = {"low": "Low", "high": "High", "max": "Max"}
ORDER = ("dsflash", "opus55", "gpt61sol")
NAMES = {"dsflash": "DeepSeek V4.1 Flash", "opus55": "Claude Opus 5.5", "gpt61sol": "GPT-6.1 Sol"}
HARNESS = {"dsflash": "DeepSeek Harness", "opus55": "Claude Code", "gpt61sol": "Codex"}
COLORS = {"dsflash": "#5786FE", "opus55": "#D97757", "gpt61sol": "#0B3D2E"}
MARKERS = {"dsflash": "o", "opus55": "s", "gpt61sol": "^"}
PAPER, INK, RULE, MUTED, TILE = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66", "#fbfaf7"
W, H = 16, 9


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def flash_rows(judging: Path) -> dict:
    summary = json.loads((judging / "summary.json").read_text())
    require(summary["ready_for_publication"], "v3.27 summary is not ready for publication")
    require(summary["failed_panels"] == [], "a v3.27 panel failed its exam")
    out = {}
    for level in LEVELS:
        runs = [json.loads(p.read_text()) for p in sorted((RUNS / level).glob("*/summary.json"))]
        group = summary["groups"][f"dsflash/{level}"]
        require(
            len(runs) == 23 and group["composite_v3"]["n"] == 23, f"{level} is not 23 judged runs"
        )
        out[level] = {
            "n": 23,
            "combined": group["composite_v3"]["mean"],
            "combined_se": group["composite_v3"]["se"],
            "code_quality": group["code_quality"]["mean"],
            "by_panel": {p: v["mean"] for p, v in group["by_panel"].items()},
            "passed": sum(r["scores"]["functional"] >= 1 for r in runs),
            "functional": 100 * statistics.mean(r["scores"]["functional"] for r in runs),
            "minutes": statistics.mean(r["duration_s"] for r in runs) / 60,
            "usd": statistics.mean(r["economics"]["marginal_cash_usd"] for r in runs),
            "billed": statistics.mean(r["cli_agent"]["cli_reported_cost_usd"] for r in runs),
        }
    return out


def comparator_rows() -> dict:
    out: dict = {m: {} for m in ORDER if m != "dsflash"}
    with COMPARATORS.open() as source:
        for row in csv.DictReader(source):
            if row["model"] in out and row["effort"] in LEVELS:
                out[row["model"]][row["effort"]] = {
                    "n": int(row["runs"]),
                    "judged": int(row["judged"]),
                    "combined": float(row["combined"]),
                    "combined_se": float(row["combined_se"]),
                    "passed": int(row["passed"]),
                    "minutes": float(row["mean_minutes"]),
                    "usd": float(row["mean_usd"]),
                }
    for model, levels in out.items():
        require(set(levels) == set(LEVELS), f"{model} lacks a level")
    return out


def card(header_note: str):
    fig = plt.figure(figsize=(W, H), dpi=100, facecolor=PAPER)

    def text(x, y, s, size=13, weight=400, face="Geist", ha="left", color=INK, va="center"):
        s = s.replace("$", r"\$")
        return fig.text(
            x / W,
            1 - y / H,
            s,
            fontsize=size,
            fontfamily=face,
            weight=weight,
            ha=ha,
            va=va,
            color=color,
        )

    logo = fig.add_axes([0.88 / W, 1 - 0.94 / H, 0.54 / W, 0.54 / H])
    mark = logo.imshow(plt.imread(ROOT / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    text(1.6, 0.67, "VulcanBench", 23, 600, "Chakra Petch SemiBold")
    text(W - 0.88, 0.67, header_note, 17, color=MUTED, ha="right")
    fig.add_artist(
        plt.Line2D(
            [0.88 / W, (W - 0.88) / W],
            [1 - 1.24 / H] * 2,
            transform=fig.transFigure,
            color=INK,
            lw=1.6,
        )
    )
    return fig, text


def footnotes(text, lines, top):
    for i, line in enumerate(lines):
        text(0.88, top + i * 0.25, line, 11.2, color=MUTED)


def legend(fig, text, y):
    for i, m in enumerate(ORDER):
        x = 0.95 + i * 3.6
        fig.add_artist(
            plt.Line2D(
                [x / W],
                [1 - y / H],
                transform=fig.transFigure,
                marker=MARKERS[m],
                color=COLORS[m],
                markersize=13,
                markeredgecolor=INK,
                markeredgewidth=0.7,
                linestyle="none",
            )
        )
        text(x + 0.25, y, NAMES[m], 16, 700 if m == "dsflash" else 600)


def main() -> None:  # noqa: PLR0915, one card set
    parser = argparse.ArgumentParser()
    parser.add_argument("--judging", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK})
    data = {"dsflash": flash_rows(args.judging), **comparator_rows()}
    args.out.mkdir(parents=True, exist_ok=True)
    fl = data["dsflash"]

    panels_note = (
        "Code quality judges: DeepSeek V4.1 Flash by Muse Spark 1.3 and GPT-6.1 Sol (Grok 4.6's Cursor account was "
        "out of usage); the others by Muse Spark 1.3 and Grok 4.6."
    )
    common = [
        panels_note,
        "Panels are not calibrated against each other. One run per task per level, 23 tasks. Flash ran in DeepSeek Harness 0.2.1-rc.2;",
        "the others are their published Frontier v4 runs in Claude Code and Codex. DeepSeek's API offers Low, High and Max only.",
    ]

    def bars(metric, title, subtitle, fmt, ylim, notes, name):
        fig, text = card("vulcanbench.com")
        text(0.88, 1.85, title, 36, 500, "Chakra Petch Medium")
        text(0.88, 2.42, subtitle, 15.5, color=MUTED)
        legend(fig, text, 3.0)
        ax = fig.add_axes([1.5 / W, 1 - 7.0 / H, (W - 0.88 - 1.5) / W, 3.55 / H])
        ax.set_facecolor(PAPER)
        width = 0.26
        for j, m in enumerate(ORDER):
            xs = [i + (j - 1) * (width + 0.02) for i in range(len(LEVELS))]
            vals = [data[m][lv][metric] for lv in LEVELS]
            ax.bar(xs, vals, width, color=COLORS[m], edgecolor=INK, linewidth=0.6, zorder=3)
            for x, v in zip(xs, vals, strict=True):
                ax.text(
                    x,
                    v + (ylim[1] - ylim[0]) * 0.015,
                    fmt(m, v),
                    ha="center",
                    va="bottom",
                    fontsize=13,
                    fontfamily="IBM Plex Mono",
                    weight=500 if m == "dsflash" else 400,
                )
        ax.set_ylim(*ylim)
        ax.set_xticks(range(len(LEVELS)), [LABEL[lv] for lv in LEVELS], fontsize=17)
        ax.tick_params(axis="x", length=0, pad=8)
        ax.tick_params(axis="y", labelsize=13, length=0, colors=INK)
        for side in ("top", "right"):
            ax.spines[side].set_visible(False)
        ax.spines["left"].set_color(INK)
        ax.spines["bottom"].set_color(INK)
        ax.yaxis.grid(True, color=RULE, lw=0.7, zorder=0)
        footnotes(text, notes, 7.85)
        fig.savefig(args.out / name, dpi=100, facecolor=PAPER)
        plt.close(fig)

    # 1: summary
    fig, text = card("vulcanbench.com")
    text(
        0.88, 1.85, "DeepSeek V4.1 Flash on VulcanBench Frontier v4", 36, 500, "Chakra Petch Medium"
    )
    text(
        0.88,
        2.42,
        "In DeepSeek Harness, against Claude Opus 5.5 and GPT-6.1 Sol at Low, High and Max",
        15.5,
        color=MUTED,
    )
    comb = [fl[lv]["combined"] for lv in LEVELS]
    mins = [fl[lv]["minutes"] for lv in LEVELS]
    usd = [fl[lv]["usd"] for lv in LEVELS]
    sol_usd = [data["gpt61sol"][lv]["usd"] for lv in LEVELS]
    ranks = []
    for lv in LEVELS:
        order = sorted(ORDER, key=lambda m: -data[m][lv]["combined"])
        ranks.append(order.index("dsflash") + 1)
    tiles = [
        (
            f"{min(comb):.1f} to {max(comb):.1f}",
            "combined score; leads at Low,\nbehind Opus 5.5 at High and Max"
            if ranks == [1, 2, 2]
            else "combined score across the three levels",
        ),
        (
            f"{min(fl[lv]['passed'] for lv in LEVELS)} to {max(fl[lv]['passed'] for lv in LEVELS)} of 23",
            "tasks fully solved at each level;\nmore effort adds no solves",
        ),
        (
            f"{min(mins):.0f} to {max(mins):.0f} min",
            f"per task; Sol {min(data['gpt61sol'][lv]['minutes'] for lv in LEVELS):.0f} to "
            f"{max(data['gpt61sol'][lv]['minutes'] for lv in LEVELS):.0f}, Opus 5.5\n"
            f"{min(data['opus55'][lv]['minutes'] for lv in LEVELS):.0f} to {max(data['opus55'][lv]['minutes'] for lv in LEVELS):.0f}",
        ),
        (
            f"${min(usd):.2f} to {max(usd):.2f}",
            f"per task at list price, about\n1/{statistics.mean(s / u for s, u in zip(sol_usd, usd, strict=True)):.0f} "
            "of GPT-6.1 Sol's",
        ),
    ]
    tw, gap, top = 3.31, 0.32, 3.0
    for i, (big, small) in enumerate(tiles):
        x = 0.88 + i * (tw + gap)
        ax = fig.add_axes([x / W, 1 - (top + 2.45) / H, tw / W, 2.45 / H])
        ax.add_patch(
            FancyBboxPatch(
                (0.012, 0.015),
                0.976,
                0.97,
                boxstyle="round,pad=0,rounding_size=0.05",
                fc=TILE,
                ec=RULE,
                lw=1.2,
                transform=ax.transAxes,
            )
        )
        ax.axis("off")
        text(x + 0.24, top + 0.62, big, 31, 600, "Chakra Petch SemiBold")
        text(x + 0.24, top + 1.45, small, 13.2, color="#3d3d39", va="top")
    y0 = 5.85
    text(0.88, y0, "Combined score", 13.5, 700)
    for k, lv in enumerate(LEVELS):
        text(6.6 + k * 1.7, y0, LABEL[lv], 13.5, 700, ha="right")
    text(12.6, y0, "$ / task (Low to Max)", 13.5, 700, ha="right")
    for j, m in enumerate(ORDER):
        y = y0 + 0.4 * (j + 1)
        bold = m == "dsflash"
        text(
            0.88,
            y,
            f"{NAMES[m]}  ·  {HARNESS[m]}",
            13,
            700 if bold else 400,
            color=INK if bold else "#3d3d39",
        )
        for k, lv in enumerate(LEVELS):
            text(
                6.6 + k * 1.7,
                y,
                f"{data[m][lv]['combined']:.1f}",
                13,
                500 if bold else 400,
                "IBM Plex Mono",
                ha="right",
            )
        lo, hi = data[m]["low"]["usd"], max(data[m][lv]["usd"] for lv in LEVELS)
        text(
            12.6,
            y,
            (
                f"{lo:.3f} to {hi:.3f}"
                if bold
                else f"{min(data[m][lv]['usd'] for lv in LEVELS):.2f} to {hi:.2f}"
            ),
            13,
            500 if bold else 400,
            "IBM Plex Mono",
            ha="right",
        )
    footnotes(
        text,
        [
            f"Flash's cost is DeepSeek's off-peak list price; runs in its weekday peak hours bill 2x, so it paid ${min(fl[lv]['billed'] for lv in LEVELS):.3f} "
            f"to ${max(fl[lv]['billed'] for lv in LEVELS):.3f} per task.",
            *common,
        ],
        7.65,
    )
    fig.savefig(args.out / "1-summary.png", dpi=100, facecolor=PAPER)
    plt.close(fig)

    lo = min(data[m][lv]["combined"] for m in ORDER for lv in LEVELS)
    hi = max(data[m][lv]["combined"] for m in ORDER for lv in LEVELS)
    bars(
        "combined",
        "Combined score",
        "VulcanBench Frontier v4  ·  50% functional, 8.5% quality, 8.5% security, 33% judged Code quality  ·  higher is better",
        lambda m, v: f"{v:.1f}",
        (int(lo) - 1, int(hi) + 2),
        [
            "Opus 5.5's High is over 22 judged runs (a safeguard stop left one run with no code to review); every other cell is over 23.",
            *common,
        ],
        "2-combined-score.png",
    )
    top = max(data[m][lv]["minutes"] for m in ORDER for lv in LEVELS)
    bars(
        "minutes",
        "Time per task",
        "VulcanBench Frontier v4  ·  mean wall-clock minutes per task  ·  lower is faster",
        lambda m, v: f"{v:.1f}",
        (0, top * 1.15),
        [
            "Flash's Low runs straddled a macOS 26 to 27.0.1 update; re-grading its first 13 runs on 27 changed no score.",
            *common,
        ],
        "3-time-per-task.png",
    )
    top = max(data[m][lv]["usd"] for m in ORDER for lv in LEVELS)
    bars(
        "usd",
        "Cost per task",
        "VulcanBench Frontier v4  ·  mean US dollars per task at list price  ·  lower is cheaper",
        lambda m, v: f"${v:.3f}" if m == "dsflash" else f"${v:.2f}",
        (0, top * 1.15),
        [
            f"Flash at DeepSeek's off-peak list price; weekday peak hours bill 2x, so it paid ${min(fl[lv]['billed'] for lv in LEVELS):.3f} to "
            f"${max(fl[lv]['billed'] for lv in LEVELS):.3f} per task. Opus 5.5 is Claude Code's own total, Sol Codex receipts, both at list price.",
            *common,
        ],
        "4-cost-per-task.png",
    )
    (args.out / "data.json").write_text(json.dumps(data, indent=1) + "\n")
    panel = {
        "dsflash": "Muse Spark 1.3 and GPT-6.1 Sol (v3.27)",
        "opus55": "Muse Spark 1.3 and Grok 4.6 (v3.15)",
        "gpt61sol": "Muse Spark 1.3 and Grok 4.6 (v3.18)",
    }
    basis = {
        "dsflash": "DeepSeek off-peak list price; billed_usd_per_task is after peak-hour doubling",
        "opus55": "Claude Code total, reprices exactly at list price",
        "gpt61sol": "Codex receipts at list price, long-context tier not applied",
    }
    with (args.out / "deepseek-flash-vs-frontier-efforts.csv").open("w", newline="") as sink:
        writer = csv.writer(sink)
        writer.writerow(
            [
                "model",
                "effort",
                "runs",
                "judged",
                "combined",
                "combined_se",
                "code_quality_panel",
                "passed",
                "usd_per_task",
                "billed_usd_per_task",
                "minutes",
                "cost_basis",
            ]
        )
        for m in ORDER:
            for lv in LEVELS:
                r = data[m][lv]
                writer.writerow(
                    [
                        m,
                        lv,
                        r["n"],
                        r.get("judged", r["n"]),
                        f"{r['combined']:.4f}",
                        f"{r['combined_se']:.4f}",
                        panel[m],
                        r["passed"],
                        f"{r['usd']:.6f}",
                        f"{r['billed']:.6f}" if "billed" in r else "",
                        f"{r['minutes']:.2f}",
                        basis[m],
                    ]
                )
    print(json.dumps({m: {lv: round(data[m][lv]["combined"], 2) for lv in LEVELS} for m in ORDER}))


if __name__ == "__main__":
    main()
