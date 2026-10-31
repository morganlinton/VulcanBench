"""Housekeeping the operator used to do by hand: orphaned sandboxes and bench tags.

Orphans: containers that look like benchmark sandboxes (harbor's
``<task>__<trial>__env`` compose projects, the harness's ``vb-<run id>``
projects, ``vb-agent-*`` agent containers, ``vulcanbench/*`` images) that no
active run owns any more. The Runs page lists them with the run they belonged
to, and removes only the ids the operator confirmed, re-checked at that
moment. Nothing is removed by image name or broad prefix while a run that
could own it is still alive.

Bench tags: ``scripts/cut_bench_tag.sh <label> [--push]`` run in the
backend's own checkout. The script refuses unless that checkout is on main,
clean and equal to origin/main, so the GUI adds nothing but a form and shows
the script's output verbatim (redacted).
"""

from __future__ import annotations

import re
import shutil
import subprocess
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from backend.runner import docker_ps
from backend.runner.config import REPO_ROOT, RunnerConfig
from backend.runner.discovery import Discovery
from backend.runner.docker_ps import Container
from backend.runner.launch import child_env
from backend.runner.redact import redact
from backend.runner.state import State

TAG_SCRIPT = Path("scripts/cut_bench_tag.sh")
_LABEL_RE = re.compile(r"^[a-z0-9][a-z0-9.-]*$")
ACTIVE = frozenset({"running", "external_running", "stopping", "preparing", "queued", "waiting"})


@dataclass
class Orphan:
    id: str
    name: str
    status: str
    state: str
    image: str
    project: str | None
    run: str | None  # the run it belonged to, when one is on disk
    reason: str


def looks_like_ours(c: Container) -> bool:
    project = c.compose_project or ""
    # Compose projects and names only: the GUI's own dashboard container runs from
    # a vulcanbench/ image too, so the image is never the signal.
    return project.endswith("__env") or project.startswith("vb-") or c.name.startswith("vb-agent-")


def orphan_containers(
    cfg: RunnerConfig,
    state: State,
    *,
    containers: list[Container] | None = None,
    discovery: Discovery | None = None,
) -> dict[str, Any]:
    """Benchmark containers no active run owns, with why they count as orphans."""
    table = containers if containers is not None else docker_ps.list_containers()
    disc = discovery or Discovery(cfg, containers=table, jobs=state.list_jobs(limit=200))
    runs = disc.all_runs()
    owner_by_name: dict[str, tuple[str, str]] = {}  # container name -> (run id, run status)
    for run in runs:
        for trial in run.trials:
            for cv in trial.containers:
                owner_by_name[cv.name] = (run.id, run.status)
    live = [r.id for r in runs if r.status in ACTIVE]
    orphans: list[Orphan] = []
    for c in table:
        if not looks_like_ours(c):
            continue
        owner = owner_by_name.get(c.name)
        if owner is not None and owner[1] in ACTIVE:
            continue
        if owner is not None:
            reason = f"its run is {owner[1]}"
        elif live:
            reason = "no run on disk refers to it"
        else:
            reason = "no run on disk refers to it and nothing is running"
        orphans.append(
            Orphan(
                id=c.id,
                name=c.name,
                status=c.status,
                state=c.state,
                image=c.image,
                project=c.compose_project,
                run=owner[0] if owner else None,
                reason=reason,
            )
        )
    return {
        "containers": [asdict(o) for o in orphans],
        "live_runs": live,
        "docker_available": shutil.which("docker") is not None,
    }


def remove_containers(ids: list[str], orphans: list[dict[str, Any]]) -> dict[str, Any]:
    """``docker rm -f`` the requested ids, but only those that are orphans right now."""
    allowed = {str(o["id"]): o for o in orphans}
    refused = [i for i in ids if i not in allowed]
    chosen = [i for i in ids if i in allowed]
    result: dict[str, Any] = {"removed": [], "refused": refused, "networks": 0, "error": None}
    if not chosen:
        return result
    docker = shutil.which("docker") or str(Path("~/.docker/bin/docker").expanduser())
    try:
        proc = subprocess.run(
            [docker, "rm", "-f", *chosen], capture_output=True, text=True, timeout=120, check=False
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        result["error"] = f"docker rm failed: {exc}"
        return result
    removed = set(proc.stdout.split())
    result["removed"] = [i for i in chosen if i in removed or i[:12] in removed]
    if proc.returncode != 0:
        result["error"] = redact(proc.stderr.strip()[-500:])
    projects = sorted({str(allowed[i]["project"]) for i in chosen if allowed[i].get("project")})
    for project in projects:
        try:
            ls = subprocess.run(
                [
                    docker,
                    "network",
                    "ls",
                    "-q",
                    "--filter",
                    f"label=com.docker.compose.project={project}",
                ],
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
            nets = ls.stdout.split()
            if nets:
                rm = subprocess.run(
                    [docker, "network", "rm", *nets],
                    capture_output=True,
                    text=True,
                    timeout=30,
                    check=False,
                )
                result["networks"] += len(rm.stdout.split())
        except (OSError, subprocess.TimeoutExpired):
            continue
    return result


# ---------------------------------------------------------------- tags --


def _git(*args: str, cwd: Path | None = None) -> str | None:
    try:
        proc = subprocess.run(
            ["git", "-C", str(cwd or REPO_ROOT), *args],
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    return proc.stdout.strip() if proc.returncode == 0 else None


def tag_preflight(repo: Path | None = None) -> dict[str, Any]:
    """What cut_bench_tag.sh will check, read without touching the checkout (no fetch)."""
    repo = repo or REPO_ROOT
    branch = _git("rev-parse", "--abbrev-ref", "HEAD", cwd=repo)
    dirty = _git("status", "--porcelain", cwd=repo)
    head = _git("rev-parse", "--short", "HEAD", cwd=repo)
    origin = _git("rev-parse", "--short", "origin/main", cwd=repo)
    problems: list[str] = []
    if branch != "main":
        problems.append(f"the checkout is on {branch}, not main")
    if dirty:
        problems.append("the working tree is not clean")
    if head and origin and head != origin:
        problems.append("local main differs from origin/main as last fetched")
    return {
        "checkout": str(repo),
        "branch": branch,
        "head": head,
        "origin_main": origin,
        "clean": not dirty,
        "problems": problems,
        "script": str(repo / TAG_SCRIPT),
        "argv_preview": ["bash", str(repo / TAG_SCRIPT), "<label>", "[--push]"],
    }


def cut_bench_tag(label: str, *, push: bool, repo: Path | None = None) -> dict[str, Any]:
    """Run the script. Its own guards decide; the GUI only shows what it printed."""
    repo = repo or REPO_ROOT
    if not _LABEL_RE.match(label) or len(label) > 60:
        return {
            "ok": False,
            "exit_code": 2,
            "output": "label: lowercase letters, digits, dots and dashes, at most 60",
            "tag": None,
            "argv": [],
        }
    argv = ["bash", str(repo / TAG_SCRIPT), label, *(["--push"] if push else [])]
    try:
        proc = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            timeout=180,
            check=False,
            cwd=str(repo),
            env=child_env(),
        )
    except subprocess.TimeoutExpired:
        return {
            "ok": False,
            "exit_code": None,
            "output": "cut_bench_tag.sh timed out",
            "tag": None,
            "argv": argv,
        }
    except OSError as exc:
        return {
            "ok": False,
            "exit_code": None,
            "output": f"could not start: {exc}",
            "tag": None,
            "argv": argv,
        }
    output = (proc.stdout + proc.stderr).strip()
    m = re.search(r"tagged (bench/\S+) at", proc.stdout)
    return {
        "ok": proc.returncode == 0,
        "exit_code": proc.returncode,
        "output": redact(output[-3000:]),
        "tag": m.group(1) if m else None,
        "argv": argv,
    }
