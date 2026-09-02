"""VulcanBench Technical Report No. 21 card: Claude Fable 5.1 on CII v4.

Follows the house single-model report-card format (see Reports No. 10, 12, 19):
1600x900 parchment, serif, small-caps masthead over a rule, centred title and
prose headline, then a two-column body with numbered tables on the left and a
two-panel figure on the right, closing with a Method paragraph.

    python scripts/cii-v4-board/make_report_card.py

Chart-integrity rules that must survive edits (see CLAUDE.md): run counts stay
visible, the pass@1 bar keeps its +/-1 stderr whisker, the unsolved table keeps
the exact families that failed, and the one-run-per-task and host-execution
caveats stay in the Method paragraph.
"""

from __future__ import annotations

import argparse
import json
import statistics
import textwrap
from collections import defaultdict
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "docs" / "results" / "cii-v4-fable51-2026-09" / "report21-fable51-cii-v4.png"
MODEL = "claude-code:claude-fable-5-1"

PAPER = "#f7f5f0"
INK = "#1c1a17"
BAND = "#efeade"
RULE = "#1c1a17"
NAVY = "#27425f"
MID = "#5d7ba0"
PALE = "#8ba3c4"
GREY = "#6b675f"

# Fable 5.1 list price; cache reads ($0.25/M) are not modelled, so the
# api-equivalent cost this yields is an upper bound.
IN_RATE, OUT_RATE = 10.00, 50.00

SERIF = "Baskerville"
DATE = "September 2, 2026"
NUMBER = "No. 21"


def load(runs_glob: str):
    suite = json.loads(
        (ROOT / "tasks" / "coding-intelligence-index-v4" / "suite.json").read_text()
    )["tasks"]
    by_task = defaultdict(list)
    for path in sorted(ROOT.glob(runs_glob)):
        try:
            s = json.loads(path.read_text())
        except Exception:
            continue
        if s.get("model") != MODEL or s.get("task_id") not in suite:
            continue
        # Default-effort card: never let sweep runs leak into these figures.
        if (s.get("effort") or {}).get("requested"):
            continue
        functional = (s.get("scores") or {}).get("functional")
        if functional is None:
            continue
        missed = [k for k, v in (s.get("verifier") or {}).get("fail_to_pass", {}).items() if not v]
        tok = s.get("tokens") or {}
        cost = (
            int(tok.get("prompt") or 0) * IN_RATE + int(tok.get("completion") or 0) * OUT_RATE
        ) / 1e6
        by_task[s["task_id"]].append(
            (
                float(functional),
                s["duration_s"] / 60.0,
                int(s.get("total_tokens") or 0),
                missed,
                cost,
            )
        )
    return by_task


def sliced(by_task, cutoff):
    rates = [
        sum(1 for f, d, *_ in runs if f == 1.0 and (cutoff is None or d <= cutoff)) / len(runs)
        for runs in by_task.values()
    ]
    mean = sum(rates) / len(rates)
    n = len(rates)
    var = sum((r - mean) ** 2 for r in rates) / (n - 1) if n > 1 else 0.0
    return mean, (var / n) ** 0.5


def short(task_id: str) -> str:
    return task_id.replace("legacy-", "").split("-binary")[0].split("-order")[0].split("-store")[0]


def main() -> None:  # noqa: PLR0912, PLR0915, linear top-to-bottom page layout
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--runs",
        default="runs-board/*/summary.json",
        help="glob of run summaries, relative to the repo root. Defaults to the live "
        "pool; point it at docs/results/cii-v4-fable51-2026-09/run-summaries/*/summary.json "
        "to regenerate from the committed snapshot on any machine.",
    )
    args = ap.parse_args()
    data = load(args.runs)
    if not data:
        raise SystemExit(f"no matching runs for {args.runs!r}")
    runs = [(short(t), f, d, tok, m, c) for t, v in data.items() for f, d, tok, m, c in v]
    n = len(runs)
    solved = [r for r in runs if r[1] == 1.0]
    unsolved = sorted((r for r in runs if r[1] < 1.0), key=lambda r: r[1])
    full, err = sliced(data, None)
    w10, _ = sliced(data, 10)
    w30, _ = sliced(data, 30)
    med_solved = statistics.median([r[2] for r in solved])
    med_unsolved = statistics.median([r[2] for r in unsolved])
    med_tok_solved = statistics.median([r[3] for r in solved if r[3]])
    med_tok_unsolved = statistics.median([r[3] for r in unsolved if r[3]])
    total_h = sum(r[2] for r in runs) / 60
    total_cost = sum(r[5] for r in runs)

    fig = plt.figure(figsize=(16, 9), dpi=100)
    fig.patch.set_facecolor(PAPER)

    def txt(x, y, s, size=9, weight="normal", style="normal", color=INK, ha="left", va="center"):
        return fig.text(
            x,
            y,
            s,
            fontsize=size,
            fontweight=weight,
            fontstyle=style,
            color=color,
            ha=ha,
            va=va,
            fontfamily=SERIF,
        )

    def rule(y, x0=0.045, x1=0.955, lw=1.1, color=RULE):
        fig.add_artist(plt.Line2D([x0, x1], [y, y], lw=lw, color=color, transform=fig.transFigure))

    # --- masthead ---------------------------------------------------------
    txt(0.045, 0.955, "V U L C A N B E N C H", 11, "bold")
    txt(0.163, 0.955, f"T E C H N I C A L   R E P O R T   {NUMBER}", 11, color=GREY)
    txt(
        0.955,
        0.955,
        f"S E P T E M B E R   2 0 2 6   ·   2 3   T A S K S   ·   1   M O D E L   ·   "
        f"2 3   R U N S   ·   1   E F F O R T   L E V E L   ·   {total_h:.1f}  H   ·   "
        f"$ {total_cost:.0f}",
        9,
        color=GREY,
        ha="right",
    )
    rule(0.936, lw=1.6)
    rule(0.9315, lw=0.7)

    # --- title block ------------------------------------------------------
    fig.text(
        0.5,
        0.876,
        "Claude Fable 5.1",
        fontsize=25,
        fontweight="bold",
        color=INK,
        ha="center",
        va="center",
        fontfamily=SERIF,
    )
    fig.text(
        0.5,
        0.828,
        "Twenty-three opaque legacy binaries, reconstructed from behavior alone and graded "
        "on byte parity",
        fontsize=11.5,
        fontstyle="italic",
        color=INK,
        ha="center",
        va="center",
        fontfamily=SERIF,
    )

    headline = (
        f"$\\bf{{It\\ solves\\ {len(solved)}\\ of\\ 23,\\ and\\ the\\ clock\\ is\\ the\\ "
        f"story.}}$ Only {w10 * 100:.0f}% of the suite falls in the first ten minutes and "
        f"{w30 * 100:.0f}% by thirty, against {full * 100:.1f}% given the full budget. The four "
        f"it never finishes are the four newest tasks, and they cost it a median of "
        f"{med_unsolved:.0f} minutes each, {med_unsolved / med_solved:.0f} times what a solved "
        f"task takes."
    )
    for i, line in enumerate(textwrap.wrap(headline, width=118)):
        fig.text(
            0.5,
            0.782 - i * 0.032,
            line,
            fontsize=11.5,
            color=INK,
            ha="center",
            va="center",
            fontfamily=SERIF,
        )

    # --- TABLE 1 ----------------------------------------------------------
    txt(0.045, 0.690, "T A B L E   1 .", 9, "bold")
    for i, line in enumerate(
        textwrap.wrap(
            "Rows are wall-clock budgets at the shipped default effort, not effort levels. "
            "One attempt per task, read from recorded durations.",
            width=76,
        )
    ):
        txt(0.108 if i == 0 else 0.045, 0.690 - i * 0.026, line, 9)

    cols = [
        (0.045, "Time budget", "left"),
        (0.205, "pass@1", "right"),
        (0.285, "Solved", "right"),
        (0.390, "Tokens/task", "right"),
        (0.475, "Time/task", "right"),
        (0.560, "$/task", "right"),
    ]
    ty = 0.612
    rule(ty + 0.020, 0.045, 0.560)
    for x, label, ha in cols:
        txt(x, ty, label, 9.5, "bold", ha=ha)
    rule(ty - 0.016, 0.045, 0.560, lw=0.7)

    def within(cutoff, idx):
        vals = [r[idx] for r in runs if cutoff is None or r[2] <= cutoff]
        return statistics.median(vals) if vals else None

    rows = [
        ("within 10 min", w10, sum(1 for r in solved if r[2] <= 10), 10),
        ("within 30 min", w30, sum(1 for r in solved if r[2] <= 30), 30),
        ("full budget (10 h)", full, len(solved), None),
    ]
    for i, (label, rate, ns, cut) in enumerate(rows):
        y = ty - 0.050 - i * 0.040
        last = i == len(rows) - 1
        if last:
            fig.patches.append(
                plt.Rectangle(
                    (0.045, y - 0.017),
                    0.401,
                    0.034,
                    transform=fig.transFigure,
                    facecolor=BAND,
                    edgecolor="none",
                    zorder=0,
                )
            )
        w = "bold" if last else "normal"
        st = "normal" if last else "italic"
        txt(0.045, y, label, 9.5, w, st)
        txt(0.205, y, f"{rate * 100:.1f}%", 9.5, w, ha="right")
        txt(0.285, y, f"{ns}/23", 9.5, w, ha="right")
        txt(0.390, y, f"{within(cut, 2 + 1) / 1000:.0f} K", 9.5, w, ha="right")
        txt(0.475, y, f"{within(cut, 2):.1f} min", 9.5, w, ha="right")
        txt(0.560, y, f"${within(cut, 5):.2f}", 9.5, w, ha="right")
    rule(ty - 0.050 - (len(rows) - 1) * 0.040 - 0.019, 0.045, 0.560)

    note = (
        f"$\\bf{{1.}}$ pass@1 carries $\\pm${err * 100:.1f} points at one attempt per task. "
        f"Median {med_tok_solved / 1000:.0f} K tokens on a solved task against "
        f"{med_tok_unsolved / 1e6:.1f} M on an unsolved one: failures are long, expensive, "
        f"unfinished reconstructions rather than quick wrong answers."
    )
    for i, line in enumerate(textwrap.wrap(note, width=78)):
        txt(0.045, 0.434 - i * 0.025, line, 9)

    # --- TABLE 2 ----------------------------------------------------------
    txt(0.045, 0.345, "T A B L E   2 .", 9, "bold")
    txt(0.108, 0.345, "The four it did not finish. Every one is a wave-11 engine.", 9)
    t2 = [
        (0.045, "Task", "left"),
        (0.205, "Score", "right"),
        (0.276, "Time", "right"),
        (0.330, "Families missed", "left"),
    ]
    ty2 = 0.303
    rule(ty2 + 0.019, 0.045, 0.560)
    for x, label, ha in t2:
        txt(x, ty2, label, 9.5, "bold", ha=ha)
    rule(ty2 - 0.015, 0.045, 0.560, lw=0.7)
    for i, (name, score, dur, _tok, missed, _cost) in enumerate(unsolved):
        y = ty2 - 0.045 - i * 0.034
        txt(0.045, y, name, 9.5, style="italic")
        txt(0.205, y, f"{score:.3f}", 9.5, ha="right")
        txt(0.276, y, f"{dur:.0f} min", 9.5, ha="right")
        txt(0.330, y, ", ".join(missed), 9.5, color=GREY)
    rule(ty2 - 0.045 - (len(unsolved) - 1) * 0.034 - 0.017, 0.045, 0.560)

    # --- FIGURE 1 ---------------------------------------------------------
    txt(0.600, 0.690, "F I G U R E   1 .", 9, "bold")
    for i, line in enumerate(
        textwrap.wrap(
            "Accuracy against the clock, and what failure costs. Shades darken as the budget "
            "opens up. The right panel is median wall clock per run.",
            width=74,
        )
    ):
        txt(0.669 if i == 0 else 0.600, 0.690 - i * 0.026, line, 9)

    axa = fig.add_axes([0.600, 0.325, 0.163, 0.295])
    axa.set_facecolor(PAPER)
    vals = [w10, w30, full]
    axa.bar(
        [0, 1, 2],
        [v * 100 for v in vals],
        0.62,
        color=[PALE, MID, NAVY],
        zorder=3,
        yerr=[0, 0, err * 100],
        error_kw={"ecolor": INK, "elinewidth": 1.1, "capsize": 4, "zorder": 4},
    )
    for x, v in zip([0, 1, 2], vals, strict=True):
        axa.text(
            x, v * 100 + 3, f"{v * 100:.1f}", ha="center", fontsize=9.5, fontfamily=SERIF, color=INK
        )
    axa.set_xticks([0, 1, 2])
    axa.set_xticklabels(
        ["10 min", "30 min", "full"], fontsize=9, fontfamily=SERIF, fontstyle="italic", color=INK
    )
    axa.set_ylim(0, 100)

    axb = fig.add_axes([0.800, 0.325, 0.155, 0.295])
    axb.set_facecolor(PAPER)
    axb.bar([0, 1], [med_solved, med_unsolved], 0.62, color=[MID, NAVY], zorder=3)
    for x, v in zip([0, 1], [med_solved, med_unsolved], strict=True):
        axb.text(x, v + 5, f"{v:.0f}", ha="center", fontsize=9.5, fontfamily=SERIF, color=INK)
    axb.set_xticks([0, 1])
    axb.set_xticklabels(
        ["solved", "unsolved"], fontsize=9, fontfamily=SERIF, fontstyle="italic", color=INK
    )
    axb.set_ylim(0, med_unsolved * 1.28)

    for ax in (axa, axb):
        for side in ("top", "right", "left"):
            ax.spines[side].set_visible(False)
        ax.spines["bottom"].set_color(INK)
        ax.tick_params(length=0, colors=INK)
        for lab in ax.get_yticklabels():
            lab.set_fontsize(8.5)
            lab.set_fontfamily(SERIF)
            lab.set_color(GREY)
    fig.text(
        0.6815,
        0.286,
        "Accuracy",
        fontsize=10,
        fontstyle="italic",
        color=INK,
        ha="center",
        fontfamily=SERIF,
    )
    fig.text(
        0.6815,
        0.258,
        "pass@1 (%), 23 tasks",
        fontsize=8.5,
        color=GREY,
        ha="center",
        fontfamily=SERIF,
    )
    fig.text(
        0.8775,
        0.286,
        "Cost of failure",
        fontsize=10,
        fontstyle="italic",
        color=INK,
        ha="center",
        fontfamily=SERIF,
    )
    fig.text(
        0.8775,
        0.258,
        "median minutes per run",
        fontsize=8.5,
        color=GREY,
        ha="center",
        fontfamily=SERIF,
    )
    for i, line in enumerate(
        textwrap.wrap(
            "Both panels are zero-based. The accuracy bar carries one standard error; the "
            "time-sliced bars are re-reads of the same runs and carry none.",
            width=80,
        )
    ):
        fig.text(
            0.600,
            0.212 - i * 0.024,
            line,
            fontsize=8.5,
            fontstyle="italic",
            color=GREY,
            fontfamily=SERIF,
        )

    effort_note = (
        "$\\bf{Effort.}$ Every figure here is the shipped default reasoning effort. The "
        "low-through-max sweep is not yet measured for this model."
    )
    for i, line in enumerate(textwrap.wrap(effort_note, width=80)):
        txt(0.600, 0.168 - i * 0.024, line, 8.5, color=GREY)

    # --- method footer ----------------------------------------------------
    rule(0.113)
    method = (
        "$\\bf{Method.}$ Coding Intelligence Index v4: 23 hand-authored tasks, each shipping a "
        "stripped legacy binary whose undocumented behavior is the contract, a drifted written "
        "spec, and hidden tests grading byte parity over generated corpora. Claude Code CLI on a "
        "Claude Max subscription, host execution on one Apple Silicon machine, shipped default "
        "reasoning effort, judges disabled, uniform 10-hour timeout, one attempt per task. "
        "Admission required a weaker reference to solve at most one run in three and a stronger "
        "reference to need ten minutes median or miss outright. Dollar figures are api-equivalent "
        "at list price (\\$10 and \\$50 per M tokens), an upper bound: these runs were billed to "
        "a subscription and cache reads are not modelled."
    )
    for i, line in enumerate(textwrap.wrap(method, width=196)):
        txt(0.045, 0.085 - i * 0.020, line, 8.2)
    txt(
        0.955,
        0.022,
        f"V U L C A N B E N C H   ·   O P E N   S O U R C E   ·   {DATE}",
        8.5,
        color=GREY,
        ha="right",
    )

    fig.savefig(OUT, facecolor=PAPER)
    print(f"{len(data)}/23 tasks, {n} runs, pass@1 {full:.4f} +/- {err:.4f}")
    print("wrote", OUT)


if __name__ == "__main__":
    main()
