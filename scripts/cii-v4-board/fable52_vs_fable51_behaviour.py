"""How Claude Fable 5.2 works differently from Claude Fable 5.1 on VulcanBench Frontier v4.

Fable 5.1's raw Frontier v4 streams did not survive the September host reset,
and the owner chose not to rerun it (chat, 2026-11-09), so the comparison uses
what both models left in the same form:

- the solver receipt of every run (usage from Claude Code's result events,
  duration, fallback), frozen in the Code quality population records (v3.4 for
  Fable 5.1; v3.30 and v3.30.1 for Fable 5.2);
- the candidate patch, extracted by the same evidence builder for both models
  (the `evidence/` directory of each round);
- for Fable 5.2 only, its raw streams: thinking summaries (recorded with
  `--thinking-display summarized`), thinking tokens, and the tool-call log.

Usage is summed over every result event in a Fable 5.2 stream: Claude Code
2.1.321 can emit a second result when a background task re-enters, and each
result's usage covers only its own invocation (the run summary kept the last
one; adapter fix pending). The population receipts of both models sum every
result the same way (harness/solver_receipts.py), and the Fable 5.2 stream
totals here are checked against them.

    python scripts/cii-v4-board/fable52_vs_fable51_behaviour.py \\
        --v34 <v3.4 dir> --v330 <v3.30 dir> --v3301 <v3.30.1 dir>

Writes fable52-vs-fable51-runs.csv (one row per run) and
fable52-vs-fable51-levels.csv (means by model and level) beside the card.
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import statistics
from datetime import datetime
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs/results/swe-v4-fable52-2026-11"
RUNS = ROOT / "runs-effort-fable52"
LEVELS = ("low", "medium", "high", "extra-high", "max")
EDIT_TOOLS = {"Edit", "Write", "MultiEdit", "NotebookEdit"}
BINARY_INSPECT = re.compile(
    r"\b(otool|objdump|llvm-objdump|nm|strings|xxd|hexdump|gdb|lldb|r2|radare2|"
    r"ghidra|readelf|capstone|disas)\b"
)
# Rough themes in Fable 5.2's thinking summaries: a run counts once per theme if any
# summary in it matches. Keyword matching, not a judgement; for describing how the
# reasoning shifts with effort, not for scoring.
THEMES = {
    "disassembly": r"disassembl|otool|objdump|assembly|instruction",
    "black_box_probing": r"black.box|probe|experiment|test input|try (it|a few)",
    "differential_fuzzing": r"fuzz|differential|random(ized)? (input|test)|"
    r"compare (against|with) the (legacy|binary|original)",
    "spec_distrust": r"spec(ification)? (is|may be) (stale|wrong|out of date|drift)|drift|"
    r"doesn.t match the spec|spec says",
    "hypotheses": r"hypothes|my guess|i suspect|likely|probably",
    "self_checks": r"double.check|verify|confirm|make sure|sanity",
    "scope_restraint": r"minimal|smallest|surgical|only change|keep (the|my) (change|patch|diff)|"
    r"don.t (need|want) to (rewrite|refactor)",
    "edge_cases": r"edge case|corner case|overflow|boundary|off.by.one|negative|empty",
    "cleanup": r"clean ?up|scratch|remove (the|my) (temp|debug)|leave the (repo|workspace)",
}
COMMENT = re.compile(r"^\s*(#|//|/\*|\*|--|;|\"\"\"|''')")


def patch_shape(patch: str) -> dict:
    files, added, removed, comments = set(), 0, 0, 0
    for line in patch.splitlines():
        if line.startswith("+++ b/"):
            files.add(line[6:])
        elif line.startswith("+") and not line.startswith("+++"):
            added += 1
            comments += bool(COMMENT.match(line[1:]))
        elif line.startswith("-") and not line.startswith("---"):
            removed += 1
    tests = sum("test" in f.lower() for f in files)
    return {
        "files": len(files),
        "test_files": tests,
        "added": added,
        "removed": removed,
        "comment_share": comments / added if added else 0.0,
    }


def evidence(directory: Path, model: str) -> dict:
    """Run id to (row, patch) from a round's private manifest and evidence files."""
    out = {}
    for row in json.loads((directory / "private-manifest.json").read_text()):
        if row["model"] != model:
            continue
        ev = json.loads((directory / "evidence" / f"{row['id']}.json").read_text())
        out[row["run_id"]] = (row, ev["candidate_patch"])
    return out


def stamp(value: str) -> float:
    return datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp()


def fable52_stream(run: Path) -> dict:
    usage = {"output": 0, "thinking": 0, "cache_read": 0, "cache_write": 0, "turns": 0}
    blocks, chars, results, texts = 0, 0, 0, []
    for line in (run / "cli-agent-stream.jsonl").open():
        event = json.loads(line)
        if event.get("type") == "result":
            results += 1
            u = event["usage"]
            usage["output"] += u["output_tokens"]
            usage["thinking"] += u.get("output_tokens_details", {}).get("thinking_tokens", 0)
            usage["cache_read"] += u["cache_read_input_tokens"]
            usage["cache_write"] += u["cache_creation_input_tokens"]
            usage["turns"] += event.get("num_turns", 0)
        elif event.get("type") == "assistant":
            for block in event["message"]["content"]:
                if block["type"] == "thinking" and block.get("thinking"):
                    blocks += 1
                    chars += len(block["thinking"])
                    texts.append(block["thinking"].lower())
    calls = [json.loads(x) for x in (run / "tool-calls.jsonl").open()]
    summary = json.loads((run / "summary.json").read_text())
    start = stamp(summary["started_at"])
    inspections = [
        i
        for i, c in enumerate(calls)
        if c["tool"] == "Bash" and BINARY_INSPECT.search(c["input"].get("command", ""))
    ]
    first = inspections[0] if inspections else None
    joined = " ".join(texts)
    return (
        usage
        | {f"theme_{k}": bool(re.search(r, joined)) for k, r in THEMES.items()}
        | {
            "results": results,
            "thinking_blocks": blocks,
            "thinking_summary_chars": chars,
            "tool_calls": len(calls),
            "tool_errors": sum(c["status"] != "ok" for c in calls),
            "edit_tool_calls": sum(c["tool"] in EDIT_TOOLS for c in calls),
            "inspected_binary": first is not None,
            "calls_before_binary_inspection": first,
            "minutes_to_binary_inspection": (stamp(calls[first]["started_at"]) - start) / 60
            if first is not None
            else None,
        }
    )


PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
COLORS = {"fable52": "#D97757", "fable51": "#5E5BA8"}  # as on the comparison card
MARKERS = {"fable52": "o", "fable51": "s"}
NAMES = {"fable52": "Claude Fable 5.2", "fable51": "Claude Fable 5.1 (with fallback)"}


def effort_card(rows: list[dict]) -> Path:
    """Minimal 16:9 card: output tokens and minutes per task by effort, both models."""
    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 9.0
    fig = plt.figure(figsize=(W, H), dpi=150, facecolor=PAPER)
    yf = lambda i: 1 - i / H  # noqa: E731

    def text(x, y, s, size=13, fam="Geist", wt=400, ha="left", color=INK):
        fig.text(
            x, yf(y), s, fontsize=size, fontfamily=fam, weight=wt, ha=ha, va="center", color=color
        )

    left, right = 0.06, 0.94
    logo = fig.add_axes([left, yf(0.86), 0.42 / W, 0.42 / H])
    mark = logo.imshow(plt.imread(ROOT / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    text(left + 0.038, 0.65, "VulcanBench", 20, "Chakra Petch SemiBold", 600)
    text(right, 0.65, "Frontier v4  ·  November 2026", 14, ha="right", color=MUTED)
    fig.add_artist(
        plt.Line2D([left, right], [yf(1.05)] * 2, transform=fig.transFigure, color=INK, lw=1.2)
    )
    text(left, 1.72, "Claude Fable 5.2 and Fable 5.1", 30, "Chakra Petch SemiBold", 600)
    x = left
    for m in ("fable52", "fable51"):
        fig.add_artist(
            plt.Line2D(
                [x + 0.004],
                [yf(2.4)],
                transform=fig.transFigure,
                marker=MARKERS[m],
                color=COLORS[m],
                markersize=9,
                markeredgecolor=INK,
                markeredgewidth=0.6,
                linestyle="none",
            )
        )
        text(x + 0.016, 2.4, NAMES[m], 13, wt=700)
        x += 0.17

    def panel(x0, title, sub, key, scale, fmt):
        text(x0, 3.0, title, 20, "Chakra Petch SemiBold", 600)
        text(x0, 3.32, sub, 12, color=MUTED)
        ax = fig.add_axes([x0 + 0.025, yf(7.6), 0.36, 3.9 / H], facecolor=PAPER)
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.tick_params(axis="x", length=0, labelsize=12, pad=8)
        ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
        ax.grid(axis="y", color=RULE, linewidth=0.5)
        ax.set_axisbelow(True)
        ax.set_xticks(range(len(LEVELS)), [e.replace("-", " ").capitalize() for e in LEVELS])
        ax.set_xlim(-0.35, len(LEVELS) - 0.35)
        series = {
            m: [
                statistics.mean(r[key] for r in rows if r["model"] == m and r["effort"] == e)
                / scale
                for e in LEVELS
            ]
            for m in ("fable52", "fable51")
        }
        top = max(max(ys) for ys in series.values())
        for m, ys in series.items():
            other = series["fable51" if m == "fable52" else "fable52"]
            ax.plot(range(len(LEVELS)), ys, color=COLORS[m], lw=2, zorder=2)
            ax.plot(
                range(len(LEVELS)),
                ys,
                MARKERS[m],
                color=COLORS[m],
                markersize=8,
                markeredgecolor=INK,
                markeredgewidth=0.6,
                zorder=3,
            )
            # The higher point at each level is labelled above, the lower one below.
            for i, (y, o) in enumerate(zip(ys, other, strict=True)):
                ax.annotate(
                    fmt(y),
                    (i, y),
                    xytext=(0, 9 if y >= o else -16),
                    textcoords="offset points",
                    ha="center",
                    fontsize=9,
                    fontfamily="IBM Plex Mono",
                    color=INK,
                )
        ax.set_ylim(0, top * 1.18)

    panel(
        left,
        "Output per task",
        "thousand tokens, thinking included  ·  mean of 23 runs",
        "output",
        1000,
        lambda v: f"{v:.0f}",
    )
    panel(0.535, "Time per task", "minutes  ·  mean of 23 runs", "minutes", 1, lambda v: f"{v:.1f}")
    text(
        left,
        8.4,
        "Tasks solved: Fable 5.2 115 of 115, Fable 5.1 104 of 115. Fable 5.1 ran in September 2026 on "
        "Claude Code 2.1.259 to 2.1.261; its refusal fallback answered part of 11 runs.",
        11,
        color=MUTED,
    )
    out = OUTPUT / "fable52-vs-fable51-effort.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--v34", type=Path, required=True)
    parser.add_argument("--v330", type=Path, required=True)
    parser.add_argument("--v3301", type=Path, required=True)
    args = parser.parse_args()

    rows = []
    for run_id, (row, patch) in evidence(args.v34, "fable").items():
        u = row["solver_receipt"]["usage"]
        rows.append(
            {
                "model": "fable51",
                "effort": row["effort"],
                "task": row["task"],
                "run_id": run_id,
                "solved": row["functional"] >= 1,
                "fallback": row["fallback"],
                "minutes": row["duration_s"] / 60,
                "output": u["output_tokens"],
                "cache_read": u["cache_read_input_tokens"],
                "cache_write": u["cache_creation_input_tokens"],
                "cli": row["solver_cli_version"],
            }
            | {f"patch_{k}": v for k, v in patch_shape(patch).items()}
        )
    fable52 = evidence(args.v330, "fable52") | evidence(args.v3301, "fable52")
    for run_id, (row, patch) in fable52.items():
        run = RUNS / row["effort"] / run_id
        s = json.loads((run / "summary.json").read_text())
        stream = fable52_stream(run)
        receipt = row["solver_receipt"]
        if (stream["output"], stream["thinking"]) != (
            receipt["usage"]["output_tokens"],
            receipt["thinking_tokens"],
        ):
            raise SystemExit(f"Fable 5.2 {run_id}: stream totals disagree with its receipt")
        rows.append(
            {
                "model": "fable52",
                "effort": row["effort"],
                "task": row["task"],
                "run_id": run_id,
                "solved": s["scores"]["functional"] >= 1,
                "fallback": set(s["served_models"]) != {"claude-fable-5-2"},
                "minutes": s["duration_s"] / 60,
                "cli": s["cli_agent"]["harness_version"],
            }
            | stream
            | {f"patch_{k}": v for k, v in patch_shape(patch).items()}
        )
    counts = {
        (m, e): sum(r["model"] == m and r["effort"] == e for r in rows)
        for m in ("fable51", "fable52")
        for e in LEVELS
    }
    if set(counts.values()) != {23}:
        raise SystemExit(f"expected 23 runs per cell, got {counts}")

    fields = list(dict.fromkeys(k for r in rows for k in r))
    with (OUTPUT / "fable52-vs-fable51-runs.csv").open("w", newline="") as fh:
        w = csv.DictWriter(fh, fields, lineterminator="\n")
        w.writeheader()
        for r in sorted(rows, key=lambda r: (r["model"], LEVELS.index(r["effort"]), r["task"])):
            w.writerow({k: (f"{v:.4f}" if isinstance(v, float) else v) for k, v in r.items()})

    measures = [
        "solved",
        "fallback",
        "minutes",
        "output",
        "thinking",
        "cache_read",
        "turns",
        "tool_calls",
        "tool_errors",
        "edit_tool_calls",
        "inspected_binary",
        "calls_before_binary_inspection",
        "minutes_to_binary_inspection",
        "thinking_blocks",
        "thinking_summary_chars",
        "patch_files",
        "patch_test_files",
        "patch_added",
        "patch_removed",
        "patch_comment_share",
        *(f"theme_{k}" for k in THEMES),
    ]
    with (OUTPUT / "fable52-vs-fable51-levels.csv").open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(["model", "effort", "n", *measures, "median_minutes", "median_output"])
        for m in ("fable52", "fable51"):
            for e in LEVELS:
                cell = [r for r in rows if r["model"] == m and r["effort"] == e]
                out = []
                for k in measures:
                    vals = [r[k] for r in cell if r.get(k) is not None]
                    if not vals:
                        out.append("")
                    elif isinstance(vals[0], bool):
                        out.append(sum(vals))
                    else:
                        out.append(f"{statistics.mean(vals):.4f}")
                w.writerow(
                    [
                        m,
                        e,
                        len(cell),
                        *out,
                        f"{statistics.median(r['minutes'] for r in cell):.4f}",
                        f"{statistics.median(r['output'] for r in cell):.1f}",
                    ]
                )
    print((OUTPUT / "fable52-vs-fable51-levels.csv").read_text())
    print(effort_card(rows))


if __name__ == "__main__":
    main()
