#!/usr/bin/env python3
"""Fail if an em-dash appears in project-authored source or docs.

VulcanBench forbids em-dashes (U+2014) in all output (see AGENTS.md and
CLAUDE.md). This check enforces that mechanically for tracked, project-authored
text. Third-party task fixtures under ``tasks/`` and vendored trees are
excluded: their upstream text is not ours to rewrite. The rule-definition files
(AGENTS.md, CLAUDE.md, and this script) are excluded because their job is to
name the forbidden character.

Usage:
    python scripts/check_no_emdash.py            # scan all tracked files
    python scripts/check_no_emdash.py a.md b.py  # scan just these (pre-commit)
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

EM_DASH = "—"

# Extensions and filenames we author and therefore police.
SCAN_SUFFIXES = {
    ".py",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".md",
    ".mdx",
    ".yml",
    ".yaml",
    ".toml",
    ".sh",
    ".css",
    ".txt",
    ".mdc",
    ".example",
}
SCAN_NAMES = {"Makefile", "Dockerfile"}

# Files whose purpose is to name the forbidden character, so they legitimately
# contain it.
EXCLUDE_EXACT = {
    "AGENTS.md",
    "CLAUDE.md",
    "dashboard/AGENTS.md",
    "scripts/check_no_emdash.py",
}

# Trees we do not author (third-party task fixtures, vendored deps, agent
# scratch) and generated lockfiles.
EXCLUDE_PREFIXES = (
    "tasks/",
    "traces/",  # published agent traces: model output kept verbatim
    ".claude/",
    "node_modules/",
    "dashboard/node_modules/",
)
EXCLUDE_SUFFIXES = ("package-lock.json", ".lock")


def _tracked_files() -> list[str]:
    out = subprocess.run(["git", "ls-files"], capture_output=True, text=True, check=True).stdout
    return out.splitlines()


def _should_scan(path: str) -> bool:
    if path in EXCLUDE_EXACT:
        return False
    if any(path.startswith(prefix) for prefix in EXCLUDE_PREFIXES):
        return False
    if any(path.endswith(suffix) for suffix in EXCLUDE_SUFFIXES):
        return False
    p = Path(path)
    return p.suffix in SCAN_SUFFIXES or p.name in SCAN_NAMES or p.name.startswith("Dockerfile")


def main(argv: list[str]) -> int:
    candidates = argv[1:] if len(argv) > 1 else _tracked_files()
    offenders: list[tuple[str, int, int, str]] = []
    for path in candidates:
        if not _should_scan(path):
            continue
        try:
            text = Path(path).read_text(encoding="utf-8")
        except (UnicodeDecodeError, FileNotFoundError, IsADirectoryError):
            continue
        for lineno, line in enumerate(text.splitlines(), 1):
            if EM_DASH in line:
                col = line.index(EM_DASH) + 1
                offenders.append((path, lineno, col, line.strip()))

    if offenders:
        print(
            f"Found {len(offenders)} em-dash(es) (U+2014). VulcanBench forbids "
            "them in all output; see AGENTS.md."
        )
        for path, lineno, col, snippet in offenders:
            print(f"  {path}:{lineno}:{col}: {snippet}")
        print("\nReplace each with a comma, a colon, parentheses, or two sentences.")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
