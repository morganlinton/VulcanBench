"""The Frontier v4 leaderboard social card (1200 x 630) for vulcanbench.com/leaderboard.

One row per model at its best effort level, ranked by combined score, drawn as a dot
on a shared scale (dots, not bars, so a scale that starts above zero does not
exaggerate gaps). Colors are the board's per-model colors, so the card matches the
leaderboard table. Everything comes from the board data that
scripts/build_swe_v4_board.py embeds in the site's leaderboard.html (window.VB_V4):

    python scripts/leaderboard-card/make_leaderboard_card.py \
        --page ../VulcanBenchCOM/leaderboard.html \
        --out ../VulcanBenchCOM/assets/cards/leaderboard-frontier-v4.png
"""

from __future__ import annotations

import argparse
import json
import math
import re
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch

ROOT = Path(__file__).resolve().parents[2]
FONTS = ROOT / "scripts/rankings-chart"
LOGO = ROOT / "docs/assets/vulcanbench-logo.png"
PAPER, INK, RULE, MUTED = "#f7f5f0", "#0b0b0b", "#d9d6cc", "#6b6b66"
SHORT = {"low": "Low", "medium": "Medium", "high": "High", "extra-high": "XHigh", "max": "Max"}


def best_per_model(columns: list[dict]) -> list[dict]:
    """Each model's highest-scoring effort level, ranked by combined score."""
    best: dict[str, dict] = {}
    for c in columns:
        if c["model"] not in best or c["combined"] > best[c["model"]]["combined"]:
            best[c["model"]] = c
    return sorted(best.values(), key=lambda c: -c["combined"])


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--page", type=Path, required=True, help="the site's leaderboard.html")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    found = re.search(r"window\.VB_V4 = (\{.*?\});?\s*</script>", args.page.read_text(), re.S)
    if not found:
        raise SystemExit("no window.VB_V4 board data in the page")
    board = json.loads(found.group(1))
    rows = best_per_model(board["columns"])
    colors = board["colors"]

    for font in FONTS.glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    W, H = 12.0, 6.3  # inches at 100 dpi: 1200 x 630
    fig = plt.figure(figsize=(W, H), dpi=100, facecolor=PAPER)

    def text(x, y, s, size, family="Geist", weight=400, color=INK, ha="left"):
        fig.text(
            x / W,
            1 - y / H,
            s,
            fontsize=size,
            fontfamily=family,
            weight=weight,
            color=color,
            ha=ha,
            va="center",
        )

    left, right = 0.55, W - 0.55
    logo = fig.add_axes([left / W, 1 - 0.80 / H, 0.42 / W, 0.42 / H])
    mark = logo.imshow(plt.imread(LOGO))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    text(left + 0.55, 0.59, "VulcanBench", 17, "Chakra Petch SemiBold", 600)
    text(right, 0.59, "vulcanbench.com/leaderboard", 11.5, "IBM Plex Mono", color=MUTED, ha="right")
    text(left, 1.28, "Frontier v4 leaderboard", 26, "Chakra Petch SemiBold", 600)
    text(
        left,
        1.70,
        f"{len(rows)} models, each at its best effort level. Combined score, higher is better.",
        12.5,
        color=MUTED,
    )

    # Rows: rank, model, a dot on the shared scale, score and level.
    top_y, bottom_y = 2.12, H - 0.42
    step = (bottom_y - top_y) / len(rows)
    lo = math.floor(min(r["combined"] for r in rows) / 5) * 5
    hi = math.ceil(max(r["combined"] for r in rows) / 5) * 5
    x0, x1 = 3.35, right - 1.55  # the scale's span in inches

    def sx(v):
        return x0 + (v - lo) / (hi - lo) * (x1 - x0)

    ax = fig.add_axes([0, 0, 1, 1], facecolor="none")
    ax.set_xlim(0, W)
    ax.set_ylim(H, 0)
    ax.axis("off")
    for t in range(lo, hi + 1, 5):
        ax.plot(
            [sx(t), sx(t)], [top_y - 0.05, bottom_y - step + 0.18], color=RULE, lw=0.7, zorder=0
        )
        text(sx(t), bottom_y + 0.02, str(t), 9.5, "IBM Plex Mono", color=MUTED, ha="center")
    for i, r in enumerate(rows):
        y = top_y + (i + 0.5) * step - 0.08
        color = colors[r["key"]]
        text(left, y, f"{i + 1}", 11, "IBM Plex Mono", 500, MUTED)
        text(left + 0.38, y, r["model"], 12, "Geist", 600)
        ax.plot(
            [x0, sx(r["combined"])],
            [y, y],
            color=color,
            lw=1.4,
            alpha=0.35,
            zorder=1,
            solid_capstyle="round",
        )
        ax.scatter(
            [sx(r["combined"])], [y], s=58, color=color, edgecolor=INK, linewidth=0.6, zorder=3
        )
        text(sx(r["combined"]) + 0.16, y, f"{r['combined']:.1f}", 11, "IBM Plex Mono", 500)
        text(sx(r["combined"]) + 0.68, y, SHORT[r["effort"]], 10, "Geist", color=MUTED)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(args.out, facecolor=PAPER)
    plt.close(fig)
    print(args.out, len(rows), "models")


if __name__ == "__main__":
    main()
