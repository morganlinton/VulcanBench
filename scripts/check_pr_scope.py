"""PR hygiene: keep engine changes and results apart, and keep raw output out of git.

Three rules, run by .github/workflows/pr-hygiene.yml on every pull request
(the reasons are in docs/HOW_WE_WORK.md):

1. Scope. A PR changes the engine (harness, tasks, sandbox, backend, run
   config) or publishes results (docs/results, traces), never both. Results
   come from a tagged engine commit that is already on main. The ``mixed-scope``
   label overrides this for a deliberate exception; say why in the PR.
2. No new raw traces. Nothing new lands under traces/ except Markdown
   pointers; raw run output goes to the archive (scripts/archive_runs.sh) or
   a GitHub Release asset.
3. No large blobs. Outside tasks/ (whose snapshots use Git LFS and get their
   own validation), no added or modified file may exceed 5 MiB.

    python scripts/check_pr_scope.py --base origin/main --head HEAD --labels "a,b"
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from dataclasses import dataclass

ENGINE_PREFIXES = (
    "harness/",
    "tasks/",
    "sandbox/",
    "backend/",
    "alembic/",
    "alembic.ini",
    "vulcanbench.toml",
    "pyproject.toml",
    "uv.lock",
)
RESULTS_PREFIXES = ("docs/results/", "traces/")
MAX_BLOB_BYTES = 5 * 1024 * 1024
OVERRIDE_LABEL = "mixed-scope"


@dataclass(frozen=True)
class Change:
    status: str  # first letter of git's name-status: A, M, D, R, C, T
    path: str


def classify(path: str) -> str:
    if path.startswith(ENGINE_PREFIXES):
        return "engine"
    if path.startswith(RESULTS_PREFIXES):
        return "results"
    return "neutral"


def scope_problems(changes: list[Change], labels: set[str]) -> list[str]:
    engine = sorted({c.path for c in changes if classify(c.path) == "engine"})
    results = sorted({c.path for c in changes if classify(c.path) == "results"})
    if not engine or not results or OVERRIDE_LABEL in labels:
        return []

    def sample(paths: list[str]) -> str:
        return ", ".join(paths[:3]) + (f" (+{len(paths) - 3} more)" if len(paths) > 3 else "")

    return [
        "this PR mixes engine changes and published results. Merge the engine "
        "change first, cut a bench tag, run from it, then open the results PR. "
        f"Engine: {sample(engine)}. Results: {sample(results)}. "
        f"(Label the PR '{OVERRIDE_LABEL}' for a deliberate exception.)"
    ]


def trace_problems(changes: list[Change]) -> list[str]:
    added = [
        c.path
        for c in changes
        if c.status in {"A", "C", "R"} and c.path.startswith("traces/")
        if not c.path.endswith(".md")
    ]
    if not added:
        return []
    shown = ", ".join(added[:3]) + (f" (+{len(added) - 3} more)" if len(added) > 3 else "")
    return [
        f"{len(added)} new raw file(s) under traces/ ({shown}). Archive raw run "
        "output with scripts/archive_runs.sh, or attach a redacted bundle to a "
        "GitHub Release, and commit only a README pointing at it."
    ]


def size_problems(changes: list[Change], sizes: dict[str, int]) -> list[str]:
    out = []
    for c in changes:
        if c.status == "D" or c.path.startswith("tasks/"):
            continue
        size = sizes.get(c.path, 0)
        if size > MAX_BLOB_BYTES:
            out.append(
                f"{c.path} is {size / 1024 / 1024:.1f} MiB (limit "
                f"{MAX_BLOB_BYTES // 1024 // 1024} MiB outside tasks/). Archive it instead."
            )
    return out


def _git(*args: str) -> str:
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout


def changed_files(base: str, head: str) -> list[Change]:
    out = _git("diff", "--name-status", "--no-renames", f"{base}...{head}")
    changes = []
    for line in out.splitlines():
        parts = line.split("\t")
        if len(parts) >= 2:
            changes.append(Change(status=parts[0][:1], path=parts[-1]))
    return changes


def blob_sizes(head: str, changes: list[Change]) -> dict[str, int]:
    sizes = {}
    for c in changes:
        if c.status != "D" and not c.path.startswith("tasks/"):
            sizes[c.path] = int(_git("cat-file", "-s", f"{head}:{c.path}").strip())
    return sizes


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--labels", default="", help="comma-separated PR labels")
    args = parser.parse_args(argv)

    labels = {lab.strip() for lab in args.labels.split(",") if lab.strip()}
    changes = changed_files(args.base, args.head)
    problems = [
        *scope_problems(changes, labels),
        *trace_problems(changes),
        *size_problems(changes, blob_sizes(args.head, changes)),
    ]
    kinds = sorted({classify(c.path) for c in changes} - {"neutral"}) or ["neutral"]
    print(f"{len(changes)} changed file(s); scope: {', '.join(kinds)}")
    for problem in problems:
        print(f"FAIL {problem}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
