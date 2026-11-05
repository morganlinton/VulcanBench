#!/usr/bin/env python3
"""Render the router-eval cards from ``analysis.json`` in VulcanBench styling.

    python scripts/router-eval/make_router_cards.py analysis.json --out-dir docs/results/router-v1-2026-11

Two PNGs:

* ``router_routing_card.png``: served-model share per cell for every router
  (ladder) and every agent leg that reported served models, with n per column.
* ``router_cost_card.png``: cost per task per leg with n and stderr whiskers,
  and the monthly cost-per-engineer curves with their crossovers.

Chart-integrity rules carried over from ``scripts/rankings-chart``: n stays
visible on every column, whiskers are one standard error, subscription legs
are drawn as API-equivalent and labelled so, and the list-price caveat is
footnoted. No em-dashes in any rendered text.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
CHART_DIR = ROOT / "scripts" / "rankings-chart"
sys.path.insert(0, str(CHART_DIR))

SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK2 = "#52514e"
MUTED = "#8a897f"
GRID = "#e7e6e1"
LAB_COLOR = {
    "anthropic": "#D97757",
    "openai": "#10A37F",
    "deepseek": "#5786FE",
    "x-ai": "#0A0A0A",
    "xai": "#0A0A0A",
    "moonshotai": "#44445E",
    "moonshot": "#44445E",
    "qwen": "#9333EA",
    "alibaba": "#9333EA",
    "z-ai": "#0e7a8a",
    "zai": "#0e7a8a",
    "meta": "#0866ff",
    "meta-llama": "#0866ff",
}
GREYS = ["#8a897f", "#b3b2a9", "#6b6a63", "#cfceC6", "#4a4945"]
BRAND = "Chakra Petch SemiBold"
BRAND_MED = "Chakra Petch Medium"
MONO = "IBM Plex Mono"
SANS = "Geist"
CELL_ORDER = [
    "python/small/easy",
    "python/small/hard",
    "python/large/easy",
    "python/large/hard",
    "rust/small/easy",
    "rust/small/hard",
    "rust/large/easy",
    "rust/large/hard",
]


def register_fonts() -> None:
    for name in (
        "geist-400",
        "geist-500",
        "geist-600",
        "chakra-500",
        "chakra-600",
        "ibm-plex-mono-400",
        "ibm-plex-mono-500",
    ):
        path = CHART_DIR / f"{name}.ttf"
        if path.exists():
            font_manager.fontManager.addfont(str(path))
    matplotlib.rcParams["font.family"] = SANS
    matplotlib.rcParams["axes.unicode_minus"] = False


def model_color(model: str, fallback_index: int) -> str:
    vendor = model.split("/", maxsplit=1)[0].lower() if "/" in model else model.lower()
    for key, color in LAB_COLOR.items():
        if vendor.startswith(key) or model.lower().startswith(key):
            return color
    if "claude" in model.lower():
        return LAB_COLOR["anthropic"]
    if "gpt" in model.lower() or model.lower().startswith("o"):
        return LAB_COLOR["openai"]
    return GREYS[fallback_index % len(GREYS)]


def short_model(model: str) -> str:
    tail = model.rsplit("/", maxsplit=1)[-1]
    return tail if len(tail) <= 26 else tail[:24] + ".."


def _header(fig: plt.Figure, title: str, subtitle: str) -> None:
    logo = CHART_DIR / "vb_logo_rounded.png"
    if logo.exists():
        img = plt.imread(str(logo))
        ax = fig.add_axes([0.03, 0.915, 0.05, 0.065])
        ax.imshow(img)
        ax.axis("off")
    fig.text(0.09, 0.955, "VulcanBench", fontsize=26, fontname=BRAND, color=INK, va="center")
    fig.text(0.09, 0.925, title, fontsize=15, fontname=BRAND_MED, color=INK2, va="center")
    fig.text(0.97, 0.94, subtitle, fontsize=10, fontname=MONO, color=MUTED, ha="right", va="center")


def _footer(fig: plt.Figure, text: str) -> None:
    fig.text(0.03, 0.02, text, fontsize=8.5, fontname=MONO, color=MUTED, va="bottom", wrap=True)


def routing_panels(analysis: dict[str, Any]) -> list[tuple[str, dict[str, Any]]]:
    panels: list[tuple[str, dict[str, Any]]] = []
    for router, data in analysis["routing"]["ladder"].items():
        if data.get("by_cell"):
            panels.append((f"{router} (single request)", data))
    for leg, data in analysis["routing"]["runs"].items():
        if data.get("by_cell"):
            panels.append((f"{leg} (agent runs)", data))
    return panels


def make_routing_card(analysis: dict[str, Any], out: Path) -> None:
    panels = routing_panels(analysis)
    if not panels:
        print("routing card skipped: no served-model data")
        return
    rows = len(panels)
    fig = plt.figure(figsize=(14, 3.1 * rows + 2.2), facecolor=SURFACE)
    _header(fig, "Router v1: which model answered, by task cell", "served-model share per cell")
    top = 0.88
    height = 0.78 / rows
    for i, (label, data) in enumerate(panels):
        ax = fig.add_axes([0.08, top - (i + 1) * height + 0.06, 0.86, height - 0.11])
        ax.set_facecolor(SURFACE)
        cells = [c for c in CELL_ORDER if c in data["by_cell"]] + sorted(
            c for c in data["by_cell"] if c not in CELL_ORDER
        )
        models = sorted(data["served_models"], key=lambda m: -data["served_models"][m])
        bottoms = [0.0] * len(cells)
        for j, model in enumerate(models):
            vals = [data["by_cell"][c]["share"].get(model, 0.0) for c in cells]
            ax.bar(
                range(len(cells)),
                vals,
                bottom=bottoms,
                color=model_color(model, j),
                width=0.72,
                edgecolor=SURFACE,
                linewidth=0.8,
                label=short_model(model),
            )
            bottoms = [b + v for b, v in zip(bottoms, vals, strict=True)]
        for k, c in enumerate(cells):
            ax.text(
                k,
                1.03,
                f"n={data['by_cell'][c]['n']}",
                ha="center",
                va="bottom",
                fontsize=8,
                fontname=MONO,
                color=MUTED,
            )
        ax.set_xticks(range(len(cells)))
        ax.set_xticklabels([c.replace("/", "\n") for c in cells], fontsize=8.5, color=INK2)
        ax.set_ylim(0, 1.18)
        ax.set_yticks([0, 0.5, 1.0])
        ax.set_yticklabels(["0", "50%", "100%"], fontsize=8, fontname=MONO, color=MUTED)
        for spine in ("top", "right"):
            ax.spines[spine].set_visible(False)
        ax.spines["left"].set_color(GRID)
        ax.spines["bottom"].set_color(GRID)
        ax.grid(axis="y", color=GRID, linewidth=0.6)
        ax.set_axisbelow(True)
        test = data.get("cell_test") or {}
        lc = (data.get("length_control") or {}).get("test") or {}
        stat = f"cell test: chi2 p={test.get('p_value', float('nan')):.3g}, NMI={test.get('nmi', 0):.2f}"
        if lc:
            stat += f"   length control: p={lc.get('p_value', float('nan')):.3g}"
        if data.get("within_run_switch_rate") is not None:
            stat += f"   within-run switch rate={data['within_run_switch_rate']:.2f}"
        ax.set_title(
            f"{label}    {stat}", loc="left", fontsize=10.5, fontname=BRAND_MED, color=INK, pad=14
        )
        ax.legend(
            loc="upper right",
            fontsize=7.5,
            frameon=False,
            ncol=min(4, max(1, len(models))),
            bbox_to_anchor=(1.0, 1.2),
        )
    _footer(
        fig,
        "Share of requests (ladder) or steps (agent runs) answered by each served model, per task cell. "
        "p from a chi-square test of cell vs served model; NMI is normalised mutual information. "
        "A router that does not route shows one colour in every column.",
    )
    fig.savefig(out, dpi=160, facecolor=SURFACE)
    plt.close(fig)
    print(f"wrote {out}")


def _style_axes(ax: Any) -> None:
    for spine in ("top", "right"):
        ax.spines[spine].set_visible(False)
    ax.spines["left"].set_color(GRID)
    ax.spines["bottom"].set_color(GRID)
    ax.set_axisbelow(True)


def _break_even_panel(fig: Any, be: dict[str, Any]) -> None:
    ax2 = fig.add_axes([0.56, 0.17, 0.4, 0.64])
    ax2.set_facecolor(SURFACE)
    for i, (leg, curve) in enumerate((be.get("curves") or {}).items()):
        xs = [p["tasks"] for p in curve["points"]]
        ys = [p["usd"] for p in curve["points"]]
        style = "--" if curve["kind"] == "subscription" else "-"
        color = GREYS[1] if curve["kind"] == "subscription" else model_color(leg, i)
        ax2.plot(xs, ys, style, color=color, linewidth=2, label=f"{leg} ({curve['kind']})")
    for c in be.get("crossovers") or []:
        if c.get("tasks_per_month_where_seat_wins"):
            ax2.axvline(c["tasks_per_month_where_seat_wins"], color=GRID, linewidth=0.8)
    ax2.set_xscale("log")
    ax2.set_xlabel("tasks per engineer per month", fontsize=9, color=INK2)
    ax2.set_ylabel("USD per engineer per month", fontsize=9, color=INK2)
    _style_axes(ax2)
    ax2.grid(color=GRID, linewidth=0.6)
    ax2.legend(fontsize=8, frameon=False)
    ax2.set_title(
        "Monthly cost per engineer by demand",
        loc="left",
        fontsize=11,
        fontname=BRAND_MED,
        color=INK,
    )


def make_cost_card(analysis: dict[str, Any], out: Path) -> None:
    cost = analysis.get("cost") or {}
    if not cost:
        print("cost card skipped: no run data")
        return
    fig = plt.figure(figsize=(14, 7.6), facecolor=SURFACE)
    _header(
        fig, "Router v1: cost per task, and when a seat beats metering", "USD per task, n and 1 SE"
    )
    ax = fig.add_axes([0.07, 0.17, 0.4, 0.64])
    ax.set_facecolor(SURFACE)
    legs = list(cost)
    vals, errs, colors, labels = [], [], [], []
    for i, leg in enumerate(legs):
        stats = cost[leg]
        is_sub = stats["billing_mode"] == "subscription-included"
        block = (
            stats["api_equivalent"]
            if is_sub or stats["cash"]["mean_usd"] is None
            else stats["cash"]
        )
        vals.append(block["mean_usd"] or 0.0)
        errs.append(block["se_usd"] or 0.0)
        colors.append(GREYS[1] if is_sub else model_color(leg, i))
        suffix = " (API-equiv.)" if is_sub else ""
        labels.append(f"{leg}{suffix}\nn={block['n']}, pass {100 * (block['pass_rate'] or 0):.0f}%")
    ax.bar(
        range(len(legs)),
        vals,
        yerr=errs,
        color=colors,
        width=0.66,
        capsize=4,
        error_kw={"ecolor": INK2, "elinewidth": 1},
    )
    for i, v in enumerate(vals):
        ax.text(
            i,
            v + (errs[i] or 0) + max(vals) * 0.02,
            f"${v:.3f}",
            ha="center",
            va="bottom",
            fontsize=8.5,
            fontname=MONO,
            color=INK,
        )
    ax.set_xticks(range(len(legs)))
    ax.set_xticklabels(labels, fontsize=8, color=INK2)
    ax.set_ylabel("USD per task", fontsize=9, color=INK2)
    for spine in ("top", "right"):
        ax.spines[spine].set_visible(False)
    ax.spines["left"].set_color(GRID)
    ax.spines["bottom"].set_color(GRID)
    ax.grid(axis="y", color=GRID, linewidth=0.6)
    ax.set_axisbelow(True)
    ax.set_title("Mean cost per task", loc="left", fontsize=11, fontname=BRAND_MED, color=INK)

    be = analysis.get("break_even") or {}
    _break_even_panel(fig, be)
    cross = "; ".join(
        f"{c['metered']} beats a {c['subscription']} seat below {c['tasks_per_month_where_seat_wins']:.0f} tasks/month"
        for c in (be.get("crossovers") or [])
        if c.get("tasks_per_month_where_seat_wins")
    )
    _footer(
        fig,
        "Router legs: the gateway's own reported bill. API legs: list-price estimate from reported tokens; "
        "negotiated, cached and batch rates differ. Subscription legs: API-equivalent list cost, cash is the seat fee; "
        "seat capacity from usage-window deltas per task. " + cross,
    )
    fig.savefig(out, dpi=160, facecolor=SURFACE)
    plt.close(fig)
    print(f"wrote {out}")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("analysis", type=Path)
    ap.add_argument("--out-dir", type=Path, default=Path("."))
    args = ap.parse_args(argv)
    register_fonts()
    analysis = json.loads(args.analysis.read_text())
    args.out_dir.mkdir(parents=True, exist_ok=True)
    make_routing_card(analysis, args.out_dir / "router_routing_card.png")
    make_cost_card(analysis, args.out_dir / "router_cost_card.png")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
