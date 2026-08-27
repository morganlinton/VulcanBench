"""Shared config + data loading for the suite-v3 result cards.

``make_chart.py`` (the four-panel rankings card) predates this module and keeps
its own copy deliberately, it is the published card, and refactoring it is a
separate change from prototyping alternatives. New cards import from here.
"""

import json
from pathlib import Path

import matplotlib
from matplotlib import font_manager

HERE = Path(__file__).resolve().parent


def register_fonts() -> None:
    """Load the brand faces (see CLAUDE.md) into matplotlib's font manager."""
    for w in (400, 500, 600, 700):
        font_manager.fontManager.addfont(str(HERE / f"geist-{w}.ttf"))
    for w in (400, 500, 600):
        font_manager.fontManager.addfont(str(HERE / f"chakra-{w}.ttf"))
    matplotlib.rcParams["font.family"] = "Geist"


SANS = "Geist"
BRAND = "Chakra Petch SemiBold"  # vulcanbench.com wordmark face
BRAND_MED = "Chakra Petch Medium"

SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK2 = "#52514e"
MUTED = "#8a897f"
GRID = "#e7e6e1"

# Per-lab hues, matching the rankings card (see CLAUDE.md). Note for future
# edits: this set FAILS the generic categorical checks, xAI black and Moonshot
# slate are below the chroma floor, and OpenAI green vs Anthropic clay is
# dE 6.8 under protanopia. That is legal only because every mark is directly
# labelled with its model name; do not drop those labels to "clean up" a card.
LAB_COLOR = {
    "Anthropic": "#D97757",
    "OpenAI": "#10A37F",
    "xAI": "#0A0A0A",
    "Moonshot": "#44445E",
    "DeepSeek": "#5786FE",
    "Alibaba": "#9333EA",
    # Z.ai has no official hex in our set; deep teal clears every neighbour
    # by dE > 15 and, like the rest, every mark stays directly labelled.
    "Z.ai": "#0e7a8a",
    # Meta brand blue; clears DeepSeek's lighter blue at chart sizes because the
    # marks stay directly labelled (see note above).
    "Meta": "#0866ff",
}

NAME = {
    "xai:grok-4.6": ("Grok 4.6", "xAI"),
    "openai:grok-4.5": ("Grok 4.5", "xAI"),
    "openai:gpt-5.6-luna": ("GPT-5.6 Luna", "OpenAI"),
    "openai:gpt-5.6-sol": ("GPT-5.6 Sol", "OpenAI"),
    "openai:gpt-5.6-terra": ("GPT-5.6 Terra", "OpenAI"),
    "anthropic:claude-fable-5": ("Claude Fable 5", "Anthropic"),
    "anthropic:claude-haiku-4-5": ("Claude Haiku 4.5", "Anthropic"),
    "anthropic:claude-opus-5": ("Claude Opus 5", "Anthropic"),
    "deepseek:deepseek-v4-flash": ("DeepSeek V4-Flash", "DeepSeek"),
    "deepseek:deepseek-v4-pro": ("DeepSeek V4 Pro", "DeepSeek"),
    "kimi:kimi-k3": ("Kimi K3", "Moonshot"),
    "qwen:qwen3.8-max": ("Qwen3.8-Max", "Alibaba"),
    "qwen:qwen3.8-27b": ("Qwen3.8-27B", "Alibaba"),
    "zai:glm-5.3": ("GLM 5.3", "Z.ai"),
    "meta:muse-spark-1.2": ("Muse Spark 1.2", "Meta"),
}

EXCLUDED = {
    "anthropic:claude-opus-4-8",  # 5/23 task coverage
    "zcode:glm-5.3",  # subscription harness: model plus product, never a board column
    "pi:meta:muse-spark-1.2",  # Pi harness: model plus agent, never a board column
    "ollama:qwen3.8:27b",  # local-inference control runs, not the DashScope column
}
# Externally sourced (vulcanbench.com Report 10), not run in this checkout.
EXTERNAL = {"anthropic:claude-opus-5"}
N_TASKS_FULL = 23


def eff_display(model: str, eff: str) -> str:
    """Label an effort with the provider's own name for it."""
    if eff in ("", "\u2014", "-"):  # stored as a dash when no effort was set
        return "default"
    if eff == "extra-high":
        if model.startswith(("deepseek:", "zai:")):
            return "max"
        if model.startswith(("qwen:", "xai:", "meta:")):
            return "xhigh"
    return eff


def model_efforts(model: str) -> list[str]:
    """The provider's own effort ladder for ``model``, low to high.

    These are not a shared scale: each API documents its own enum, so a card
    comparing them compares each model at its own settings.
    """
    if model.startswith("deepseek:"):
        return ["low", "high", "extra-high"]  # DeepSeek: low/high/max
    if model.startswith("qwen:"):
        return ["low", "medium", "extra-high"]  # Qwen: low/medium/xhigh
    if model.startswith("zai:"):
        return ["low", "high", "extra-high"]  # GLM 5.3: low/high/max
    if model.startswith("meta:"):
        return ["low", "high", "extra-high"]  # Muse Spark: low/high/xhigh (minimal/medium untested)
    if model.startswith("xai:"):
        return ["low", "medium", "high", "extra-high"]  # Grok 4.6+: adds xhigh
    return ["low", "medium", "high"]


def load_rows() -> list[dict]:
    """Every (model, effort) column, minus deliberate exclusions."""
    with open(HERE / "v3_rankings.json") as f:
        rows = [r for r in json.load(f) if r["model"] not in EXCLUDED]
    unknown = {r["model"] for r in rows} - set(NAME)
    if unknown:
        raise SystemExit(f"models missing from NAME (add or exclude explicitly): {unknown}")
    return rows


def best_per_model(rows: list[dict]) -> list[dict]:
    """One point per model at its best-scoring effort.

    Ties break to the cheaper run, so a model is never flattered by an
    expensive coin-flip.
    """
    best: dict[str, dict] = {}
    for r in rows:
        cur = best.get(r["model"])
        cand = (r["pass1"], -r["cost"] / max(r["n_runs"], 1))
        if cur is None or cand > (cur["pass1"], -cur["cost"] / max(cur["n_runs"], 1)):
            best[r["model"]] = r

    pts = []
    for model, r in best.items():
        disp, lab = NAME[model]
        pts.append(
            dict(
                model=model,
                label=disp,
                lab=lab,
                effort=eff_display(model, r["effort"]),
                cost_per_run=r["cost"] / max(r["n_runs"], 1),
                pass1=r["pass1"] * 100,
                se=(r["se"] or 0) * 100,
                n_runs=r["n_runs"],
                n_tasks=r["n_tasks"],
                partial=r["n_tasks"] < N_TASKS_FULL,
                external=model in EXTERNAL,
            )
        )
    return sorted(pts, key=lambda p: -p["pass1"])


def top_effort_row(model_rows: list[dict]) -> dict:
    """The row at the highest rung of the provider's effort ladder that has data.

    Models without a sweep (a single 'default' or 'extra-high' column) keep
    their only row.
    """
    ladder = model_efforts(model_rows[0]["model"])
    rank = {e: i for i, e in enumerate(ladder)}
    return max(model_rows, key=lambda r: (rank.get(r["effort"], -1), r["pass1"]))


def best_effort_row(model_rows: list[dict]) -> dict:
    """The row with the highest pass@1; ties break to the cheaper run.

    Full-coverage columns are preferred: a partial column (fewer than
    N_TASKS_FULL tasks) can only win when the model has no full column at all,
    so a 7-task sweep never outranks a 23-task one on a fraction of a point.
    """
    full = [r for r in model_rows if r["n_tasks"] >= N_TASKS_FULL]
    pool = full or model_rows
    return max(pool, key=lambda r: (r["pass1"], -r["cost"] / max(r["n_runs"], 1)))


def top_per_model(rows: list[dict], rule: str = "best") -> list[dict]:
    """One point per model.

    ``rule="best"`` picks the best-scoring effort level (the leaderboard
    default); ``rule="max"`` picks the highest rung of the provider's effort
    ladder. Same record shape as ``best_per_model`` plus speed/cost fields.
    """
    pick = {"best": best_effort_row, "max": top_effort_row}[rule]
    per_model: dict[str, list[dict]] = {}
    for r in rows:
        per_model.setdefault(r["model"], []).append(r)
    pts = []
    for model, mrows in per_model.items():
        r = pick(mrows)
        disp, lab = NAME[model]
        pts.append(
            dict(
                model=model,
                label=disp,
                lab=lab,
                effort=eff_display(model, r["effort"]),
                cost_per_run=r["cost"] / max(r["n_runs"], 1),
                cost_total=r["cost"],
                pass1=r["pass1"] * 100,
                se=(r["se"] or 0) * 100,
                minutes=(r["avg_duration_s"] or 0) / 60,
                n_runs=r["n_runs"],
                n_tasks=r["n_tasks"],
                partial=r["n_tasks"] < N_TASKS_FULL,
                external=model in EXTERNAL,
            )
        )
    return sorted(pts, key=lambda p: (-p["pass1"], p["cost_per_run"]))
