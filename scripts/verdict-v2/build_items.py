#!/usr/bin/env python3
"""Build the Verdict v2 item file from every family that has a builder.

Prints per-family counts, answer balance and shortcut accuracy so a
lopsided family is visible before any model sees it. Families without a
builder are listed as unbuilt. Writes no item content to stdout.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from harness.verdict.v2.items import answer_label, write_items  # noqa: E402
from harness.verdict.v2.registry import FAMILIES, BuildContext, builder_for  # noqa: E402

# Run archives and task trees live in the main checkout (they are untracked),
# so a worktree build points at it by default.
MAIN_CHECKOUT = Path.home() / "dev" / "VulcanBench"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--seed", type=int, default=20260924)
    parser.add_argument("--per-family", type=int, default=250)
    parser.add_argument("--family", action="append", help="build only these (repeatable)")
    parser.add_argument("--data-root", type=Path, default=MAIN_CHECKOUT)
    parser.add_argument("-o", "--out", type=Path, default=REPO / "verdict-v2-items" / "items.jsonl")
    args = parser.parse_args()

    root = args.data_root
    ctx = BuildContext(
        seed=args.seed,
        repo=REPO,
        run_roots=tuple(sorted(p for p in root.glob("runs*") if p.is_dir())),
        tasks_roots=(root / "tasks",),
        per_family=args.per_family,
    )
    items = []
    unbuilt = []
    for family in FAMILIES:
        if args.family and family.family_id not in args.family:
            continue
        build = builder_for(family.family_id)
        if build is None:
            unbuilt.append(family.family_id)
            continue
        built = build(ctx)
        items.extend(built)
        splits = Counter(i.split for i in built)
        units = len({i.source_unit for i in built})
        answers = Counter(answer_label(i.question, i.answer) for i in built)
        top_share = answers.most_common(1)[0][1] / len(built) if built else 0.0
        shortcut_acc = {
            name: sum(i.shortcuts.get(name) == answer_label(i.question, i.answer) for i in built)
            / len(built)
            for name in sorted({n for i in built for n in i.shortcuts})
        }
        shortcuts = ", ".join(f"{k} {v:.0%}" for k, v in shortcut_acc.items()) or "none"
        print(
            f"{family.family_id:18} {len(built):5} items  dev {splits['dev']:4}  "
            f"test {splits['test']:4}  units {units:4}  top answer {top_share:.0%}  "
            f"shortcuts: {shortcuts}"
        )
    if unbuilt:
        print(f"unbuilt ({len(unbuilt)}): {', '.join(unbuilt)}")
    ids = [i.item_id for i in items]
    if len(ids) != len(set(ids)):
        print("duplicate item ids", file=sys.stderr)
        return 1
    write_items(items, args.out)
    print(f"wrote {len(items)} items to {args.out}")
    write_manifest(args, items)
    return 0


def write_manifest(args: argparse.Namespace, items: list) -> None:
    """Record how the item file was built, next to it: the freeze record."""

    def git(*cmd: str) -> str:
        return subprocess.run(
            ["git", *cmd], cwd=REPO, capture_output=True, text=True, check=False
        ).stdout.strip()

    counts: dict[str, Counter[str]] = {}
    for item in items:
        counts.setdefault(item.family, Counter())[item.split] += 1
    manifest = {
        "suite": "verdict-v2",
        "built_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "seed": args.seed,
        "per_family": args.per_family,
        "families_requested": args.family,
        "data_root": str(args.data_root),
        "git_commit": git("rev-parse", "HEAD"),
        # Uncommitted changes to builders make the commit an incomplete record.
        "git_dirty": bool(git("status", "--porcelain", "--", "harness", "scripts")),
        "items": len(items),
        "items_sha256": hashlib.sha256(args.out.read_bytes()).hexdigest(),
        "per_family_splits": {f: dict(c) for f, c in sorted(counts.items())},
    }
    path = args.out.with_suffix(".manifest.json")
    path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"wrote manifest to {path}")


if __name__ == "__main__":
    raise SystemExit(main())
