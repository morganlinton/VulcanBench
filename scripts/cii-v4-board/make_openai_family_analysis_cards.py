"""OpenAI's five models on VulcanBench Frontier v4: effort returns and Code quality by effort.

Reads the aggregate CSV and JSON that make_openai_family_cards.py writes (the
validated model by effort cells and their source records) and adds two cards
to that report. Nothing is re-judged or re-priced; the populations, protocols,
receipts and pricing are whatever the family generator already validated.

    python scripts/cii-v4-board/make_openai_family_analysis_cards.py
    python scripts/cii-v4-board/make_openai_family_analysis_cards.py --refresh

The second form runs make_openai_family_cards.py first to refresh the family
aggregates, which needs the private run directories. Writes
docs/results/swe-v4-openai-family-2026-09/openai-family-effort-returns.png and
openai-family-quality-vs-effort.png, each with an SVG, a CSV of the plotted
rows and a JSON record of sources and hashes.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import itertools
import json
import math
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import make_openai_family_cards as family
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs/results/swe-v4-openai-family-2026-09"
BOARD = ROOT / "scripts/cii-v4-board"
STEPS = tuple(itertools.pairwise(family.LEVELS))


def level_name(effort: str) -> str:
    return effort.replace("-", " ").capitalize()


@dataclass(frozen=True)
class Cell:
    model: str
    effort: str
    n_score: int
    n_econ: int
    combined: float
    code_quality: float
    code_quality_se: float
    minutes: float
    passed: int
    usd: float


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def load_cells(score_csv: Path, economics_csv: Path) -> dict[tuple[str, str], Cell]:
    score_rows = {(row["model"], row["effort"]): row for row in read_rows(score_csv)}
    econ_rows = {(row["model"], row["effort"]): row for row in read_rows(economics_csv)}
    if score_rows.keys() != econ_rows.keys():
        missing = sorted(score_rows.keys() - econ_rows.keys())
        extra = sorted(econ_rows.keys() - score_rows.keys())
        raise ValueError(f"score/economics cell mismatch: missing={missing}, extra={extra}")

    cells: dict[tuple[str, str], Cell] = {}
    for key, score in score_rows.items():
        econ = econ_rows[key]
        cells[key] = Cell(
            model=score["model"],
            effort=score["effort"],
            n_score=int(score["n"]),
            n_econ=int(econ["n"]),
            combined=float(score["combined"]),
            code_quality=float(score["code_quality"]),
            code_quality_se=float(score["code_quality_se"]),
            minutes=float(score["minutes"]),
            passed=int(score["passed"]),
            usd=float(econ["usd"]),
        )
    validate_cells(cells)
    return cells


def validate_cells(cells: dict[tuple[str, str], Cell]) -> None:
    expected = {(model, effort) for model in family.MODELS for effort in family.LEVELS_FOR[model]}
    actual = set(cells)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise ValueError(f"unexpected model/effort cells: missing={missing}, extra={extra}")
    if cells["sol", "max"].n_score != 22 or cells["sol", "max"].n_econ != 23:
        raise ValueError("expected Sol Max n=22 judged and n=23 economics")


def levels_for(cells: dict[tuple[str, str], Cell], model: str) -> list[str]:
    return [effort for effort in family.LEVELS if (model, effort) in cells]


def best_effort(cells: dict[tuple[str, str], Cell], model: str) -> str:
    return max(levels_for(cells, model), key=lambda effort: cells[model, effort].combined)


def transitions(cells: dict[tuple[str, str], Cell], model: str) -> list[dict[str, Any]]:
    levels = levels_for(cells, model)
    result: list[dict[str, Any]] = []
    for before_effort, after_effort in itertools.pairwise(levels):
        before = cells[model, before_effort]
        after = cells[model, after_effort]
        result.append(
            {
                "model": model,
                "from": before_effort,
                "to": after_effort,
                "score_delta": after.combined - before.combined,
                "quality_delta": after.code_quality - before.code_quality,
                "passed_delta": after.passed - before.passed,
                "cost_delta": after.usd - before.usd,
                "runtime_delta": after.minutes - before.minutes,
            }
        )
    return result


def save_card(fig: plt.Figure, output: Path) -> None:
    fig.savefig(output, facecolor=family.PAPER)
    fig.savefig(output.with_suffix(".svg"), facecolor=family.PAPER)
    plt.close(fig)


def source_provenance(
    score_csv: Path,
    economics_csv: Path,
    score_json: Path,
    economics_json: Path,
) -> dict[str, Any]:
    score_record = json.loads(score_json.read_text(encoding="utf-8"))
    economics_record = json.loads(economics_json.read_text(encoding="utf-8"))
    return {
        "score_csv": {"file": score_csv.name, "sha256": sha256(score_csv)},
        "economics_csv": {"file": economics_csv.name, "sha256": sha256(economics_csv)},
        "score_report": {
            "file": score_json.name,
            "sha256": sha256(score_json),
            "sources": score_record.get("sources"),
        },
        "economics_report": {
            "file": economics_json.name,
            "sha256": sha256(economics_json),
            "sources": economics_record.get("sources"),
            "pricing_verified": economics_record.get("pricing_verified"),
        },
    }


def write_record(
    stem: str,
    output_dir: Path,
    csv_path: Path,
    provenance: dict[str, Any],
    metadata: dict[str, Any],
) -> None:
    record = {
        "card": stem,
        "provenance": provenance,
        "supporting_table": csv_path.name,
        "png_sha256": sha256(output_dir / f"{stem}.png"),
        "csv_sha256": sha256(csv_path),
        **metadata,
    }
    (output_dir / f"{stem}.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


def delta_bars(ax, values: dict[tuple[str, int], float], label_fmt) -> tuple[float, float]:
    """Grouped bars of a per-step change, one bar per model, in the family bar style."""
    low, high = 0.0, 0.0
    for model in family.MODELS:
        xs = [i for i in range(len(STEPS)) if (model, i) in values]
        ys = [values[model, i] for i in xs]
        low, high = min(low, *ys), max(high, *ys)
        ax.bar(
            [i + family.BAR_SHIFTS[model] for i in xs],
            ys,
            width=0.15,
            color=family.COLORS[model],
            edgecolor=family.INK,
            linewidth=0.4,
            zorder=2,
        )
        for i, y in zip(xs, ys, strict=True):
            ax.annotate(
                label_fmt(y),
                (i + family.BAR_SHIFTS[model], y),
                xytext=(0, 3 if y >= 0 else -3),
                textcoords="offset points",
                ha="center",
                va="bottom" if y >= 0 else "top",
                fontsize=7.5,
                fontfamily="IBM Plex Mono",
                color=family.INK,
                rotation=90,
            )
    ax.axhline(0, color=family.INK, linewidth=0.8, zorder=1)
    ax.set_xlim(-0.6, len(STEPS) - 0.4)
    ax.set_xticks(range(len(STEPS)), [f"{level_name(a)} to\n{level_name(b)}" for a, b in STEPS])
    return low, high


def delta_limits(low: float, high: float) -> tuple[float, float, float]:
    """Axis bottom, top and tick step with headroom for the rotated bar labels."""
    span = max(high, -low, 1e-9)
    step = next(s for s in (0.1, 0.2, 0.5, 1, 2, 5, 10, 20) if span / s <= 7)
    top = step * math.ceil(high / step) + step * 1.2 if high > 0 else step * 0.4
    bottom = step * math.floor(low / step) - step * 0.8 if low < 0 else 0.0
    return bottom, top, step


def effort_returns_card(
    cells: dict[tuple[str, str], Cell],
    output_dir: Path,
    provenance: dict[str, Any],
) -> str:
    fig = family.new_figure()
    text, line = family.make_text(fig)
    family.masthead(
        fig,
        text,
        line,
        "VulcanBench Frontier v4: OpenAI's five models across effort levels",
        "Change in combined score, API-equivalent cost and runtime at each step up in effort; "
        "the same runs as the score and economics cards.",
    )
    rows = [row for model in family.MODELS for row in transitions(cells, model)]
    by_step = {(r["model"], STEPS.index((r["from"], r["to"]))): r for r in rows}

    chart_top, chart_h = 3.75, 2.7
    for panel, (x0, w) in (("score", (0.085, 0.405)), ("cost", (0.565, 0.39))):
        tx = family.LEFT if panel == "score" else x0 - 0.04
        text(
            tx,
            3.1,
            "Change in combined score" if panel == "score" else "Change in API-equivalent cost",
            21,
            True,
            heading=True,
        )
        text(
            tx,
            3.42,
            "Points  ·  higher is better  ·  one bar per step up in effort"
            if panel == "score"
            else "USD per task at list prices  ·  lower is better",
            12,
            color=family.MUTED,
        )
        ax = fig.add_axes(
            [x0, family.yf(chart_top + chart_h), w, chart_h / family.HEIGHT_IN],
            facecolor=family.PAPER,
        )
        family.style_axis(ax)
        key = "score_delta" if panel == "score" else "cost_delta"
        fmt = (lambda v: f"{v:+.1f}") if panel == "score" else (lambda v: f"{v:+.2f}")
        low, high = delta_bars(ax, {k: r[key] for k, r in by_step.items()}, fmt)
        bottom, top, step = delta_limits(low, high)
        ax.set_ylim(bottom, top)
        ticks = [t * step for t in range(math.ceil(bottom / step), math.floor(top / step) + 1)]
        ax.set_yticks(ticks, [f"{t:g}" for t in ticks])

    # Table: runtime change at every step, the third quantity the panels do not carry
    t0 = 7.2
    text(
        family.LEFT,
        t0,
        "Table 1  |  Change in minutes per task at each step up in effort",
        14.5,
        False,
        heading=True,
    )
    cols = (0.52, 0.66, 0.80, 0.94)
    line(family.LEFT, family.RIGHT, t0 + 0.28, family.INK, 1.2)
    text(family.LEFT, t0 + 0.52, "Model", 12.5, True)
    for x, (a, b) in zip(cols, STEPS, strict=True):
        text(x, t0 + 0.52, f"{level_name(a)} to {level_name(b)}", 12.5, True, ha="right")
    text(cols[-1], t0 + 0.78, "minutes per task", 10, ha="right", color=family.MUTED)
    line(family.LEFT, family.RIGHT, t0 + 0.95, family.INK, 0.6)
    step, y = 0.32, t0 + 1.22
    for model in family.MODELS:
        text(family.LEFT, y, family.NAMES[model], 12.5, True)
        for i, x in enumerate(cols):
            present = (model, i) in by_step
            text(
                x,
                y,
                f"{by_step[model, i]['runtime_delta']:+.1f}" if present else "no level",
                13.5,
                numeric=True,
                ha="right",
                color=family.INK if present else family.MUTED,
            )
        y += step
    line(family.LEFT, family.RIGHT, y - step / 2, family.INK, 1.2)
    family.footnotes(
        text,
        y - step / 2 + 0.18,
        [
            "Changes are differences of cell means; each cell's standard error is on the score and "
            "economics cards. Score and cost changes are labeled on the bars.",
            "GPT-5.5 offers four effort levels (no max). Sol at max is judged on 22 of 23 runs; its "
            "economics population is 23 priced runs. Costs are API-equivalent list-price solver estimates.",
        ],
    )

    stem = "openai-family-effort-returns"
    png = output_dir / f"{stem}.png"
    save_card(fig, png)
    csv_path = output_dir / f"{stem}.csv"
    with csv_path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0].keys()), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    write_record(
        stem,
        output_dir,
        csv_path,
        provenance,
        {
            "panels": ["score_delta", "cost_delta"],
            "table": "runtime_delta",
            "transition_rows": len(rows),
        },
    )
    return stem


def quality_vs_effort_card(  # noqa: PLR0915, one linear figure
    cells: dict[tuple[str, str], Cell],
    output_dir: Path,
    provenance: dict[str, Any],
) -> str:
    fig = family.new_figure()
    text, line = family.make_text(fig)
    family.masthead(
        fig,
        text,
        line,
        "VulcanBench Frontier v4: OpenAI's five models across effort levels",
        "Code quality and full task passes at every effort level each model offers; "
        "the same runs as the score card.",
    )
    chart_top, chart_h = 3.75, 2.85
    axes = {}
    for panel, (x0, w) in (("quality", (0.085, 0.405)), ("passed", (0.565, 0.39))):
        tx = family.LEFT if panel == "quality" else x0 - 0.04
        text(
            tx,
            3.1,
            "Code quality" if panel == "quality" else "Full tasks passed",
            21,
            True,
            heading=True,
        )
        text(
            tx,
            3.42,
            "/100  ·  higher is better  ·  axis 55 to 80"
            if panel == "quality"
            else "Out of 23 tasks  ·  higher is better",
            12,
            color=family.MUTED,
        )
        ax = fig.add_axes(
            [x0, family.yf(chart_top + chart_h), w, chart_h / family.HEIGHT_IN],
            facecolor=family.PAPER,
        )
        family.style_axis(ax)
        ax.set_xlim(-0.45, len(family.LEVELS) - 0.55)
        axes[panel] = ax
    axes["quality"].set_ylim(55, 80)
    axes["quality"].set_yticks(range(55, 81, 5))
    axes["passed"].set_ylim(0, 25)
    axes["passed"].set_yticks(range(0, 26, 5))

    for model in family.MODELS:
        levels = levels_for(cells, model)
        xs = [family.LEVELS.index(effort) for effort in levels]
        quality = [cells[model, effort].code_quality for effort in levels]
        quality_se = [cells[model, effort].code_quality_se for effort in levels]
        passed = [cells[model, effort].passed for effort in levels]
        axes["quality"].plot(xs, quality, color=family.COLORS[model], linewidth=2, zorder=2)
        axes["quality"].errorbar(
            xs,
            quality,
            yerr=quality_se,
            fmt=family.MARKERS[model],
            color=family.COLORS[model],
            markersize=7.5,
            markeredgecolor=family.INK,
            markeredgewidth=0.6,
            ecolor=family.INK,
            elinewidth=0.8,
            capsize=2.5,
            zorder=3,
        )
        axes["passed"].plot(
            xs,
            passed,
            color=family.COLORS[model],
            marker=family.MARKERS[model],
            markersize=7.5,
            linewidth=2,
            markeredgecolor=family.INK,
            markeredgewidth=0.6,
            zorder=3,
        )

    # Table: each model from its Low cell to its best cell
    t0 = 7.2
    text(
        family.LEFT,
        t0,
        "Table 1  |  Change from Low to each model's best effort level",
        14.5,
        False,
        heading=True,
    )
    cols = (0.38, 0.53, 0.67, 0.81, family.RIGHT)
    headers = ("Best effort", "Code quality", "Tasks passed", "Combined", "Best combined")
    line(family.LEFT, family.RIGHT, t0 + 0.28, family.INK, 1.2)
    text(family.LEFT, t0 + 0.52, "Model", 12.5, True)
    for x, header in zip(cols, headers, strict=True):
        text(x, t0 + 0.52, header, 12.5, True, ha="right")
    for x in cols[1:4]:
        text(x, t0 + 0.78, "change", 10, ha="right", color=family.MUTED)
    text(cols[4], t0 + 0.78, "/100", 10, ha="right", color=family.MUTED)
    line(family.LEFT, family.RIGHT, t0 + 0.95, family.INK, 0.6)

    summary_rows: list[dict[str, Any]] = []
    step, y = 0.32, t0 + 1.22
    for model in family.MODELS:
        levels = levels_for(cells, model)
        low = cells[model, levels[0]]
        effort = best_effort(cells, model)
        best = cells[model, effort]
        row = {
            "model": model,
            "best_effort": effort,
            "low_code_quality": low.code_quality,
            "best_code_quality": best.code_quality,
            "quality_delta": best.code_quality - low.code_quality,
            "low_passed": low.passed,
            "best_passed": best.passed,
            "passed_delta": best.passed - low.passed,
            "low_combined": low.combined,
            "best_combined": best.combined,
            "combined_delta": best.combined - low.combined,
        }
        summary_rows.append(row)
        text(family.LEFT, y, family.NAMES[model], 12.5, True)
        text(cols[0], y, level_name(effort), 12, ha="right")
        text(cols[1], y, f"{row['quality_delta']:+.1f}", 13.5, numeric=True, ha="right")
        text(cols[2], y, f"{row['passed_delta']:+d}", 13.5, numeric=True, ha="right")
        text(cols[3], y, f"{row['combined_delta']:+.1f}", 13.5, numeric=True, ha="right")
        text(cols[4], y, f"{row['best_combined']:.1f}", 13.5, numeric=True, ha="right")
        y += step
    line(family.LEFT, family.RIGHT, y - step / 2, family.INK, 1.2)
    family.footnotes(
        text,
        y - step / 2 + 0.18,
        [
            "Whiskers are one task standard error. Code quality uses the published Muse Spark 1.3 and "
            "Grok 4.6 review protocol; the axis is 55 to 80, not 0 to 100.",
            "Best effort means the highest observed combined score in this sweep. GPT-5.5 offers four "
            "effort levels (no max). Sol at max is judged on 22 of 23 runs.",
        ],
    )

    stem = "openai-family-quality-vs-effort"
    png = output_dir / f"{stem}.png"
    save_card(fig, png)
    csv_path = output_dir / f"{stem}.csv"
    with csv_path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle, fieldnames=list(summary_rows[0].keys()), lineterminator="\n"
        )
        writer.writeheader()
        writer.writerows(summary_rows)
    write_record(
        stem,
        output_dir,
        csv_path,
        provenance,
        {"code_quality_axis": [55, 80], "code_quality_error_bars": True},
    )
    return stem


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--refresh",
        action="store_true",
        help=(
            "Run make_openai_family_cards.py first to refresh the family aggregates "
            "from the private run directories."
        ),
    )
    args = parser.parse_args()
    if args.refresh:
        subprocess.run(
            [sys.executable, str(BOARD / "make_openai_family_cards.py")], cwd=ROOT, check=True
        )
    score_csv = OUTPUT / "openai-family-efforts.csv"
    economics_csv = OUTPUT / "openai-family-economics-efforts.csv"
    score_json = OUTPUT / "openai-family.json"
    economics_json = OUTPUT / "openai-family-economics.json"
    cells = load_cells(score_csv, economics_csv)
    provenance = source_provenance(score_csv, economics_csv, score_json, economics_json)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    effort_returns_card(cells, OUTPUT, provenance)
    quality_vs_effort_card(cells, OUTPUT, provenance)


if __name__ == "__main__":
    main()
