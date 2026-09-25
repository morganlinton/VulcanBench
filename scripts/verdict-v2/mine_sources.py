#!/usr/bin/env python3
"""Mine raw sources for the Verdict v2 mined families into a private cache.

Read-only: every call is a GET through the authenticated ``gh`` CLI
(``gh api`` REST and GraphQL). Nothing is created, commented, starred or
forked. Output goes to ``verdict-v2-items/mined/`` (gitignored) as JSONL,
appended as it goes, so an interrupted run resumes where it stopped.

Stages:

- ``fix-file``: per curated repo, merged PRs since the cutoff that close an
  issue and change exactly one non-test source file; stores the issue and
  the source tree (paths and sizes) at the merge's first parent.
- ``advisories``: GitHub reviewed advisories published since the cutoff for
  pip, go, npm and rust with a GitHub fix commit or PR; stores CWEs and the
  before/after contents of the most-changed source files of the fix.
- ``pool``: the VulcanCyber v1 task pool (upstream fix commits), same shape
  as advisories but without CWEs (vuln-pair only).
- ``semver``: merged PRs since the cutoff in curated Python libraries that
  touch package code; stores the patches and, for public modules, the full
  file contents at both commits so the builder's API differ can label them.

Usage::

    python scripts/verdict-v2/mine_sources.py --stage all --max-calls 4000
"""

from __future__ import annotations

import argparse
import calendar
import gzip
import json
import re
import subprocess
import sys
import time
from collections import defaultdict
from collections.abc import Iterable
from pathlib import Path
from typing import Any
from urllib.parse import quote

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from harness.verdict.v2.families.mined import (  # noqa: E402
    ADVISORY_CACHE,
    CUTOFF,
    FIX_FILE_CACHE,
    MINED_DIR,
    SEMVER_CACHE,
    is_public_module,
    python_module_parts,
    source_lang,
    tree_path,
)

MAIN_CHECKOUT = Path.home() / "dev" / "VulcanBench"
MAX_FILE_BYTES = 400_000
# Other sessions share the token's 5,000 an hour, so stop well before zero.
RATE_FLOOR = 300

FIX_FILE_REPOS = {
    "python": [
        "pallets/werkzeug",
        "pallets/flask",
        "pallets/click",
        "pallets/jinja",
        "encode/httpx",
        "encode/starlette",
        "fastapi/fastapi",
        "pydantic/pydantic",
        "python-attrs/attrs",
        "Textualize/rich",
        "Textualize/textual",
        "tqdm/tqdm",
        "pytest-dev/pytest",
        "HypothesisWorks/hypothesis",
        "psf/black",
        "python/mypy",
        "pypa/pip",
        "pypa/packaging",
        "urllib3/urllib3",
        "aio-libs/aiohttp",
        "sqlalchemy/sqlalchemy",
        "marshmallow-code/marshmallow",
        "python-jsonschema/jsonschema",
        "tox-dev/tox",
        "pypa/virtualenv",
        "pypa/setuptools",
        "pypa/hatch",
        "python-poetry/poetry",
        "sphinx-doc/sphinx",
        "pylint-dev/pylint",
        "networkx/networkx",
        "celery/celery",
        "redis/redis-py",
        "streamlit/streamlit",
        "huggingface/huggingface_hub",
        "fsspec/filesystem_spec",
        "dask/dask",
        "pydata/xarray",
        "python-pillow/Pillow",
        "Delgan/loguru",
        "hynek/structlog",
        "joke2k/faker",
        "getsentry/sentry-python",
        "Kludex/uvicorn",
        "fastapi/typer",
        "ipython/ipython",
        "conda/conda",
        "jd/tenacity",
        "python-websockets/websockets",
        "agronholm/anyio",
        "pyca/cryptography",
        "yaml/pyyaml",
    ],
    "go": [
        "gin-gonic/gin",
        "labstack/echo",
        "gofiber/fiber",
        "go-chi/chi",
        "spf13/cobra",
        "spf13/viper",
        "prometheus/prometheus",
        "helm/helm",
        "traefik/traefik",
        "caddyserver/caddy",
        "gohugoio/hugo",
        "cli/cli",
        "go-gitea/gitea",
        "junegunn/fzf",
        "jesseduffield/lazygit",
        "charmbracelet/bubbletea",
        "etcd-io/etcd",
        "minio/minio",
        "open-telemetry/opentelemetry-go",
        "golangci/golangci-lint",
        "goreleaser/goreleaser",
        "rclone/rclone",
        "syncthing/syncthing",
        "ollama/ollama",
        "stretchr/testify",
        "uber-go/zap",
        "jackc/pgx",
        "go-gorm/gorm",
        "derailed/k9s",
        "hashicorp/terraform",
        "containerd/containerd",
        "grpc/grpc-go",
        "nats-io/nats-server",
        "cockroachdb/pebble",
    ],
    "rust": [
        "tokio-rs/tokio",
        "tokio-rs/axum",
        "hyperium/hyper",
        "clap-rs/clap",
        "BurntSushi/ripgrep",
        "sharkdp/fd",
        "sharkdp/bat",
        "astral-sh/ruff",
        "astral-sh/uv",
        "rust-lang/cargo",
        "nushell/nushell",
        "alacritty/alacritty",
        "helix-editor/helix",
        "starship/starship",
        "rustls/rustls",
        "tauri-apps/tauri",
        "meilisearch/meilisearch",
        "typst/typst",
        "GitoxideLabs/gitoxide",
        "swc-project/swc",
        "biomejs/biome",
        "oxc-project/oxc",
        "serde-rs/serde",
        "rust-lang/rust-clippy",
        "denoland/deno",
        "wasmerio/wasmer",
        "zellij-org/zellij",
        "sxyazi/yazi",
    ],
    "js": [
        "vitejs/vite",
        "vuejs/core",
        "sveltejs/svelte",
        "expressjs/express",
        "fastify/fastify",
        "honojs/hono",
        "colinhacks/zod",
        "prettier/prettier",
        "eslint/eslint",
        "webpack/webpack",
        "rollup/rollup",
        "vitest-dev/vitest",
        "jestjs/jest",
        "axios/axios",
        "nodejs/undici",
        "remix-run/react-router",
        "TanStack/query",
        "trpc/trpc",
        "typeorm/typeorm",
        "withastro/astro",
        "nuxt/nuxt",
        "date-fns/date-fns",
        "chartjs/Chart.js",
        "mermaid-js/mermaid",
        "pmndrs/zustand",
        "Automattic/mongoose",
        "knex/knex",
        "socketio/socket.io",
        "markedjs/marked",
        "typescript-eslint/typescript-eslint",
        "nestjs/nest",
        "microsoft/playwright",
        "puppeteer/puppeteer",
        "excalidraw/excalidraw",
        "sequelize/sequelize",
        "storybookjs/storybook",
        "denoland/std",
        "unjs/h3",
        "unjs/ofetch",
    ],
}

SEMVER_REPOS = [
    "pallets/werkzeug",
    "pallets/flask",
    "pallets/click",
    "pallets/jinja",
    "pallets/itsdangerous",
    "pallets/markupsafe",
    "encode/httpx",
    "encode/starlette",
    "fastapi/fastapi",
    "pydantic/pydantic",
    "python-attrs/attrs",
    "python-attrs/cattrs",
    "Textualize/rich",
    "Textualize/textual",
    "tqdm/tqdm",
    "pytest-dev/pytest",
    "pytest-dev/pluggy",
    "HypothesisWorks/hypothesis",
    "pypa/packaging",
    "urllib3/urllib3",
    "aio-libs/aiohttp",
    "sqlalchemy/sqlalchemy",
    "sqlalchemy/alembic",
    "marshmallow-code/marshmallow",
    "python-jsonschema/jsonschema",
    "networkx/networkx",
    "celery/celery",
    "celery/kombu",
    "redis/redis-py",
    "huggingface/huggingface_hub",
    "fsspec/filesystem_spec",
    "dask/dask",
    "pydata/xarray",
    "python-pillow/Pillow",
    "hynek/structlog",
    "joke2k/faker",
    "getsentry/sentry-python",
    "fastapi/typer",
    "jd/tenacity",
    "python-websockets/websockets",
    "agronholm/anyio",
    "more-itertools/more-itertools",
    "tkem/cachetools",
    "tox-dev/filelock",
    "tox-dev/platformdirs",
    "python-poetry/tomlkit",
    "executablebooks/markdown-it-py",
    "pygments/pygments",
    "boto/boto3",
    "psf/requests",
    "arrow-py/arrow",
    "dateutil/dateutil",
    "sympy/sympy",
    "scikit-learn/scikit-learn",
    "pandas-dev/pandas",
    "langchain-ai/langchain",
    "BerriAI/litellm",
    "huggingface/datasets",
    "Kludex/uvicorn",
    "python-trio/trio",
    "sphinx-doc/sphinx",
    "ipython/ipython",
    "pypa/setuptools",
    "python-jsonschema/referencing",
    "mahmoud/boltons",
    "pytoolz/toolz",
    "wntrblm/nox",
    "python/typing_extensions",
    "psf/black",
    "PyCQA/isort",
    "jazzband/pip-tools",
    "encode/httpcore",
    "pydantic/pydantic-settings",
    "tiangolo/sqlmodel",
    "falconry/falcon",
    "bottlepy/bottle",
    "Pylons/pyramid",
    "tornadoweb/tornado",
    "django/django",
    "jazzband/django-debug-toolbar",
    "encode/django-rest-framework",
    "psycopg/psycopg",
    "MagicStack/asyncpg",
    "aio-libs/yarl",
    "aio-libs/multidict",
    "aio-libs/frozenlist",
    "aio-libs/aiobotocore",
    "pyvista/pyvista",
    "Qiskit/qiskit",
    "PennyLaneAI/pennylane",
    "optuna/optuna",
    "mlflow/mlflow",
    "kedro-org/kedro",
    "pola-rs/polars",
    "apache/arrow-nanoarrow",
    "ibis-project/ibis",
    "narwhals-dev/narwhals",
    "zarr-developers/zarr-python",
    "h5py/h5py",
    "astropy/astropy",
    "scipy/scipy",
    "numpy/numpy",
    "matplotlib/matplotlib",
    "bokeh/bokeh",
    "plotly/plotly.py",
    "altair-viz/altair",
    "pallets-eco/flask-sqlalchemy",
    "pallets-eco/blinker",
    "psf/cachecontrol",
    "pyparsing/pyparsing",
    "sqlalchemy/mako",
    "benoitc/gunicorn",
    "gevent/gevent",
    "pyca/pyopenssl",
    "paramiko/paramiko",
    "pyinvoke/invoke",
    "boto/botocore",
    "googleapis/google-auth-library-python",
    "PyCQA/bandit",
    "PyCQA/pyflakes",
    "PyCQA/flake8",
    "pylint-dev/astroid",
    "python-lsp/python-lsp-server",
    "davidhalter/jedi",
    "davidhalter/parso",
    "jupyter/jupyter_client",
    "jupyter/nbformat",
    "jupyter/nbconvert",
    "ipython/traitlets",
    "scrapy/scrapy",
    "scrapy/parsel",
    "python-telegram-bot/python-telegram-bot",
    "Rapptz/discord.py",
    "slackapi/python-slack-sdk",
    "coleifer/peewee",
    "tortoise/tortoise-orm",
    "mongodb/mongo-python-driver",
    "elastic/elasticsearch-py",
    "PrefectHQ/prefect",
    "dagster-io/dagster",
    "huggingface/peft",
    "huggingface/accelerate",
    "huggingface/trl",
    "openai/openai-agents-python",
    "pydantic/pydantic-ai",
    "pydantic/logfire",
    "instructor-ai/instructor",
    "run-llama/llama_index",
    "ewels/rich-click",
    "pexpect/pexpect",
    "giampaolo/psutil",
    "marshmallow-code/webargs",
    "hynek/stamina",
    "jaraco/keyring",
    "pypa/twine",
    "pypa/build",
    "pypa/pipx",
    "pdm-project/pdm",
    "shapely/shapely",
    "geopandas/geopandas",
    "pyproj4/pyproj",
    "statsmodels/statsmodels",
    "pymc-devs/pymc",
    "arviz-devs/arviz",
    "mkdocs/mkdocs",
    "mkdocstrings/mkdocstrings",
    "mkdocstrings/griffe",
    "litestar-org/litestar",
    "sanic-org/sanic",
    "wagtail/wagtail",
    "keras-team/keras",
    "pytorch/vision",
]

ECOSYSTEMS = ("pip", "go", "npm", "rust")
LINK = re.compile(
    r"https://github\.com/([\w.-]+)/([\w.-]+)/(commit|commits|pull)/([0-9a-f]{7,40}|\d+)"
)
PR_COMMIT = re.compile(r"https://github\.com/([\w.-]+)/([\w.-]+)/pull/\d+/commits/([0-9a-f]{40})")


class Budget(Exception):
    pass


class GH:
    """Read-only ``gh api`` wrapper with rate-limit checks and backoff."""

    def __init__(self, max_calls: int, log: Path) -> None:
        self.max_calls = max_calls
        self.calls = 0
        self.log = log
        self.rate_limit_waits = 0
        self.errors: dict[str, int] = defaultdict(int)

    def note(self, message: str) -> None:
        line = f"{time.strftime('%H:%M:%S')} {message}"
        print(line, flush=True)
        with self.log.open("a") as fh:
            fh.write(line + "\n")

    def _limits(self, graphql: bool) -> tuple[int, int] | None:
        """(remaining, reset epoch) from live response headers.

        ``gh api rate_limit`` can report a different pool from the one the calls
        draw on, so the numbers come from a real request's headers instead.
        """
        if graphql:
            out = subprocess.run(
                ["gh", "api", "graphql", "-f", "query={ rateLimit { remaining resetAt } }"],
                capture_output=True,
                text=True,
                check=False,
            )
            m = re.search(r'"remaining":(\d+),"resetAt":"([^"]+)"', out.stdout)
            if not m:
                return None
            reset_at = calendar.timegm(time.strptime(m.group(2), "%Y-%m-%dT%H:%M:%SZ"))
            return int(m.group(1)), int(reset_at)
        out = subprocess.run(
            ["gh", "api", "-i", "repos/cli/cli", "--jq", ".id"],
            capture_output=True,
            text=True,
            check=False,
        )
        text = out.stdout + out.stderr
        left = re.search(r"(?i)x-ratelimit-remaining:\s*(\d+)", text)
        reset = re.search(r"(?i)x-ratelimit-reset:\s*(\d+)", text)
        if not left or not reset:
            return None
        return int(left.group(1)), int(reset.group(1))

    def _wait_for_reset(self, graphql: bool, force: bool) -> None:
        limits = self._limits(graphql)
        if limits is None:
            if force:
                time.sleep(120)
            return
        left, reset = limits
        if force or left < RATE_FLOOR:
            wait = max(10, reset - int(time.time()) + 10)
            self.rate_limit_waits += 1
            kind = "graphql" if graphql else "core"
            self.note(f"rate limit {kind} remaining {left}: sleeping {wait}s until reset")
            time.sleep(wait)

    def _run(self, args: list[str]) -> str | None:
        if self.calls >= self.max_calls:
            raise Budget
        self.calls += 1
        graphql = args[0] == "graphql"
        if self.calls % 50 == 0:
            self._wait_for_reset(graphql, force=False)
        delay = 30.0
        attempts = 0
        while attempts < 6:
            out = subprocess.run(["gh", "api", *args], capture_output=True, text=True, check=False)
            if out.returncode == 0:
                time.sleep(0.1)
                return out.stdout
            err = out.stderr + out.stdout[:500]
            if re.search(r"HTTP (404|409|410|422|451)|Not Found|No commit found", err):
                self.errors["not-found"] += 1
                return None
            if re.search(r"API rate limit exceeded", err, re.I):
                # Primary limit: wait for the reset; this does not use up an attempt.
                self.errors["rate-limit"] += 1
                self._wait_for_reset(graphql, force=True)
                continue
            if re.search(r"rate limit|HTTP 403|HTTP 429|HTTP 5\d\d|timeout|EOF", err, re.I):
                attempts += 1
                self.rate_limit_waits += 1
                self.errors["backoff"] += 1
                self.note(f"backoff {delay:.0f}s: {err.strip()[:160]}")
                time.sleep(delay)
                delay = min(delay * 2, 900)
                continue
            self.errors["other"] += 1
            self.note(f"gh error: {err.strip()[:200]}")
            return None
        raise Budget  # persistent failures: stop rather than record false skips

    def json(self, path: str, *extra: str) -> Any:
        out = self._run([path, *extra])
        return None if out is None else json.loads(out) if out.strip() else None

    def raw(self, repo: str, path: str, ref: str) -> str | None:
        out = self._run(
            [
                "-H",
                "Accept: application/vnd.github.raw+json",
                f"repos/{repo}/contents/{quote(path)}?ref={ref}",
            ]
        )
        if out is None or len(out) > MAX_FILE_BYTES:
            return None
        return out

    def graphql(self, query: str, **variables: str) -> Any:
        args = ["graphql", "-f", f"query={query}"]
        for key, value in variables.items():
            args += ["-f", f"{key}={value}"]
        out = self._run(args)
        return None if out is None else json.loads(out)


def append(path: Path, record: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(record, sort_keys=True) + "\n")


def _records(path: Path) -> list[dict[str, Any]]:
    out = []
    if path.exists():
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return out


def load_keys(path: Path, field: str) -> set[str]:
    keys = set()
    if path.exists():
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            try:
                keys.add(str(json.loads(line)[field]))
            except (json.JSONDecodeError, KeyError):
                continue
    return keys


PR_SEARCH = """
query($q: String!, $after: String) {
  search(query: $q, type: ISSUE, first: 50, after: $after) {
    pageInfo { hasNextPage endCursor }
    nodes {
      ... on PullRequest {
        number title mergedAt changedFiles
        labels(first: 10) { nodes { name } }
        mergeCommit { oid parents(first: 2) { nodes { oid } } }
        files(first: 100) { nodes { path additions deletions changeType } }
        closingIssuesReferences(first: 3) {
          nodes { number title body labels(first: 10) { nodes { name } } }
        }
      }
    }
  }
}
"""
BUGGY = re.compile(r"(?i)\b(fix|fixe[sd]|bug|regression|crash|broken|incorrect|wrong|error|fail)")
API_TITLE = re.compile(
    r"(?i)remov|drop|deprecat|renam|breaking|delet|replace|refactor|clean|signature"
    r"|keyword|positional|\badd\b|new |support|expose|public|api"
)
NOT_BUG = re.compile(r"(?i)^(feat|docs?|chore|refactor|perf|build|ci|deps|style)\b|deprecat")


def search_prs(gh: GH, query: str, pages: int) -> list[dict[str, Any]]:
    nodes: list[dict[str, Any]] = []
    after: str | None = None
    for _ in range(pages):
        variables = {"q": query}
        if after:
            variables["after"] = after
        data = gh.graphql(PR_SEARCH, **variables)
        if not data or "data" not in data or data["data"] is None:
            break
        search = data["data"]["search"]
        nodes.extend(n for n in search["nodes"] if n)
        if not search["pageInfo"]["hasNextPage"]:
            break
        after = search["pageInfo"]["endCursor"]
    return nodes


def fetch_tree(gh: GH, root: Path, repo: str, sha: str) -> bool:
    path = tree_path(root, repo, sha)
    if path.exists():
        return True
    data = gh.json(
        f"repos/{repo}/git/trees/{sha}?recursive=1",
        "--jq",
        '{truncated, files: [.tree[] | select(.type == "blob") | [.path, .size]]}',
    )
    if not data:
        return False
    files = [[p, s] for p, s in data["files"] if source_lang(p)]
    path.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(path, "wt", encoding="utf-8") as fh:
        json.dump({"repo": repo, "sha": sha, "truncated": data["truncated"], "files": files}, fh)
    return not data["truncated"]


def mine_fix_file(gh: GH, root: Path, per_repo: int, pages: int, revisit: bool) -> None:
    out = root / FIX_FILE_CACHE
    done = set() if revisit else load_keys(root / "fix_file_repos.jsonl", "repo")
    have = {f"{r['repo']}#{r['number']}" for r in _records(out)}
    repos = [(lang, r) for lang, rs in FIX_FILE_REPOS.items() for r in rs]
    # Interleave languages so a partial run still covers all four.
    repos.sort(key=lambda lr: (FIX_FILE_REPOS[lr[0]].index(lr[1]), lr[0]))
    for _lang, repo in repos:
        if repo in done:
            continue
        query = f"repo:{repo} is:pr is:merged merged:>={CUTOFF} linked:issue"
        prs = search_prs(gh, query, pages)
        kept = 0
        for pr in prs:
            if kept >= per_repo:
                break
            if f"{repo}#{pr['number']}" in have:
                kept += 1
                continue
            issues = pr.get("closingIssuesReferences", {}).get("nodes", [])
            if len(issues) != 1 or not pr.get("mergeCommit"):
                continue
            issue = issues[0]
            labels = [n["name"] for n in pr["labels"]["nodes"]] + [
                n["name"] for n in issue["labels"]["nodes"]
            ]
            text = " ".join([pr["title"], issue["title"], *labels])
            if not BUGGY.search(text) or NOT_BUG.search(pr["title"]):
                continue
            files = pr["files"]["nodes"]
            if pr["changedFiles"] > 100:
                continue
            src = [f for f in files if source_lang(f["path"])]
            if len(src) != 1 or src[0]["changeType"] != "MODIFIED":
                continue
            parents = pr["mergeCommit"]["parents"]["nodes"]
            if not parents:
                continue
            base = parents[0]["oid"]
            if not fetch_tree(gh, root, repo, base):
                continue
            append(
                out,
                {
                    "status": "ok",
                    "repo": repo,
                    "number": pr["number"],
                    "pr_title": pr["title"],
                    "merged_at": pr["mergedAt"],
                    "merge_sha": pr["mergeCommit"]["oid"],
                    "base_sha": base,
                    "fixed_file": src[0]["path"],
                    "other_files": [f["path"] for f in files if f is not src[0]],
                    "issue_number": issue["number"],
                    "issue_title": issue["title"],
                    "issue_body": (issue["body"] or "")[:20000],
                    "labels": labels,
                },
            )
            kept += 1
        append(root / "fix_file_repos.jsonl", {"repo": repo, "prs": len(prs), "kept": kept})
        gh.note(f"fix-file {repo}: {len(prs)} PRs searched, {kept} kept (calls {gh.calls})")


def _source_files(files: Iterable[dict[str, Any]]) -> list[dict[str, Any]]:
    src = [
        f
        for f in files
        if source_lang(f["filename"]) and f.get("status") == "modified" and f.get("patch")
    ]
    return sorted(src, key=lambda f: -(f["additions"] + f["deletions"]))


def fetch_fix(gh: GH, repo: str, ref: str, kind: str, max_files: int) -> dict[str, Any] | None:
    """Resolve a commit or PR to its fix commit and fetch the changed source files."""
    sha = ref
    if kind == "pull":
        pr = gh.json(f"repos/{repo}/pulls/{ref}", "--jq", "{merged_at, merge_commit_sha}")
        if not pr or not pr.get("merged_at") or not pr.get("merge_commit_sha"):
            return None
        sha = pr["merge_commit_sha"]
    commit = gh.json(
        f"repos/{repo}/commits/{sha}",
        "--jq",
        "{sha, date: .commit.committer.date, parents: [.parents[].sha], files: [.files[]? | "
        "{filename, status, additions, deletions, patch}]}",
    )
    if not commit or not commit["parents"]:
        return None
    parent = commit["parents"][0]
    entries = []
    for f in _source_files(commit["files"])[:max_files]:
        before = gh.raw(repo, f["filename"], parent)
        after = gh.raw(repo, f["filename"], commit["sha"])
        if before is None or after is None:
            continue
        entries.append({"path": f["filename"], "before": before, "after": after})
    if not entries:
        return None
    return {"fix_commit": commit["sha"], "commit_date": commit["date"], "files": entries}


def fix_refs(advisory: dict[str, Any]) -> list[tuple[str, str, str]]:
    refs: list[tuple[str, str, str]] = []
    for url in advisory.get("references") or []:
        m = PR_COMMIT.match(url)
        if m:
            refs.append((f"{m.group(1)}/{m.group(2)}", "commit", m.group(3)))
            continue
        m = LINK.match(url)
        if m:
            kind = "pull" if m.group(3) == "pull" else "commit"
            refs.append((f"{m.group(1)}/{m.group(2)}", kind, m.group(4)))
    source = (advisory.get("source_code_location") or "").lower().rstrip("/")
    # Prefer commits in the advisory's own repository, then commits, then PRs.
    refs.sort(key=lambda r: (not source.endswith(r[0].lower()), r[1] != "commit"))
    return refs


def mine_advisories(gh: GH, root: Path, per_repo: int, max_files: int) -> None:  # noqa: PLR0912
    out = root / ADVISORY_CACHE
    done = load_keys(out, "id")
    listing_path = root / "advisory_listing.jsonl"
    if not listing_path.exists():
        for eco in ECOSYSTEMS:
            raw = gh._run(
                [
                    f"/advisories?type=reviewed&ecosystem={eco}&published={CUTOFF}..2026-12-31"
                    "&per_page=100",
                    "--paginate",
                    "--jq",
                    ".[] | {ghsa_id, cve_id, published_at, cwes: [.cwes[]?.cwe_id], references, "
                    "source_code_location, packages: [.vulnerabilities[]?.package.name]}",
                ]
            )
            for line in (raw or "").splitlines():
                if line.strip():
                    append(listing_path, {**json.loads(line), "ecosystem": eco})
    listing = [json.loads(x) for x in listing_path.read_text().splitlines() if x.strip()]
    by_repo: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for adv in listing:
        refs = fix_refs(adv)
        if refs:
            by_repo[refs[0][0].lower()].append(adv)
    order: list[dict[str, Any]] = []
    depth = 0
    repos = sorted(by_repo)
    while any(len(by_repo[r]) > depth for r in repos) and depth < per_repo:
        for r in repos:
            if len(by_repo[r]) > depth:
                order.append(by_repo[r][depth])
        depth += 1
    seen_commits: dict[str, dict[str, Any]] = {}
    for adv in order:
        if adv["ghsa_id"] in done:
            continue
        repo, kind, ref = fix_refs(adv)[0]
        base = {
            "id": adv["ghsa_id"],
            "cve": adv.get("cve_id"),
            "published_at": adv["published_at"],
            "ecosystem": adv["ecosystem"],
            "packages": adv.get("packages", []),
            "cwes": adv.get("cwes", []),
            "repo": repo,
        }
        cache_key = f"{repo}:{kind}:{ref}"
        fix = seen_commits.get(cache_key)
        if fix is None:
            fix = fetch_fix(gh, repo, ref, kind, max_files) or {}
            seen_commits[cache_key] = fix
        if not fix:
            append(out, {**base, "status": "skip:no-fix-files"})
            continue
        append(out, {**base, "status": "ok", **fix})
        gh.note(f"advisory {adv['ghsa_id']} {repo} ok (calls {gh.calls})")


def mine_pool(gh: GH, root: Path, max_files: int) -> None:
    out = root / ADVISORY_CACHE
    done = load_keys(out, "id")
    for meta_path in sorted((MAIN_CHECKOUT / "tasks" / "vulcancyber-v1").glob("*/metadata.json")):
        meta = json.loads(meta_path.read_text())
        up = meta.get("upstream") or {}
        task_id = f"pool:{meta.get('id')}"
        if task_id in done or not up.get("commit") or not up.get("url"):
            continue
        m = re.match(r"https://github\.com/([\w.-]+/[\w.-]+)", up["url"])
        merged = str(meta.get("upstream_merged") or "")
        if not m or merged < CUTOFF:
            continue
        repo = m.group(1)
        fix = fetch_fix(gh, repo, up["commit"], "commit", max_files)
        base: dict[str, Any] = {
            "id": task_id,
            "published_at": None,
            "merged_at": merged,
            "cwes": [],
            "repo": repo,
            "ecosystem": "pool",
            "packages": [],
        }
        append(
            out, {**base, "status": "ok", **fix} if fix else {**base, "status": "skip:no-fix-files"}
        )
        gh.note(f"pool {task_id} {'ok' if fix else 'skipped'}")


def mine_semver(gh: GH, root: Path, per_repo: int, pages: int, revisit: bool) -> None:
    out = root / SEMVER_CACHE
    done = set() if revisit else load_keys(root / "semver_repos.jsonl", "repo")
    have = {f"{r['repo']}#{r['number']}" for r in _records(out)}
    for repo in SEMVER_REPOS:
        if repo in done:
            continue
        prs = search_prs(gh, f"repo:{repo} is:pr is:merged merged:>={CUTOFF}", pages)
        candidates = []
        for pr in prs:
            if not pr.get("mergeCommit") or pr["changedFiles"] > 60:
                continue
            files = pr["files"]["nodes"]
            pkg = [f for f in files if python_module_parts(f["path"])]
            churn = sum(f["additions"] + f["deletions"] for f in pkg)
            if not 1 <= len(pkg) <= 8 or churn > 400:
                continue
            public = any(is_public_module(f["path"]) for f in pkg)
            quiet = not API_TITLE.search(pr["title"])
            candidates.append((not public, quiet, pr["number"], pr, pkg))
        # Public-module PRs first (private-only changes are patch by construction),
        # then titles that suggest an API change, because major bumps are rare.
        candidates.sort(key=lambda c: (c[0], c[1], -c[2]))
        kept = 0
        private_kept = 0
        for private_only, _quiet, _n, pr, pkg in candidates:
            if kept >= per_repo:
                break
            if private_only and private_kept >= max(2, per_repo // 5):
                continue
            if f"{repo}#{pr['number']}" in have:
                kept += 1
                private_kept += private_only
                continue
            record = semver_record(gh, repo, pr, pkg)
            if record is None:
                continue
            append(out, record)
            kept += 1
            private_kept += private_only
        append(root / "semver_repos.jsonl", {"repo": repo, "prs": len(prs), "kept": kept})
        gh.note(
            f"semver {repo}: {len(prs)} PRs, {len(candidates)} candidates, {kept} kept "
            f"(calls {gh.calls})"
        )


def semver_record(
    gh: GH, repo: str, pr: dict[str, Any], pkg: list[dict[str, Any]]
) -> dict[str, Any] | None:
    merge = pr["mergeCommit"]["oid"]
    parents = pr["mergeCommit"]["parents"]["nodes"]
    if not parents:
        return None
    before = parents[0]["oid"]
    compare = gh.json(
        f"repos/{repo}/compare/{before}...{merge}",
        "--jq",
        "[.files[]? | {filename, status, additions, deletions, patch, previous_filename}]",
    )
    if not compare:
        return None
    changed = {f["filename"]: f for f in compare if python_module_parts(f["filename"])}
    expected = {f["path"] for f in pkg}
    if set(changed) != expected:
        return None  # a rebase merge: the merge commit alone is not the whole PR
    files, contents = [], {}
    for path in sorted(changed):
        f = changed[path]
        entry = {
            "path": path,
            "status": f["status"],
            "patch": f.get("patch"),
            "previous_path": f.get("previous_filename"),
        }
        files.append(entry)
        for p in {path, f.get("previous_filename") or path}:
            if not is_public_module(p):
                continue
            got: dict[str, str | None] = {"before": None, "after": None}
            if f["status"] != "added":
                got["before"] = gh.raw(repo, f.get("previous_filename") or p, before)
                if got["before"] is None:
                    return None
            if f["status"] != "removed":
                got["after"] = gh.raw(repo, path, merge)
                if got["after"] is None:
                    return None
            contents[p] = got
    return {
        "status": "ok",
        "repo": repo,
        "number": pr["number"],
        "title": pr["title"],
        "merged_at": pr["mergedAt"],
        "before_sha": before,
        "after_sha": merge,
        "files": files,
        "contents": contents,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--stage", choices=["fix-file", "advisories", "pool", "semver", "all"], default="all"
    )
    parser.add_argument("--out", type=Path, default=REPO / MINED_DIR)
    parser.add_argument("--max-calls", type=int, default=4000)
    parser.add_argument("--per-repo", type=int, default=10)
    parser.add_argument("--pages", type=int, default=4, help="GraphQL search pages of 50 PRs")
    parser.add_argument("--max-files", type=int, default=2, help="source files per fix")
    parser.add_argument(
        "--revisit",
        action="store_true",
        help="search finished repos again (with a larger --per-repo); known PRs are skipped",
    )
    args = parser.parse_args()
    root: Path = args.out
    root.mkdir(parents=True, exist_ok=True)
    gh = GH(args.max_calls, root / f"mine-{args.stage}.log")
    stages = ["pool", "advisories", "fix-file", "semver"] if args.stage == "all" else [args.stage]
    try:
        for stage in stages:
            gh.note(f"stage {stage} start (calls {gh.calls})")
            if stage == "fix-file":
                mine_fix_file(gh, root, args.per_repo, args.pages, args.revisit)
            elif stage == "advisories":
                mine_advisories(gh, root, args.per_repo, args.max_files)
            elif stage == "pool":
                mine_pool(gh, root, args.max_files)
            else:
                mine_semver(gh, root, args.per_repo, args.pages, args.revisit)
    except Budget:
        gh.note("call budget reached; rerun to resume")
    gh.note(f"done: {gh.calls} calls, {gh.rate_limit_waits} waits, errors {dict(gh.errors)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
