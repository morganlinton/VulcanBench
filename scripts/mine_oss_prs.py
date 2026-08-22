#!/usr/bin/env python3
"""Mine candidate coding-task PRs for a language-specific suite (read-only, via ``gh``).

Sibling of ``mine_security_prs.py`` but **keyword-agnostic**: instead of a security
signal it selects real, merged, *test-bearing* PRs of task-shaped size across a
curated repo set keyed by VulcanBench ``domain`` tag. Built for Python Suite v1
(see ``tasks/python-1/CHARTER.md``): the point is to fill the difficulty band, so
this is discovery only — a human measures each candidate (repeat >= 3) and admits.

It shells out to the authenticated ``gh`` CLI only and makes read calls only
(``gh search prs``, ``gh pr view``). It never writes into a task and never
fabricates provenance.

Usage::

    python scripts/mine_oss_prs.py                       # all Python domains
    python scripts/mine_oss_prs.py --domain data-orm     # one domain
    python scripts/mine_oss_prs.py --since 2026-02-01 --json out.json --per-repo 30

Output columns: domain | repo | PR | merged | +/- LOC | files | title.
Kept: merged on/after ``--since`` AND touches a test path AND task-shaped size
(``--min-loc``..``--max-loc`` net source lines, ``--max-files`` files) unless
overridden. ``base_commit`` for slicing is the parent of the PR's first commit
(reported in JSON). Always re-confirm the base actually fails before building.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from dataclasses import asdict, dataclass

# --- Curated Python repo set, keyed by VulcanBench domain tag ----------------
# Seeds, not exhaustive — extend freely; discovery is the point. Keep the set
# spread across domains so the mid band does not end up dominated by one library.
DOMAIN_REPOS: dict[str, list[str]] = {
    # App-scale, pure-Python frameworks whose PRs are real multi-module
    # engineering (not one-file library edge fixes) — the shape that still
    # discriminates the 2026 frontier. Mine these with a wider --max-loc.
    "app-scale": [
        "django/django",
        "sqlalchemy/sqlalchemy",
        "celery/celery",
        "scrapy/scrapy",
        "sphinx-doc/sphinx",
        "dbt-labs/dbt-core",
        "encode/django-rest-framework",
        "pallets/werkzeug",
        "python-poetry/poetry",
        "mitmproxy/mitmproxy",
    ],
    "web-async": [
        "pallets/flask",
        "encode/starlette",
        "encode/httpx",
        "aio-libs/aiohttp",
        "fastapi/fastapi",
        "tornadoweb/tornado",
    ],
    "data-orm": [
        "tobymao/sqlglot",
        "sqlalchemy/sqlalchemy",
        "pandas-dev/pandas",
        "pola-rs/polars",
        "ibis-project/ibis",
    ],
    "parsing": [
        "python/cpython",  # narrow with --path-ish titles; big repo, scan shallow
        "lark-parser/lark",
        "python-jsonschema/jsonschema",
        "marshmallow-code/marshmallow",
        "pydantic/pydantic",
    ],
    "scientific": [
        "networkx/networkx",
        "numpy/numpy",
        "scipy/scipy",
        "PennyLaneAI/pennylane",
        "sympy/sympy",
    ],
    "stdlib-utility": [
        "more-itertools/more-itertools",
        "python-attrs/attrs",
        "arrow-py/arrow",
        "dateutil/dateutil",
        "jaraco/inflect",
    ],
    "cli-tooling": [
        "pallets/click",
        "fastapi/typer",
        "pypa/packaging",
        "pypa/pip",
        "python-poetry/poetry",
    ],
    "concurrency": [
        "python-trio/trio",
        "agronholm/anyio",
        "aio-libs/aiohttp",
        "dask/dask",
    ],
}

_TEST_MARKERS = (
    "test",
    "tests",
    "spec",
    "conftest.py",
    "test_",
    "_test.py",
    ".test.",
    ".spec.",
)


@dataclass
class Candidate:
    domain: str
    repo: str
    number: int
    title: str
    url: str
    merged_at: str
    additions: int = 0
    deletions: int = 0
    changed_files: int = 0
    touches_tests: bool = False
    src_additions: int = 0  # additions outside test paths (rough "task size")
    base_commit: str = ""
    merge_commit: str = ""


def _gh_json(args: list[str], retries: int = 3) -> object | None:
    """Run a ``gh`` command expected to emit JSON; None on failure, [] on empty.

    Retries transient GitHub server errors (503 / rate-limit / abuse) with a short
    backoff — the GraphQL API 503s intermittently under load, and without a retry
    those drop real candidates silently.
    """
    for attempt in range(retries):
        try:
            proc = subprocess.run(
                ["gh", *args],
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                check=False,
                timeout=90,
            )
        except (OSError, subprocess.TimeoutExpired) as e:
            print(f"  ! gh failed ({e})", file=sys.stderr)
            return None
        if proc.returncode == 0:
            if not proc.stdout.strip():
                return []
            try:
                return json.loads(proc.stdout)
            except json.JSONDecodeError:
                print("  ! could not parse gh json", file=sys.stderr)
                return None
        msg = proc.stderr.strip().splitlines()[-1] if proc.stderr.strip() else "(no stderr)"
        transient = any(t in msg for t in ("503", "502", "rate limit", "abuse", "secondary"))
        if transient and attempt < retries - 1:
            time.sleep(1.5 * (attempt + 1))
            continue
        print(f"  ! gh rc={proc.returncode}: {msg}", file=sys.stderr)
        return None
    return None


def _is_test_path(path: str) -> bool:
    p = path.lower()
    return any(m in p for m in _TEST_MARKERS)


def _search_repo(repo: str, since: str, per_repo: int) -> list[dict]:
    """Recent merged PRs for ``repo`` (number/title/url/closedAt), newest first."""
    rows = _gh_json(
        [
            "search",
            "prs",
            "--repo",
            repo,
            "--state",
            "closed",
            "--merged-at",
            f">={since}",
            "--limit",
            str(per_repo),
            "--json",
            "number,title,url,closedAt",
        ]
    )
    return rows if isinstance(rows, list) else []


def _enrich(repo: str, number: int) -> dict | None:
    """PR file list + commit oids to compute size, test-touch, and base commit."""
    data = _gh_json(
        [
            "pr",
            "view",
            str(number),
            "--repo",
            repo,
            "--json",
            "files,commits,mergeCommit,mergedAt,additions,deletions",
        ]
    )
    return data if isinstance(data, dict) else None


def mine(
    domains: list[str],
    since: str,
    per_repo: int,
    min_loc: int,
    max_loc: int,
    max_files: int,
    include_untested: bool,
) -> list[Candidate]:
    out: list[Candidate] = []
    for domain in domains:
        for repo in DOMAIN_REPOS[domain]:
            print(f"· {domain}: {repo}", file=sys.stderr)
            for row in _search_repo(repo, since, per_repo):
                num = row.get("number")
                if not isinstance(num, int):
                    continue
                info = _enrich(repo, num)
                if not info:
                    continue
                files = info.get("files") or []
                paths = [f.get("path", "") for f in files]
                touches_tests = any(_is_test_path(p) for p in paths)
                if not touches_tests and not include_untested:
                    continue
                src_add = sum(
                    int(f.get("additions", 0) or 0)
                    for f in files
                    if not _is_test_path(f.get("path", ""))
                )
                changed = len(files)
                if not (min_loc <= src_add <= max_loc) or changed > max_files:
                    continue
                commits = info.get("commits") or []
                first_oid = commits[0].get("oid", "") if commits else ""
                out.append(
                    Candidate(
                        domain=domain,
                        repo=repo,
                        number=num,
                        title=row.get("title", ""),
                        url=row.get("url", ""),
                        merged_at=(info.get("mergedAt") or row.get("closedAt") or "")[:10],
                        additions=int(info.get("additions", 0) or 0),
                        deletions=int(info.get("deletions", 0) or 0),
                        changed_files=changed,
                        touches_tests=touches_tests,
                        src_additions=src_add,
                        merge_commit=(info.get("mergeCommit") or {}).get("oid", "") or "",
                        base_commit=f"{first_oid}^" if first_oid else "",
                    )
                )
    return out


def _print_table(cands: list[Candidate]) -> None:
    print(f"\n{'domain':14} {'repo':26} {'PR':>6} {'merged':10} {'+src/-':>9} {'f':>3}  title")
    print("-" * 100)
    for c in sorted(cands, key=lambda x: (x.domain, x.repo, -x.number)):
        print(
            f"{c.domain:14} {c.repo:26} {c.number:>6} {c.merged_at:10} "
            f"{str(c.src_additions) + '/' + str(c.deletions):>9} {c.changed_files:>3}  "
            f"{c.title[:52]}"
        )
    print(f"\n{len(cands)} candidate(s). Measure each (repeat >= 3) before admitting.")


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="Mine candidate coding-task PRs (Python Suite v1)")
    p.add_argument(
        "--domain",
        action="append",
        choices=sorted(DOMAIN_REPOS),
        help="limit to one or more domains (default: all)",
    )
    p.add_argument("--since", default="2026-02-01", help="only PRs merged on/after this date")
    p.add_argument(
        "--per-repo", type=int, default=25, help="max recent merged PRs scanned per repo"
    )
    p.add_argument(
        "--min-loc", type=int, default=10, help="min net source additions (skip trivial)"
    )
    p.add_argument("--max-loc", type=int, default=400, help="max net source additions (skip huge)")
    p.add_argument("--max-files", type=int, default=20, help="max changed files")
    p.add_argument(
        "--include-untested", action="store_true", help="keep PRs that touch no test path"
    )
    p.add_argument("--json", help="write full candidate list to this JSON path")
    args = p.parse_args(argv)

    domains = args.domain or sorted(DOMAIN_REPOS)
    cands = mine(
        domains,
        args.since,
        args.per_repo,
        args.min_loc,
        args.max_loc,
        args.max_files,
        args.include_untested,
    )
    _print_table(cands)
    if args.json:
        with open(args.json, "w", encoding="utf-8") as fh:
            json.dump([asdict(c) for c in cands], fh, indent=2)
        print(f"wrote {args.json}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
