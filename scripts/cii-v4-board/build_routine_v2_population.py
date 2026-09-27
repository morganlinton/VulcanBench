"""Build the judging population for the private Routine v2 sweeps.

Same scan and row shape as the Routine v1 builder (``build_routine_population.py``):
every finished run under ``VulcanRoutine/runs/runs-routine-v2-<model>/<level>/``,
unfinished runs listed under "excluded", untried task and level pairs under
"missing", the latest of any duplicate kept. One rule is added, as v3.11 ruled
for Frontier: a run whose patch changes no source file in the task's language
has no code to review, so it is excluded with that reason rather than judged.

The record names private tasks, so it is written inside the private
repository (``results/private/routine-v2-comparison.json``), never here.

    python scripts/cii-v4-board/build_routine_v2_population.py
"""

from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_routine_population import ALL_LEVELS, build  # noqa: E402

ROUTINE = Path(os.environ.get("VULCANROUTINE_ROOT", ROOT.parent / "VulcanRoutine")).resolve()
TASKS = ROUTINE / "tasks/routine-v2"
OUTPUT = ROUTINE / "results/private/routine-v2-comparison.json"
# Must match MODELS in harness/maintenance_review_routine_v2.py, which checks
# every cell at freeze. The default roster is Routine v1's.
MODELS = {
    "astra": ("runs-routine-v2-astra", ALL_LEVELS, "codex:gpt-6-astra", "GPT-6 Astra in Codex"),
    "terra": ("runs-routine-v2-terra", ALL_LEVELS, "codex:gpt-5.6-terra", "GPT-5.6 Terra in Codex"),
    "luna": ("runs-routine-v2-luna", ALL_LEVELS, "codex:gpt-5.6-luna", "GPT-5.6 Luna in Codex"),
    "sol": ("runs-routine-v2-sol", ALL_LEVELS, "codex:gpt-5.6-sol", "GPT-5.6 Sol in Codex"),
    "gpt55": (
        "runs-routine-v2-gpt55",
        ("low", "medium", "high", "extra-high"),
        "codex:gpt-5.5",
        "GPT-5.5 in Codex",
    ),
    "fable": (
        "runs-routine-v2-fable",
        ALL_LEVELS,
        "claude-code:claude-fable-5-1",
        "Claude Fable 5.1 in Claude Code",
    ),
    "opus55": (
        "runs-routine-v2-opus55",
        ALL_LEVELS,
        "claude-code:claude-opus-5-5",
        "Claude Opus 5.5 in Claude Code",
    ),
    "swe2": (
        "runs-routine-v2-swe2",
        ("medium", "high", "max"),
        "devin:swe-2",
        "SWE-2 in Devin CLI",
    ),
}
SOURCE_SUFFIXES = {
    "python": (".py",),
    "javascript": (".js", ".mjs", ".cjs"),
    "rust": (".rs",),
    "c": (".c", ".h"),
    "cpp": (".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx"),
}
NO_SOURCE_REASON = "Changed no source file in the task's language; nothing to review"


def changed_paths(patch: str) -> list[str]:
    return [
        m.group(1) for m in re.finditer(r"^diff --git a/\S+ b/(\S+)$", patch, flags=re.MULTILINE)
    ]


def exclude_sourceless(record: dict, tasks_root: Path) -> dict:
    """Move rows whose patch touches no source file in the task's language to "excluded"."""
    kept = []
    for row in record["rows"]:
        language = json.loads((tasks_root / row["task"] / "metadata.json").read_text())[
            "languages"
        ][0]
        patch = (Path(row["source_directory"]) / "final.patch").read_text()
        if any(p.endswith(SOURCE_SUFFIXES[language]) for p in changed_paths(patch)):
            kept.append(row)
            continue
        record["excluded"].append(
            {
                "model": row["model"],
                "effort": row["effort"],
                "task": row["task"],
                "run_id": row["run_id"],
                "reason": NO_SOURCE_REASON,
                "finished": True,
                "duration_s": row["duration_s"],
                "functional": row["functional"],
            }
        )
    record["rows"] = kept
    cells: dict[str, int] = {}
    for r in kept:
        cells[f"{r['model']}/{r['effort']}"] = cells.get(f"{r['model']}/{r['effort']}", 0) + 1
    record["cells"] = cells
    return record


def main() -> None:
    record = build(MODELS, ROUTINE / "runs", TASKS)
    record["suite"] = "routine-v2"
    record = exclude_sourceless(record, TASKS)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(record, indent=1, sort_keys=True) + "\n")
    print(
        f"{len(record['rows'])} rows, {len(record['excluded'])} excluded, "
        f"{len(record['missing'])} missing, {len(record['duplicates'])} duplicates"
    )


if __name__ == "__main__":
    main()
