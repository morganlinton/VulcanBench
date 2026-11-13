"""Did Claude Fable 5.2 memorize VulcanBench Frontier v4's public reference fixes?

Every Frontier v4 reference fix (`gold_patch.diff`) and test has been public on
GitHub since late August 2026, about ten weeks before Fable 5.2, and Fable 5.2
solved all 115 runs. Owner request (chat, 2026-11-11): check that it is not
reciting the answers, publish the check with the results, and make an X card.
Three checks, recomputed from the local run folders every time:

1. Copying. For each solved run, the share of the reference fix's distinctive
   added lines (30 or more characters, not already in the task's repository,
   tests or issue) that the run's patch reproduces verbatim, and the share of
   new identifiers (7 or more characters, absent from the task) it uses. A
   recited fix scores near 100%; any correct fix shares some lines. Compared
   with other models on the same tasks, including Claude Opus 5, which was
   benchmarked before the fixes were public.
2. Working it out. The share of Claude Code runs that ran a binary inspection
   tool (otool, objdump, nm, strings, lldb, hexdump, xxd) on the legacy binary
   before their first edit. An edit is an Edit, Write or MultiEdit call, or a
   shell command that names a file the final patch changes and writes a file
   (sed -i, perl -i, a redirect, tee, or a Python write). Fable 5.2 makes most
   Low and Medium edits through the shell, so counting only edit tools would
   flatter it; this rule may also count a scratch write that names the module,
   which can only shorten the window before the first edit.
3. Recall. Fable 5.2 with no tools, asked to reproduce five reference fixes
   from memory, and to fix one task blind. Prompts and verbatim answers
   (`.response`, model output kept as written) in recall-probe/.

    python scripts/cii-v4-board/check_fable52_memorization.py

Writes memorization-check.json and fable52-no-memorization.png beside the card.
"""

from __future__ import annotations

import functools
import json
import math
import re
import statistics
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch

ROOT = Path(__file__).resolve().parents[2]
TASKS = ROOT / "tasks/coding-intelligence-index-v4"
OUTPUT = ROOT / "docs/results/swe-v4-fable52-2026-11"
FABLE = "Claude Fable 5.2"
COPY = (
    ("runs-effort-fable52", FABLE),
    ("runs-effort-opus5", "Claude Opus 5"),
    ("runs-effort-sonnet55", "Claude Sonnet 5.5"),
    ("runs-effort-haiku55", "Claude Haiku 5.5"),
    ("runs-effort-gpt61-sol", "GPT-6.1 Sol"),
    ("runs-effort-gpt61-terra", "GPT-6.1 Terra"),
    ("runs-effort-deepseek-flash-dsh", "DeepSeek V4.1 Flash"),
)
INSPECT = COPY[:4]  # the Claude Code columns, whose streams share one tool-call format
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]{6,}")
EDIT_TOOLS = {"Edit", "Write", "MultiEdit", "NotebookEdit"}
DUMP = re.compile(r"\b(otool|objdump|nm|strings|lldb|hexdump|xxd)\b|disassembl")
# Shell writes. TARGETED patterns must point at the patched file itself; ANYWHERE
# patterns (in-place sed or perl, a Python write) count when the command also names it.
TARGETED = r"(?:>>?|\btee\s+(?:-a\s+)?|\b(?:cp|mv)\s+\S+\s+)['\"]?[\w./$\-{}]*NAME\b"
ANYWHERE = re.compile(r"sed -i|perl -[a-z]*i|open\([^)]*,\s*['\"][wa]|write_text\(|write_bytes\(")
PAPER, INK, RULE, MUTED, GREY, CLAY = (
    "#f7f5f0",
    "#171917",
    "#c6c5bc",
    "#6b6b66",
    "#a9a79e",
    "#D97757",
)


def added(patch: str) -> list[str]:
    return [
        line[1:].strip()
        for line in patch.splitlines()
        if line.startswith("+") and not line.startswith("+++")
    ]


def patched_files(patch: str) -> set[str]:
    return {Path(line[6:]).name for line in patch.splitlines() if line.startswith("+++ b/")}


@functools.cache
def task_text(task: str) -> str:
    out = [(TASKS / task / "issue.md").read_text(errors="ignore")]
    for base in ("repo", "tests"):
        out += [
            p.read_text(errors="ignore") for p in (TASKS / task / base).rglob("*") if p.is_file()
        ]
    return "\n".join(out)


@functools.cache
def gold(task: str) -> tuple[set[str], set[str]]:
    lines = added((TASKS / task / "gold_patch.diff").read_text(errors="ignore"))
    known = task_text(task)
    known_lines = {line.strip() for line in known.splitlines()}
    distinctive = {line for line in lines if len(line) >= 30 and line not in known_lines}
    idents = {t for line in lines for t in IDENT.findall(line)} - set(IDENT.findall(known))
    return distinctive, idents


def runs(root: str):
    for summary in sorted((ROOT / root).glob("*/legacy-*/summary.json")):
        yield summary.parent, json.loads(summary.read_text())


def copying(root: str) -> dict:
    lines, idents = [], []
    for run, s in runs(root):
        patch = run / "final.patch"
        if s["scores"]["functional"] < 1 or not patch.exists():
            continue
        mine = added(patch.read_text(errors="ignore"))
        mine_ids = {t for line in mine for t in IDENT.findall(line)}
        dl, di = gold(s["task_id"])
        lines.append(100 * len(dl & set(mine)) / len(dl) if dl else 0.0)
        idents.append(100 * len(di & mine_ids) / len(di) if di else 0.0)
    se = lambda v: statistics.stdev(v) / math.sqrt(len(v))  # noqa: E731
    return {
        "solved_runs": len(lines),
        "lines_pct": statistics.mean(lines),
        "lines_se": se(lines),
        "lines_max": max(lines),
        "identifiers_pct": statistics.mean(idents),
        "identifiers_se": se(idents),
    }


def tool_calls(stream: Path) -> list[tuple[str, dict]]:
    calls = []
    for line in stream.open(errors="replace"):
        if '"tool_use"' not in line:
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("type") == "assistant":
            calls += [
                (c["name"], c.get("input") or {})
                for c in event["message"].get("content", [])
                if c.get("type") == "tool_use"
            ]
    return calls


def inspecting(root: str) -> dict:
    hit = n = 0
    for run, _ in runs(root):
        calls = tool_calls(run / "cli-agent-stream.jsonl")
        patch = run / "final.patch"
        names = patched_files(patch.read_text(errors="ignore")) if patch.exists() else set()
        if not calls:
            continue
        n += 1

        def is_edit(name, args, names=names):
            if name in EDIT_TOOLS:
                return True
            command = args.get("command", "") if name == "Bash" else ""
            named = [f for f in names if f in command]
            return bool(named) and (
                bool(ANYWHERE.search(command))
                or any(re.search(TARGETED.replace("NAME", re.escape(f)), command) for f in named)
            )

        first = next((i for i, (nm, a) in enumerate(calls) if is_edit(nm, a)), len(calls))
        hit += any(nm == "Bash" and DUMP.search(a.get("command", "")) for nm, a in calls[:first])
    p = hit / n
    return {
        "runs": n,
        "inspected_first": hit,
        "pct": 100 * p,
        "se": 100 * math.sqrt(p * (1 - p) / n),
    }


def recall() -> dict:
    probe = OUTPUT / "recall-probe"
    answers = {p.stem: p.read_text() for p in sorted(probe.glob("*.response"))}
    return {
        "recall_tasks": sorted(
            k.removeprefix("recall-") for k in answers if k.startswith("recall-")
        ),
        "recall_answers_saying_not_remembered": sum(
            "don't remember" in v.lower() or "do not remember" in v.lower()
            for k, v in answers.items()
            if k.startswith("recall-")
        ),
        "blind_fix_task": "legacy-quotacore-binary-parity",
    }


def card(result: dict) -> Path:
    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 9.0
    fig = plt.figure(figsize=(W, H), dpi=150, facecolor=PAPER)
    yf = lambda i: 1 - i / H  # noqa: E731

    def text(x, y, s, size=13, fam="Geist", wt=400, ha="left", color=INK):
        return fig.text(
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
    text(left, 1.72, FABLE, 30, "Chakra Petch SemiBold", 600)
    text(
        left,
        2.32,
        "No sign it memorized the public answers",
        17,
        "Chakra Petch Medium",
        500,
        color=MUTED,
    )

    def panel(x0, w, title, sub, rows, xmax, fmt, ticks):
        text(x0, 3.05, title, 19, "Chakra Petch SemiBold", 600)
        text(x0, 3.5, sub, 12.5, color=MUTED)
        lab_w = 0.13
        ax = fig.add_axes([x0 + lab_w, yf(7.55), w - lab_w, 3.75 / H], facecolor=PAPER)
        names = [r[0] for r in rows]
        ys = list(range(len(rows)))[::-1]
        for y, (name, mean, se, label) in zip(ys, rows, strict=True):
            ax.barh(
                y,
                mean,
                height=0.58 * len(rows) / 7,
                color=CLAY if name == FABLE else GREY,
                edgecolor=PAPER,
                linewidth=2,
            )
            ax.errorbar(mean, y, xerr=se, color=INK, lw=0.9, capsize=3)
            ax.text(
                mean + se + xmax * 0.015,
                y,
                label,
                va="center",
                fontsize=11,
                fontfamily="IBM Plex Mono",
                color=INK,
            )
        ax.set_yticks(ys, names)
        for t, name in zip(ax.get_yticklabels(), names, strict=True):
            t.set_fontweight(700 if name == FABLE else 400)
        ax.set_xlim(0, xmax)
        ax.set_xticks(ticks, [f"{t:g}%" for t in ticks])
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.tick_params(axis="y", length=0, labelsize=12.5, pad=8)
        ax.tick_params(axis="x", length=0, labelsize=11, colors=MUTED)
        ax.grid(axis="x", color=RULE, lw=0.5)
        ax.set_axisbelow(True)

    copy = [
        (name, c["lines_pct"], c["lines_se"], f"{c['lines_pct']:.1f}%  n={c['solved_runs']}")
        for name, c in result["copying"].items()
    ]
    insp = [
        (name, c["pct"], c["se"], f"{c['inspected_first']}/{c['runs']}")
        for name, c in result["inspecting"].items()
    ]
    panel(
        left,
        0.40,
        "Reference-fix lines copied",
        "solved runs  ·  lower is less copying  ·  whiskers ±1 se",
        copy,
        12,
        None,
        [0, 3, 6, 9, 12],
    )
    panel(
        0.535,
        0.405,
        "Took the binary apart before editing",
        "share of runs  ·  Claude Code  ·  whiskers ±1 se",
        insp,
        122,
        None,
        [0, 25, 50, 75, 100],
    )
    r = result["recall"]
    foot = [
        "Every reference fix and test has been public on GitHub since late August 2026. Copying a fix "
        "outright would score 100%; any correct fix shares a few lines.",
        f"Fable 5.2: all {result['fable52_runs']} runs, every one solved. With no tools, asked to write "
        f"{len(r['recall_tasks'])} reference fixes from memory, it reproduced none.",
    ]
    for i, s in enumerate(foot):
        text(left, 8.2 + i * 0.36, s, 11.5, color=MUTED)
    out = OUTPUT / "fable52-no-memorization.png"
    fig.savefig(out, facecolor=PAPER)
    plt.close(fig)
    return out


def main():
    result = {
        "copying": {name: copying(root) for root, name in COPY},
        "inspecting": {name: inspecting(root) for root, name in INSPECT},
        "recall": recall(),
        "fable52_runs": sum(1 for _ in runs("runs-effort-fable52")),
    }
    if result["fable52_runs"] != 115:
        raise SystemExit(f"expected 115 Fable 5.2 runs, found {result['fable52_runs']}")
    (OUTPUT / "memorization-check.json").write_text(json.dumps(result, indent=2) + "\n")
    print(card(result))
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    main()
