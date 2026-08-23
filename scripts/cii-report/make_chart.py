#!/usr/bin/env python3
"""Render the CII v1 results chart (brand-styled PNG).

Reads per-task run data aggregated by build_data() below directly from ./runs,
writes docs/results/cii-v1-2026-08/cii-v1-results.png.

Chart-integrity rules (CLAUDE.md): per-column run counts stay visible, ±1
stderr whiskers stay visible, coverage caveats stay footnoted.
"""

from __future__ import annotations

import glob
import json
import math
import statistics
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.font_manager as fm
import matplotlib.pyplot as plt
from matplotlib.offsetbox import AnnotationBbox, OffsetImage
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "scripts" / "rankings-chart"
OUT = ROOT / "docs" / "results" / "cii-v1-2026-08" / "cii-v1-results.png"

for f in ASSETS.glob("chakra-*.ttf"):
    fm.fontManager.addfont(str(f))
CHAKRA = "Chakra Petch"
CHAKRA_MED = "Chakra Petch Medium"
CHAKRA_SEMI = "Chakra Petch SemiBold"

CODEX = ("GPT 5.6 Sol", "Codex CLI", "#10A37F")
OPUS = ("Claude Opus 5", "Claude Code CLI", "#D97757")
EXCLUDED = "oss-pydantic-none-discriminator"


def build_data():
    tasks = [t for t in json.load(open(ROOT / "tasks/cii-v1/suite.json"))["tasks"] if t != EXCLUDED]
    C, O = "codex:gpt-5.6-sol", "claude-code:claude-opus-5"
    data = {t: {C: [], O: []} for t in tasks}
    for t in tasks:
        for p in glob.glob(str(ROOT / f"runs/{t}-*/summary.json")):
            s = json.load(open(p))
            m, fv = s.get("model"), (s.get("scores") or {}).get("functional")
            if m in data[t] and fv is not None:
                data[t][m].append(fv)
    return tasks, data, C, O


def agg(tasks, data, m):
    rates = {t: sum(1 for x in data[t][m] if x == 1.0) / len(data[t][m]) for t in tasks if data[t][m]}
    vals = list(rates.values())
    return {
        "rates": rates,
        "p1": sum(vals) / len(vals),
        "se": statistics.pstdev(vals) / math.sqrt(len(vals)),
        "n_tasks": len(vals),
        "n_runs": sum(len(data[t][m]) for t in tasks),
    }


def main():
    tasks, data, C, O = build_data()
    a_c, a_o = agg(tasks, data, C), agg(tasks, data, O)

    fig = plt.figure(figsize=(12.8, 8.0), dpi=200, facecolor="white")

    # header
    logo = Image.open(ASSETS / "vb_logo_rounded.png")
    ax_logo = fig.add_axes([0.045, 0.895, 0.055, 0.088])
    ax_logo.imshow(logo)
    ax_logo.axis("off")
    fig.text(0.115, 0.945, "VulcanBench", family=CHAKRA_SEMI, fontsize=21, color="#0b0b0b")
    fig.text(0.115, 0.905, "Coding Intelligence Index v1 — frontier results", family=CHAKRA_MED,
             fontsize=12.5, color="#0b0b0b")
    fig.text(0.955, 0.945, "August 2026", family=CHAKRA, fontsize=10.5, color="#666666", ha="right")

    # ── left panel: headline pass@1 ─────────────────────────────
    axL = fig.add_axes([0.06, 0.235, 0.26, 0.60])
    models = [(OPUS, a_o), (CODEX, a_c)]
    for i, ((name, harness, color), a) in enumerate(models):
        axL.bar(i, a["p1"], width=0.62, color=color, zorder=3)
        axL.errorbar(i, a["p1"], yerr=a["se"], color="#0b0b0b", capsize=5, lw=1.4, zorder=4)
        axL.text(i, a["p1"] + a["se"] + 0.035, f"{a['p1']*100:.1f}%", ha="center",
                 family=CHAKRA_SEMI, fontsize=15, color="#0b0b0b")
        axL.text(i, -0.075, name, ha="center", family=CHAKRA_MED, fontsize=10.5, color="#0b0b0b")
        axL.text(i, -0.135, f"via {harness}", ha="center", family=CHAKRA, fontsize=8.5, color="#666666")
        axL.text(i, -0.19, f"n={a['n_runs']} runs / {a['n_tasks']} tasks", ha="center",
                 family=CHAKRA, fontsize=8.5, color="#666666")
    axL.set_ylim(0, 1.12)
    axL.set_xlim(-0.6, 1.6)
    axL.set_xticks([])
    axL.set_yticks([0, 0.25, 0.5, 0.75, 1.0])
    axL.set_yticklabels(["0%", "25%", "50%", "75%", "100%"], family=CHAKRA, fontsize=9)
    axL.spines[["top", "right"]].set_visible(False)
    axL.set_title("pass@1 (functional == 1.0)", family=CHAKRA_MED, fontsize=11.5, pad=12)

    # ── right panel: the tasks that separate the models ─────────
    interesting = [t for t in tasks
                   if (data[t][C] and min(1.0, *(data[t][C])) < 1.0)
                   or (data[t][O] and min(1.0, *(data[t][O])) < 1.0)]
    interesting.sort(key=lambda t: (a_o["rates"].get(t, 1) + a_c["rates"].get(t, 1)))
    axR = fig.add_axes([0.46, 0.235, 0.50, 0.60])
    for j, t in enumerate(interesting):
        y = len(interesting) - 1 - j
        rc, ro = a_c["rates"].get(t), a_o["rates"].get(t)
        if rc is not None and ro is not None:
            axR.plot([rc, ro], [y, y], color="#cccccc", lw=2.5, zorder=2)
        if rc is not None:
            axR.scatter(rc, y, s=95, color=CODEX[2], zorder=3)
        if ro is not None:
            axR.scatter(ro, y, s=95, color=OPUS[2], zorder=3)
        label = t.replace("oss-", "")
        n = max(len(data[t][C]), len(data[t][O]))
        axR.text(-0.04, y, f"{label}  (n={n}×2)", ha="right", va="center",
                 family=CHAKRA, fontsize=9, color="#0b0b0b")
    axR.set_xlim(-0.05, 1.08)
    axR.set_ylim(-0.7, len(interesting) - 0.3)
    axR.set_yticks([])
    axR.set_xticks([0, 1/3, 2/3, 1.0])
    axR.set_xticklabels(["0/3", "1/3", "2/3", "3/3"], family=CHAKRA, fontsize=9)
    axR.spines[["top", "right", "left"]].set_visible(False)
    axR.set_title("per-task solve rate — every task either model missed", family=CHAKRA_MED,
                  fontsize=11.5, pad=12)
    axR.scatter([], [], s=95, color=OPUS[2], label="Claude Opus 5")
    axR.scatter([], [], s=95, color=CODEX[2], label="GPT 5.6 Sol")
    axR.legend(loc="upper right", frameon=False, prop=fm.FontProperties(family=CHAKRA, size=9))

    fig.text(0.06, 0.075,
             "37 of 38 tasks (oss-pydantic-none-discriminator needs a per-task venv the host runner lacks; Docker/API only). "
             "Saturated tasks n=1 per model; every task either model ever missed was re-measured at n=3.",
             family=CHAKRA, fontsize=8, color="#666666")
    fig.text(0.06, 0.045,
             "Both models billed via subscriptions (Codex CLI / Claude Code CLI agent harnesses — scores are model+harness, not raw API). "
             "All tasks from upstream PRs merged May–Aug 2026, after both models' training cutoffs. Whiskers: ±1 stderr across tasks.",
             family=CHAKRA, fontsize=8, color="#666666")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT, facecolor="white", bbox_inches="tight")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
