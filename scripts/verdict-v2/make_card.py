#!/usr/bin/env python3
"""Render the VulcanBench Verdict v2 results card for Jev 1.13.0.

House layout, after scripts/verdict-v1/make_jev_card.py and the Frontier v4
cards (scripts/cii-v4-board/make_sol_v37_card.py): masthead, title, legend,
two charts, one table, short notes. The left column holds the four indices and
Table 1; the right column holds skill for all 20 question families as paired
horizontal bars, so every family name stays readable. How the benchmark works
is explained on the report page, not on the card.

Skill is 100 * (accuracy - floor) / (1 - floor): 0 is no better than the best
dumb strategy (most common answer, guessing or a surface shortcut), 100 is
perfect. Whiskers are +/-1 standard error from the export.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch
from matplotlib.transforms import blended_transform_factory

REPO = Path(__file__).resolve().parents[2]
RESULTS_DIR = REPO / "docs" / "results" / "verdict-v2-2026-09"
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
JEV = INK  # the subject in brand black
REF = "#10A37F"  # same colour as the v1 control bar (OpenAI lab colour, GPT-6 Astra)
JEV_WHISKER = "#8f8e86"  # mid grey reads on the black bar and on paper
WIDTH_IN, HEIGHT_IN = 16, 11.5
LEFT, RIGHT = 0.06, 0.94
COL_SPLIT = 0.475  # right edge of the left column
RIGHT_COL = 0.525  # left edge of the right column
JEV_ROW = "jev-1.13.0"
REF_ROW = "gpt-6-astra-high (reference)"
PILLARS = (("software", "Software"), ("general", "General"))
# Plain names, shortened from each family's summary in the export.
NAMES = {
    "code-output": "What does this program print",
    "type-check-pair": "Which snippet type-checks",
    "patch-pair": "Which patch passes hidden tests",
    "failing-test": "Which hidden test fails",
    "fix-file": "Which file the fix touches",
    "bug-function": "Which function holds the bug",
    "vuln-pair": "Which version is vulnerable",
    "weakness-class": "Which weakness class",
    "mutant-kill": "Does this test catch the change",
    "expected-value": "Is the expected value right",
    "incident-root-cause": "Which service caused the incident",
    "semver-impact": "Patch, minor or major bump",
    "constraint-pick": "Which assignment fits every rule",
    "entailment": "Does the conclusion follow",
    "word-problem": "Which word-problem answer",
    "estimate-band": "Which band holds the quantity",
    "table-lookup": "Which table row answers",
    "table-count-band": "How many table rows match",
    "policy-decision": "Is the case allowed by policy",
    "policy-clause": "Which policy clause decides",
}
INDICES = (
    ("verdict_index", "Verdict Index\n20 families", 1),
    ("software_index", "Software\n12 families", 1),
    ("general_index", "General\n8 families", 1),
    ("calibration_index", "Calibration\n(Brier skill x100)\n20 families", 100),
)


class Card:
    def __init__(self) -> None:
        self.fig = plt.figure(figsize=(WIDTH_IN, HEIGHT_IN), dpi=150, facecolor=PAPER)

    @staticmethod
    def yf(inches: float) -> float:
        """Figure fraction for a position measured in inches from the top edge."""
        return 1 - inches / HEIGHT_IN

    def text(
        self,
        x,
        y_in,
        label,
        size=16,
        bold=False,
        heading=False,
        numeric=False,
        ha="left",
        color=INK,
    ):
        if numeric:
            family, weight = "IBM Plex Mono", 500 if bold else 400
        elif heading:
            family, weight = (
                ("Chakra Petch SemiBold" if bold else "Chakra Petch Medium"),
                (600 if bold else 500),
            )
        else:
            family, weight = "Geist", 700 if bold else 400
        self.fig.text(
            x,
            self.yf(y_in),
            label,
            fontsize=size,
            fontfamily=family,
            weight=weight,
            ha=ha,
            va="center",
            color=color,
        )

    def line(self, x1, x2, y_in, color=RULE, width=0.8):
        self.fig.add_artist(
            plt.Line2D(
                [x1, x2],
                [self.yf(y_in), self.yf(y_in)],
                transform=self.fig.transFigure,
                color=color,
                lw=width,
            )
        )

    def swatch(self, x, y_in, color):
        self.fig.add_artist(
            plt.Line2D(
                [x],
                [self.yf(y_in)],
                transform=self.fig.transFigure,
                marker="s",
                color=color,
                markersize=10,
                markeredgecolor=INK,
                markeredgewidth=0.5,
                linestyle="none",
            )
        )

    def axes(self, x0, w, top_in, h_in):
        ax = self.fig.add_axes([x0, self.yf(top_in + h_in), w, h_in / HEIGHT_IN], facecolor=PAPER)
        ax.tick_params(axis="both", length=0, labelsize=11, pad=6)
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.set_axisbelow(True)
        return ax


def masthead(card: Card) -> None:
    logo = card.fig.add_axes([LEFT, card.yf(0.86), 0.42 / WIDTH_IN, 0.42 / HEIGHT_IN])
    mark = logo.imshow(plt.imread(REPO / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    card.text(LEFT + 0.038, 0.65, "VulcanBench", 20, True, heading=True)
    card.text(RIGHT, 0.65, "September 2026", 14, ha="right", color=MUTED)
    card.line(LEFT, RIGHT, 1.05, INK, 1.2)


def title(card: Card, data: dict) -> None:
    items = [f["test_items"] for f in data["families"].values()]
    card.text(LEFT, 1.66, "VulcanBench Verdict v2: Jev 1.13.0", 33, True, heading=True)
    card.text(
        LEFT,
        2.08,
        "Skill on 20 question families: 0 is no better than the best dumb strategy "
        "(most common answer, guessing or a surface shortcut), 100 is perfect.",
        14,
        color=MUTED,
    )
    card.swatch(LEFT + 0.005, 2.5, JEV)
    card.text(LEFT + 0.017, 2.5, "Jev 1.13.0  ·  TypeSafe API", 14.5, True)
    card.swatch(0.272, 2.5, REF)
    card.text(0.284, 2.5, "GPT-6 Astra, high effort, Codex  ·  reference, not ranked", 14.5)
    card.text(
        RIGHT,
        2.5,
        f"n={sum(items):,} test items per model, {min(items)} to {max(items)} per family",
        13,
        ha="right",
        color=MUTED,
    )


def value_label(ax, x, y, text, horizontal=False, size=10):
    offset = (4, 0) if horizontal else (0, 4)
    ax.annotate(
        text,
        (x, y),
        xytext=offset,
        textcoords="offset points",
        ha="left" if horizontal else "center",
        va="center" if horizontal else "bottom",
        fontsize=size,
        fontfamily="IBM Plex Mono",
        color=INK,
    )


def index_chart(card: Card, data: dict) -> None:
    jev = data["results"][JEV_ROW]["indices"]
    ref = data["results"][REF_ROW]["indices"]
    card.text(LEFT, 2.98, "Indices: Jev against the reference", 20, True, heading=True)
    card.text(
        LEFT,
        3.29,
        "Mean family skill, 0 to 100  ·  higher is better  ·  whiskers ±1 standard error",
        11.5,
        color=MUTED,
    )
    ax = card.axes(0.088, COL_SPLIT - 0.088, 3.58, 1.55)
    ax.grid(axis="y", color=RULE, linewidth=0.5)
    width = 0.34
    for i, (key, _label, scale) in enumerate(INDICES):
        for offset, stats, color, ecolor in (
            (-width / 2, jev[key], JEV, JEV_WHISKER),
            (width / 2, ref[key], REF, INK),
        ):
            value, se = stats["value"] * scale, stats["se"] * scale
            ax.bar(
                i + offset,
                value,
                width,
                color=color,
                edgecolor=INK,
                linewidth=0.5,
                yerr=se if se else None,
                error_kw={"ecolor": ecolor, "elinewidth": 1.1, "capsize": 3},
                zorder=2,
            )
            value_label(ax, i + offset, value + se, f"{value:.0f}")
    # Calibration is a different quantity (Brier skill), so it sits behind a divider.
    ax.axvline(2.5, color=MUTED, linewidth=0.8, linestyle=(0, (3, 3)))
    ax.set_xticks(range(len(INDICES)), [label for _, label, _ in INDICES], fontsize=10.5)
    ax.set_xlim(-0.55, len(INDICES) - 0.45)
    ax.set_ylim(0, 118)
    ax.set_yticks(range(0, 101, 25))


def family_order(data: dict, pillar: str) -> list[str]:
    jev = data["results"][JEV_ROW]["families"]
    return sorted(data["pillars"][pillar], key=lambda f: -jev[f]["skill"])


def family_chart(card: Card, data: dict) -> None:
    jev = data["results"][JEV_ROW]["families"]
    ref = data["results"][REF_ROW]["families"]
    indices = {row: data["results"][row]["indices"] for row in (JEV_ROW, REF_ROW)}
    card.text(RIGHT_COL, 2.98, "Skill on every question family", 20, True, heading=True)
    card.text(
        RIGHT_COL,
        3.29,
        "0 = best dumb strategy, 100 = perfect  ·  sorted by Jev within each pillar  ·  "
        "whiskers ±1 SE",
        11.5,
        color=MUTED,
    )
    x0, x1 = 0.728, RIGHT
    ax = card.axes(x0, x1 - x0, 3.6, 6.55)
    ax.grid(axis="x", color=RULE, linewidth=0.5)
    ax.spines["left"].set_visible(False)
    to_label = blended_transform_factory(card.fig.transFigure, ax.transData)
    bar_h, y = 0.36, 0.0
    for pillar, name in PILLARS:
        ax.text(RIGHT_COL, y, name, transform=to_label, fontsize=13.5, va="center",
                fontfamily="Chakra Petch Medium", weight=500, color=INK)  # fmt: skip
        ax.text(
            RIGHT_COL + 0.058,
            y,
            f"index {indices[JEV_ROW][pillar + '_index']['value']:.1f} Jev, "
            f"{indices[REF_ROW][pillar + '_index']['value']:.1f} reference",
            transform=to_label,
            fontsize=10,
            va="center",
            color=MUTED,
        )
        y += 0.95
        for family in family_order(data, pillar):
            ax.text(RIGHT_COL, y, NAMES[family], transform=to_label, fontsize=10.5,
                    va="center", color=INK)  # fmt: skip
            ax.text(
                x0 - 0.006,
                y,
                f"n={data['families'][family]['test_items']}",
                transform=to_label,
                fontsize=9.5,
                va="center",
                ha="right",
                fontfamily="IBM Plex Mono",
                color=MUTED,
            )
            for offset, stats, color, ecolor in (
                (-bar_h / 2, jev[family], JEV, JEV_WHISKER),
                (bar_h / 2, ref[family], REF, INK),
            ):
                value, se = stats["skill"], stats["skill_se"]
                ax.barh(
                    y + offset,
                    value,
                    bar_h,
                    color=color,
                    edgecolor=INK,
                    linewidth=0.4,
                    xerr=se if se else None,
                    error_kw={"ecolor": ecolor, "elinewidth": 1.0, "capsize": 2},
                    zorder=2,
                )
                value_label(ax, max(value + se, 0), y + offset, f"{value:.0f}", True, 8.5)
            y += 1.0
        y += 0.45
    ax.axvline(0, color=INK, linewidth=1.0, zorder=3)
    ax.set_ylim(y - 0.7, -0.6)
    ax.set_yticks([])
    ax.set_xlim(-12, 114)
    ax.set_xticks(range(0, 101, 25))
    ax.tick_params(axis="x", labelsize=10.5)


def table(card: Card, data: dict) -> float:
    jev = data["results"][JEV_ROW]["families"]
    ref = data["results"][REF_ROW]["families"]
    indices = {row: data["results"][row]["indices"] for row in (JEV_ROW, REF_ROW)}
    shortcuts = data["shortcuts"]
    card.text(LEFT, 6.08, "Table 1  |  Every family", 14.5, heading=True)
    card.text(
        COL_SPLIT,
        6.1,
        "AUROC: 0.5 is chance, dash if none  ·  shortcut: skill over guessing",
        9.5,
        ha="right",
        color=MUTED,
    )
    card.line(LEFT, COL_SPLIT, 6.3, INK, 1.2)
    cols = (0.262, 0.312, 0.362, 0.417, COL_SPLIT)
    heads = ("Test\nitems", "Jev\nskill", "Ref.\nskill", "Jev\nAUROC", "Best\nshortcut")
    card.text(LEFT, 6.51, "Family", 11.5, True)
    for head, x in zip(heads, cols, strict=True):
        card.fig.text(x, card.yf(6.51), head, fontsize=11, weight=700, ha="right",
                      va="center", linespacing=1.0, color=INK)  # fmt: skip
    card.line(LEFT, COL_SPLIT, 6.73, INK, 0.6)
    y, step = 6.87, 0.172
    for pillar, name in PILLARS:
        families = family_order(data, pillar)
        values = (
            f"{sum(data['families'][f]['test_items'] for f in families):,}",
            f"{indices[JEV_ROW][pillar + '_index']['value']:.1f}",
            f"{indices[REF_ROW][pillar + '_index']['value']:.1f}",
            "",
            "",
        )
        if pillar != PILLARS[0][0]:
            card.line(LEFT, COL_SPLIT, y - step / 2, RULE, 0.6)
        card.text(LEFT, y, f"{name} index", 11, True)
        for value, x in zip(values, cols, strict=True):
            card.text(x, y, value, 11, True, numeric=True, ha="right")
        card.line(LEFT, COL_SPLIT, y + step / 2, RULE, 0.6)
        y += step
        for family in families:
            auroc = jev[family]["ranking"]
            values = (
                f"{data['families'][family]['test_items']}",
                f"{jev[family]['skill']:.1f}",
                f"{ref[family]['skill']:.1f}",
                f"{auroc:.2f}" if auroc is not None else "-",
                f"{shortcuts[family]['best_shortcut_skill']:.1f}",
            )
            card.text(LEFT + 0.008, y, NAMES[family], 10.5)
            for value, x in zip(values, cols, strict=True):
                card.text(x, y, value, 10.5, numeric=True, ha="right")
            y += step
    card.line(LEFT, COL_SPLIT, y - step / 2, INK, 1.2)
    return y - step / 2


def notes(card: Card, data: dict, top: float) -> None:
    run = data["run"][JEV_ROW]
    lines = (
        f"{run['answered']:,} test items; standard errors and 95% intervals resample source units. "
        f"Jev cost ${run['total_cost_usd']:.2f} in total at {run['latency_ms_p50'] / 1000:.1f} s "
        "median latency from California; it has no effort setting.",
        "Reference: GPT-6 Astra at high effort through Codex, same inputs, no tools. It shows each "
        "family is answerable and is not a leaderboard entry.",
        "The admission gate was amended after the pilot: the reference only needs skill 40, "
        "and a family is too easy when Jev reaches 90.",
    )
    for i, note in enumerate(lines):
        card.text(LEFT, top + 0.19 + 0.2 * i, note, 10.5, color=MUTED)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--results", type=Path, default=RESULTS_DIR / "verdict-v2-jev.json")
    parser.add_argument("-o", "--out", type=Path, default=RESULTS_DIR / "verdict-v2-jev.png")
    args = parser.parse_args()
    for font in (REPO / "scripts" / "rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    data = json.loads(args.results.read_text())
    missing = set(data["families"]) - set(NAMES)
    if missing:
        raise SystemExit(f"no card name for families: {sorted(missing)}")

    card = Card()
    masthead(card)
    title(card, data)
    index_chart(card, data)
    family_chart(card, data)
    notes(card, data, table(card, data))
    card.fig.savefig(args.out, facecolor=PAPER)
    plt.close(card.fig)
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
