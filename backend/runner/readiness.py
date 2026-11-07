"""Is this Mac ready to run benchmarks and build suites? One read-only checklist.

Every check is local and quota-free: the supervisor's state, the Docker
daemon, the newest bench tag and its run worktree, ``gh auth status`` (exit
code only; its output is discarded unread because it can name the account
and token scopes), and the suite builder's ``suite access`` in the tooling
worktree. Nothing here starts a job, calls a model or reads a credential
file. Harness logins are not checked here: every plan runs the harness
doctor for the harness it names.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from typing import Any

from backend.runner import docker_ps, launch, suites
from backend.runner.config import RunnerConfig

GH_TIMEOUT_S = 10.0

OK = "ok"
WARN = "warn"
FAIL = "fail"


def _check(
    cid: str, label: str, status: str, detail: str, fix: str | None = None, *, needed_for: str
) -> dict[str, Any]:
    return {
        "id": cid,
        "label": label,
        "status": status,
        "detail": detail,
        "fix": fix,
        "needed_for": needed_for,
    }


def supervisor_check(describe: dict[str, Any]) -> dict[str, Any]:
    status = str(describe.get("status") or "unknown")
    if status == "active" and not describe.get("hold_reason"):
        return _check(
            "supervisor",
            "Queue supervisor",
            OK,
            "running; queued jobs start one at a time",
            needed_for="runs",
        )
    detail = str(describe.get("hold_reason") or describe.get("detail") or status)
    return _check(
        "supervisor",
        "Queue supervisor",
        FAIL,
        f"not running jobs: {detail}",
        "Restart the VulcanBench backend (or the Mac app) with VULCANBENCH_RUNNER=1.",
        needed_for="runs",
    )


def docker_check(available: bool) -> dict[str, Any]:
    if available:
        return _check(
            "docker", "Docker", OK, "the daemon answers", needed_for="docker runs and validation"
        )
    return _check(
        "docker",
        "Docker",
        WARN,
        "the daemon does not answer",
        "Start Docker Desktop. Runs in the local sandbox work without it; task validation does not.",
        needed_for="docker runs and validation",
    )


def tag_check(cfg: RunnerConfig) -> dict[str, Any]:
    tags = launch.bench_tags()
    if not tags:
        return _check(
            "bench_tag",
            "Bench tag",
            FAIL,
            "the checkout has no bench/* tag",
            "Cut one from main with the Cut a bench tag card on the Launch page.",
            needed_for="runs",
        )
    broken: dict[str, Any] | None = None
    for i, tag in enumerate(tags):
        info = launch.worktree_status(cfg, tag)
        if not info["exists"]:
            continue
        if info["problems"]:
            broken = broken or info
            continue
        detail = f"{tag} is prepared at {info.get('tag_commit') or '?'}"
        if i == 1:
            detail += f"; the newer {tags[0]} is not"
        elif i > 1:
            detail += f"; {i} newer tags are not (newest: {tags[0]})"
        return _check("bench_tag", "Bench tag", OK, detail, needed_for="runs")
    if broken is not None:
        return _check(
            "bench_tag",
            "Bench tag",
            FAIL,
            f"{broken['tag']}: " + "; ".join(str(p) for p in broken["problems"]),
            "Fix the worktree or prepare another tag; launches refuse until then.",
            needed_for="runs",
        )
    return _check(
        "bench_tag",
        "Bench tag",
        WARN,
        f"no bench tag has a run worktree yet (newest: {tags[0]})",
        "Pick a bench tag on the Launch page and prepare its worktree (no model calls).",
        needed_for="runs",
    )


def gh_check() -> dict[str, Any]:
    gh = shutil.which("gh")
    if gh is None:
        return _check(
            "gh",
            "GitHub CLI",
            WARN,
            "gh is not installed",
            "Install it (brew install gh), then run gh auth login in a terminal.",
            needed_for="building eval suites",
        )
    env = {k: v for k, v in os.environ.items() if k in ("HOME", "PATH", "USER", "TMPDIR")}
    env["GH_PROMPT_DISABLED"] = "1"
    try:
        # Fixed argv, no shell; the output is discarded unread.
        res = subprocess.run(
            [gh, "auth", "status", "--hostname", "github.com"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            env=env,
            timeout=GH_TIMEOUT_S,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return _check(
            "gh",
            "GitHub CLI",
            WARN,
            "gh auth status did not answer",
            "Run gh auth status in a terminal to see why.",
            needed_for="building eval suites",
        )
    if res.returncode == 0:
        return _check(
            "gh", "GitHub CLI", OK, "logged in to github.com", needed_for="building eval suites"
        )
    return _check(
        "gh",
        "GitHub CLI",
        WARN,
        "not logged in to github.com",
        "Run gh auth login in a terminal on this Mac.",
        needed_for="building eval suites",
    )


def builder_check(cfg: RunnerConfig) -> dict[str, Any]:
    try:
        res = suites.access_status(cfg)
    except suites.SuitesError as exc:
        msg = exc.detail.get("message") if isinstance(exc.detail, dict) else str(exc.detail)
        return _check(
            "builder",
            "Eval suite builder",
            WARN,
            f"suite access failed: {msg}",
            "Open the Suites page for details.",
            needed_for="building eval suites",
        )
    tooling, access = res["tooling"], res["access"]
    if not tooling["ready"] or access is None:
        return _check(
            "builder",
            "Eval suite builder",
            WARN,
            "; ".join(str(p) for p in tooling["problems"]) or "no tooling worktree",
            "Cut a bench tag that contains the eval suite builder and prepare it.",
            needed_for="building eval suites",
        )
    if not access["configured"]:
        return _check(
            "builder",
            "Eval suite builder",
            WARN,
            f"no password is set at {tooling['tag']}",
            "Run .venv/bin/vulcanbench suite password-hash, put the hash in [builder].password_hash "
            "in vulcanbench.toml, commit it, cut a tag and prepare it.",
            needed_for="building eval suites",
        )
    if not access["unlocked"]:
        # Locked is the normal resting state, not a problem: say how to open it.
        return _check(
            "builder",
            "Eval suite builder",
            OK,
            f"locked; unlock it on the Eval suites page to build one (runs from {tooling['tag']})",
            needed_for="building eval suites",
        )
    return _check(
        "builder",
        "Eval suite builder",
        OK,
        f"unlocked; commands run from {tooling['tag']}",
        needed_for="building eval suites",
    )


def readiness(cfg: RunnerConfig, supervisor: dict[str, Any]) -> dict[str, Any]:
    checks = [
        supervisor_check(supervisor),
        tag_check(cfg),
        docker_check(docker_ps.docker_available()),
        gh_check(),
        builder_check(cfg),
    ]
    worst = (
        FAIL
        if any(c["status"] == FAIL for c in checks)
        else (WARN if any(c["status"] == WARN for c in checks) else OK)
    )
    return {"status": worst, "checks": checks}
