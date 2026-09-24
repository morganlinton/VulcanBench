"""Runtime integrity audit for CLI-harness runs: web and filesystem.

VulcanBench's own loop has no web tools and runs its sandbox network-off, but
external harnesses (Cursor, and Claude Code / Codex when ``--network`` is set)
can browse. For a suite built from real merged PRs that is a solved-answer
oracle: the fix exists at a known public URL. Terminal-Bench, which also allows
internet, tells benchmark users to "remain vigilant" about agents locating
solutions; this module is that vigilance, automated.

Every v3 task records its provenance in ``metadata.upstream`` (repo URL, PR,
fix commit). The audit parses a run's captured CLI event stream, extracts every
web search and fetch, and matches them against that provenance. Verdicts
escalate:

- ``no_web``: the stream contains no web tool use.
- ``web_blocked``: the agent attempted web calls and every one was rejected by
  the harness's deny rules. No data reached the model, so this is clean --
  but it is recorded, because an agent reaching for the web on a
  decontaminated task is worth seeing.
- ``web_used``: browsed, but never touched the task's upstream project.
- ``upstream_access``: fetched content from the task's upstream repository.
  Post-fix sources (e.g. the repaired file on ``main``) contain the solution,
  so this is contamination even without the PR itself.
- ``solution_retrieval``: fetched the exact source PR, the fix commit, or a
  raw diff/patch of the upstream repo.

Blocking the web is only half the problem. External harnesses execute on the
host, so an agent whose workspace sits inside the VulcanBench checkout can walk
up the tree and read ``tasks/<suite>/<id>/gold_patch.diff`` -- the grader's
answer key -- and the hidden tests it is about to be scored against. Observed:
46 runs of a supposedly clean sweep did exactly that, and all 46 solved. Runs
are now given a workspace outside the repo, and the filesystem half of this
audit is the check on that containment:

- ``clean``: every path the agent touched is inside its workspace.
- ``out_of_workspace``: read something outside the workspace that is not
  benchmark data (a system file, a global config). Recorded, not fatal.
- ``benchmark_data_access``: touched VulcanBench's own task tree or another
  run directory. Contaminating regardless of which task it belonged to.
- ``answer_key_access``: touched this run's own gold patch, hidden tests, or
  task metadata. The strongest possible contamination signal.

The audit annotates; it never rescores. Reports decide what a contaminated
run is worth.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

# Cursor/Claude Code camel-case forms plus Grok Build's snake-case tool
# titles as they appear in trace updates ("title":"web_search") and Devin's
# lower-case "webfetch".
_WEB_MARKERS = (
    "webSearchToolCall",
    "webFetchToolCall",
    '"WebSearch"',
    '"WebFetch"',
    '"web_search"',
    '"web_fetch"',
    '"webfetch"',
)
_URL_RE = re.compile(r"https?://[^\s\"'\\]+")

VERDICTS = ("no_web", "web_blocked", "web_used", "upstream_access", "solution_retrieval")


def upstream_refs(metadata: dict[str, Any]) -> dict[str, str]:
    """Extract the provenance identifiers the tripwire matches against."""
    up = metadata.get("upstream") or {}
    url = str(up.get("url") or "")
    repo_m = re.search(r"github\.com/([\w.-]+/[\w.-]+)", url)
    pr_m = re.search(r"/pull/(\d+)", url)
    return {
        "repo": repo_m.group(1).lower() if repo_m else "",
        "pr": pr_m.group(1) if pr_m else "",
        "commit": str(up.get("commit") or "")[:12].lower(),
    }


def _line_hits(line: str, refs: dict[str, str]) -> tuple[bool, bool]:
    """(upstream_access, solution_retrieval) signals for one stream line."""
    low = line.lower()
    repo = refs["repo"]
    repo_hit = bool(repo) and repo in low
    solution = False
    if repo_hit:
        if refs["pr"] and re.search(rf"/pull/{refs['pr']}\b", line):
            solution = True
        if refs["commit"] and refs["commit"] in low:
            solution = True
        if ".diff" in low or ".patch" in low or "patch-diff" in low:
            solution = True
    return repo_hit, solution


_CALL_ID_RE = re.compile(r'"(?:toolCallId|call_id|callID|callId)"\s*:\s*"((?:[^"\\]|\\.)*)"')


def _call_id(line: str) -> str:
    m = _CALL_ID_RE.search(line)
    return m.group(1) if m else ""


def _web_calls(stream_path: Path) -> dict[str, dict[str, Any]]:
    """Collect web tool calls keyed by call id, marking rejected ones.

    A call is emitted twice: ``started`` carries the url/query, ``completed``
    carries the result. Counting the ``started`` event alone scores a call the
    harness went on to REJECT as real access -- which turned every blocked
    attempt into a false contamination flag.
    """
    calls: dict[str, dict[str, Any]] = {}
    if not stream_path.exists():
        return calls
    with stream_path.open(encoding="utf-8", errors="replace") as f:
        for line in f:
            if not any(m in line for m in _WEB_MARKERS):
                continue
            cid = _call_id(line) or f"anon-{len(calls)}"
            call = calls.setdefault(cid, {"urls": set(), "searches": 0, "rejected": False})
            if '"rejected"' in line:
                call["rejected"] = True
            if "webSearchToolCall" in line or '"WebSearch"' in line:
                call["searches"] = max(call["searches"], line.count("searchTerm"), 1)
            for url in _URL_RE.findall(line):
                call["urls"].add(url)
    return calls


def audit_stream(stream_path: Path, metadata: dict[str, Any]) -> dict[str, Any]:
    """Audit one run's CLI event stream. Missing stream -> trivially no_web."""
    refs = upstream_refs(metadata)
    searches = fetches = blocked = 0
    hosts: set[str] = set()
    upstream_access = solution = False

    for call in _web_calls(stream_path).values():
        if call["rejected"]:
            blocked += 1
            continue
        if call["searches"]:
            searches += call["searches"]
        if call["urls"]:
            fetches += 1
        for url in call["urls"]:
            host = re.sub(r"^https?://", "", url).split("/", 1)[0]
            if host:
                hosts.add(host.lower())
            repo_hit, sol = _line_hits(url, refs)
            upstream_access = upstream_access or repo_hit
            solution = solution or sol

    if solution:
        verdict = "solution_retrieval"
    elif upstream_access:
        verdict = "upstream_access"
    elif searches or fetches:
        verdict = "web_used"
    elif blocked:
        verdict = "web_blocked"
    else:
        verdict = "no_web"
    return {
        "verdict": verdict,
        "web_searches": searches,
        "web_fetches": fetches,
        "web_blocked": blocked,
        "hosts": sorted(hosts)[:40],
        "upstream": refs,
        "contaminated": verdict in ("upstream_access", "solution_retrieval"),
    }


_PATH_RE = re.compile(
    r'"(?:path|file_path|filePath|target_file|target_directory)"\s*:\s*"((?:[^"\\]|\\.)*)"'
)
_CMD_RE = re.compile(r'"command"\s*:\s*"((?:[^"\\]|\\.)*)"')
_ANSWER_KEY_PARTS = ("gold_patch", "/tests/", "metadata.json")

# Claude Code keeps background-command logs in its own temp dir, as
# ``/private/tmp/claude-<uid>/<cwd-slug>/<session-uuid>/tasks/<id>.output``.
# Those ``/tasks/`` paths are the CLI's plumbing, not the benchmark task tree,
# so they must not count as benchmark data (they flagged 33 of the first 69
# Opus 5.5 Frontier v4 runs as contaminated). The shell-word scan can also
# cut the path at the slug, leaving ``/<session-uuid>/tasks/<id>.output`` or
# a bare ``/tasks/<id>.output``, so those fragments are matched too.
_CLI_TASK_LOG_RE = re.compile(
    r"^(?:/(?:private/)?tmp/claude-\d+/[^/]+)?"
    r"(?:/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})?"
    r"/tasks/(?:b[a-z0-9]{8}\.output)?$"
)

FS_VERDICTS = ("clean", "out_of_workspace", "benchmark_data_access", "answer_key_access")


def _candidate_paths(line: str) -> list[str]:
    """Absolute paths named by a tool call: explicit path args and shell words."""
    out = [p for p in _PATH_RE.findall(line) if p.startswith("/")]
    for cmd in _CMD_RE.findall(line):
        out.extend(re.findall(r"(?<![\w/])(/[\w./~-]{4,})", cmd))
    return out


def audit_filesystem(  # noqa: PLR0912, linear path-scan over one stream
    stream_path: Path,
    workspace: Path | None,
    task_id: str,
    repo_root: Path | None = None,
) -> dict[str, Any]:
    """Check that a CLI agent stayed inside its workspace.

    ``repo_root`` defaults to this checkout: the tree holding ``tasks/`` and
    ``runs/``, i.e. exactly the benchmark data an agent must never read.
    """
    root = (repo_root or Path(__file__).resolve().parents[2]).resolve()
    ws = str(workspace.resolve()) if workspace else None
    # CLI-harness runs execute in a VulcanBench-created tmp perimeter
    # (``vulcanbench-<run_id>-XXXX/workspace``) that is gone by re-annotation
    # time; paths under it are inside the containment regardless of which
    # workspace path this audit was handed. The run id embeds the task id.
    perimeter = re.compile(rf"/vulcanbench-{re.escape(task_id)}-[^/]+(?:/|$)") if task_id else None
    outside: set[str] = set()
    benchmark: set[str] = set()
    answer_key: set[str] = set()

    if stream_path.exists():
        with stream_path.open(encoding="utf-8", errors="replace") as f:
            for line in f:
                if '"path"' not in line and '"command"' not in line and '"target_' not in line:
                    continue
                for raw in _candidate_paths(line):
                    if ws and (raw == ws or raw.startswith(ws + "/")):
                        continue
                    if perimeter and perimeter.search(raw):
                        continue
                    # /dev/null etc. are shell plumbing, not data access.
                    if raw == "/dev" or raw.startswith("/dev/"):
                        continue
                    outside.add(raw)
                    in_tasks = "/tasks/" in raw and not _CLI_TASK_LOG_RE.match(raw)
                    in_runs = f"{root}/runs" in raw or ("/runs/" in raw and str(root) in raw)
                    if not (in_tasks or in_runs):
                        continue
                    benchmark.add(raw)
                    own = f"/{task_id}/" in raw or raw.endswith(f"/{task_id}")
                    if own and any(part in raw for part in _ANSWER_KEY_PARTS):
                        answer_key.add(raw)

    if answer_key:
        verdict = "answer_key_access"
    elif benchmark:
        verdict = "benchmark_data_access"
    elif outside:
        verdict = "out_of_workspace"
    else:
        verdict = "clean"
    return {
        "verdict": verdict,
        "out_of_workspace_paths": sorted(outside)[:40],
        "benchmark_data_paths": sorted(benchmark)[:40],
        "answer_key_paths": sorted(answer_key)[:20],
        "contaminated": verdict in ("benchmark_data_access", "answer_key_access"),
    }


def audit_run(
    stream_path: Path,
    metadata: dict[str, Any],
    workspace: Path | None = None,
    repo_root: Path | None = None,
) -> dict[str, Any]:
    """Combined integrity block for a CLI-harness run."""
    web = audit_stream(stream_path, metadata)
    fs = audit_filesystem(stream_path, workspace, str(metadata.get("id") or ""), repo_root)
    return {
        "web": web,
        "filesystem": fs,
        "contaminated": bool(web["contaminated"] or fs["contaminated"]),
    }
