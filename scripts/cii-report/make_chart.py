#!/usr/bin/env python3
"""Render the CII v1 results chart (brand-styled PNG).

Reads per-task run data aggregated by build_data() below directly from ./runs,
writes docs/results/cii-v1-2026-08/cii-v1-results.png.

Chart-integrity rules (CLAUDE.md): per-column run counts stay visible, ±1
stderr whiskers stay visible, coverage caveats stay footnoted.

Coverage rule: the chart reports the SYMMETRIC set, tasks measured for all
three models. Tasks recycled into cii-v1 after the August sweep (and the
pydantic task, which the host CLI runner cannot measure) are excluded and
footnoted; build_data() fails loudly if the symmetric set drifts from the
expected exclusion list, so a silent coverage change cannot ship.
"""

from __future__ import annotations

import glob
import json
import math
import os
import statistics
from pathlib import Path

os.environ.setdefault("MPLBACKEND", "Agg")

import matplotlib.font_manager as fm
import matplotlib.pyplot as plt
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "scripts" / "rankings-chart"
OUT = ROOT / "docs" / "results" / "cii-v1-2026-08" / "cii-v1-results.png"

for f in ASSETS.glob("chakra-*.ttf"):
    fm.fontManager.addfont(str(f))
CHAKRA = "Chakra Petch"
CHAKRA_MED = "Chakra Petch Medium"
CHAKRA_SEMI = "Chakra Petch SemiBold"

# (display name, harness, color). Sonnet's deep rose is the validated pick
# next to Anthropic clay and OpenAI green: it clears the colorblind and
# normal-vision separation checks on light AND dark surfaces, which no
# darker clay/sienna shade does (worst remaining pair is the pre-existing
# clay/green protan WARN, mitigated by the legend and direct labels).
OPUS = ("Claude Opus 5", "Claude Code CLI", "#D97757")
SONNET = ("Claude Sonnet 5", "Claude Code CLI", "#B03A55")
CODEX = ("GPT 5.6 Sol", "Codex CLI", "#10A37F")

OP = "claude-code:claude-opus-5"
SN = "claude-code:claude-sonnet-5"
C = "codex:gpt-5.6-sol"
MODELS = [(OPUS, OP), (SONNET, SN), (CODEX, C)]

# Tasks without symmetric three-model coverage, with the reason footnoted.
EXCLUDED = {
    "oss-pydantic-none-discriminator",  # needs per-task venv; Docker/API only
    "oss-sqlglot-pushdown-semantics",  # recycled into v1 after the sweep
    "oss-zod-fromjsonschema-epic",  # recycled into v1 after the sweep
    "env-ledger-concurrent-transfers",  # recycled into v1 after the sweep
}


def build_data():
    with open(ROOT / "tasks/cii-v1/suite.json") as fh:
        suite = json.load(fh)
    tasks = [t for t in suite["tasks"] if t not in EXCLUDED]
    data = {t: {C: [], OP: [], SN: []} for t in tasks}
    for t in tasks:
        for p in glob.glob(str(ROOT / f"runs/{t}-*/summary.json")):
            with open(p) as fh:
                s = json.load(fh)
            m, fv = s.get("model"), (s.get("scores") or {}).get("functional")
            if m in data[t] and fv is not None:
                data[t][m].append(fv)
    asymmetric = [t for t in tasks if not all(data[t][m] for _, m in MODELS)]
    if asymmetric:
        raise SystemExit(
            f"tasks without full three-model coverage (add to EXCLUDED or measure): {asymmetric}"
        )
    return tasks, data


def agg(tasks, data, m):
    rates = {
        t: sum(1 for x in data[t][m] if x == 1.0) / len(data[t][m]) for t in tasks if data[t][m]
    }
    vals = list(rates.values())
    return {
        "rates": rates,
        "p1": sum(vals) / len(vals),
        "se": statistics.pstdev(vals) / math.sqrt(len(vals)),
        "n_tasks": len(vals),
        "n_runs": sum(len(data[t][m]) for t in tasks),
    }


def main():  # noqa: PLR0915
    tasks, data = build_data()
    aggs = [(spec, agg(tasks, data, m)) for spec, m in MODELS]

    fig = plt.figure(figsize=(12.8, 8.0), dpi=200, facecolor="white")

    # header
    logo = Image.open(ASSETS / "vb_logo_rounded.png")
    ax_logo = fig.add_axes([0.045, 0.895, 0.055, 0.088])
    ax_logo.imshow(logo)
    ax_logo.axis("off")
    fig.text(0.115, 0.945, "VulcanBench", family=CHAKRA_SEMI, fontsize=21, color="#0b0b0b")
    fig.text(
        0.115,
        0.905,
        "Coding Intelligence Index v1, frontier results",
        family=CHAKRA_MED,
        fontsize=12.5,
        color="#0b0b0b",
    )
    fig.text(0.955, 0.945, "August 2026", family=CHAKRA, fontsize=10.5, color="#666666", ha="right")

    # left panel: headline pass@1
    axL = fig.add_axes([0.055, 0.235, 0.30, 0.60])
    for i, ((name, harness, color), a) in enumerate(aggs):
        axL.bar(i, a["p1"], width=0.62, color=color, zorder=3)
        axL.errorbar(i, a["p1"], yerr=a["se"], color="#0b0b0b", capsize=5, lw=1.4, zorder=4)
        axL.text(
            i,
            a["p1"] + a["se"] + 0.035,
            f"{a['p1'] * 100:.1f}%",
            ha="center",
            family=CHAKRA_SEMI,
            fontsize=14,
            color="#0b0b0b",
        )
        axL.text(i, -0.075, name, ha="center", family=CHAKRA_MED, fontsize=9.5, color="#0b0b0b")
        axL.text(
            i, -0.135, f"via {harness}", ha="center", family=CHAKRA, fontsize=8, color="#666666"
        )
        axL.text(
            i,
            -0.19,
            f"n={a['n_runs']} runs / {a['n_tasks']} tasks",
            ha="center",
            family=CHAKRA,
            fontsize=8,
            color="#666666",
        )
    axL.set_ylim(0, 1.12)
    axL.set_xlim(-0.6, 2.6)
    axL.set_xticks([])
    axL.set_yticks([0, 0.25, 0.5, 0.75, 1.0])
    axL.set_yticklabels(["0%", "25%", "50%", "75%", "100%"], family=CHAKRA, fontsize=9)
    axL.spines[["top", "right"]].set_visible(False)
    axL.set_title("pass@1 (functional == 1.0)", family=CHAKRA_MED, fontsize=11.5, pad=12)

    # right panel: the tasks that separate the models
    interesting = [
        t for t in tasks if any(data[t][m] and min(1.0, *(data[t][m])) < 1.0 for _, m in MODELS)
    ]
    interesting.sort(key=lambda t: sum(a["rates"].get(t, 1) for _, a in aggs))
    axR = fig.add_axes([0.50, 0.235, 0.46, 0.60])
    # fixed per-model lanes inside each row so coincident values never occlude
    LANE = [0.16, 0.0, -0.16]
    for j, t in enumerate(interesting):
        y = len(interesting) - 1 - j
        rs = [a["rates"].get(t) for _, a in aggs]
        known = [r for r in rs if r is not None]
        if len(known) > 1:
            axR.plot([min(known), max(known)], [y, y], color="#cccccc", lw=2.5, zorder=2)
        for ((_, _, color), _a), r, dy in zip(aggs, rs, LANE, strict=True):
            if r is not None:
                axR.scatter(
                    r, y + dy, s=90, color=color, zorder=3, edgecolors="white", linewidths=1.2
                )
        label = t.replace("oss-", "")
        ns = {len(data[t][m]) for _, m in MODELS if data[t][m]}
        n_str = f"n={ns.pop()}\N{MULTIPLICATION SIGN}3" if len(ns) == 1 else "n varies"
        axR.text(
            -0.04,
            y,
            f"{label}  ({n_str})",
            ha="right",
            va="center",
            family=CHAKRA,
            fontsize=9,
            color="#0b0b0b",
        )
    axR.set_xlim(-0.05, 1.08)
    axR.set_ylim(-0.7, len(interesting) - 0.3)
    axR.set_yticks([])
    axR.set_xticks([0, 1 / 3, 2 / 3, 1.0])
    axR.set_xticklabels(["0/3", "1/3", "2/3", "3/3"], family=CHAKRA, fontsize=9)
    axR.spines[["top", "right", "left"]].set_visible(False)
    axR.set_title(
        "per-task solve rate, every task any model missed",
        family=CHAKRA_MED,
        fontsize=11.5,
        pad=12,
    )
    for name, _h, color in (OPUS, SONNET, CODEX):
        axR.scatter([], [], s=90, color=color, label=name)
    axR.legend(loc="upper right", frameon=False, prop=fm.FontProperties(family=CHAKRA, size=9))

    fig.text(
        0.06,
        0.075,
        "37 of 41 tasks: symmetric three-model coverage only. Excluded: oss-pydantic-none-discriminator (needs a per-task venv the "
        "host runner lacks) plus 3 tasks recycled into the suite after the sweep. Saturated tasks n=1; every miss re-measured at n=3.",
        family=CHAKRA,
        fontsize=8,
        color="#666666",
    )
    fig.text(
        0.06,
        0.045,
        "All models billed via subscriptions (Codex CLI / Claude Code CLI agent harnesses; scores are model+harness, not raw API). "
        "All tasks from upstream PRs merged May to Aug 2026, after every training cutoff. Whiskers: ±1 stderr across tasks.",
        family=CHAKRA,
        fontsize=8,
        color="#666666",
    )

    OUT.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT, facecolor="white", bbox_inches="tight")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
