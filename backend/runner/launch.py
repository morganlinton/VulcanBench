"""Launch plans: what the GUI would run, validated before it is shown.

A plan is built server-side from the form, shown to the operator with the
exact argv, working directory and the names of the environment variables the
child gets, and only a confirm click with the plan's hash turns it into a
queued job. The supervisor re-validates right before it starts the child.

Rules enforced here (docs/HOW_WE_WORK.md, docs/GUI.md):

- Every run starts from a worktree under ``[runner].run_worktrees`` created by
  ``scripts/run_worktree.sh``, detached at the plan's ``bench/...`` tag, with
  a clean tree under the scoring paths. The backend's own checkout is never a
  launch directory.
- The child environment is an allowlist. ``VULCANBENCH_*`` (so
  ``VULCANBENCH_ALLOW_DIRTY`` cannot leak in), ``DATABASE_URL``, ``*_API_KEY``
  and ``*_TOKEN`` never pass. The backend opens no credential file.

The only plan kind built here is ``prepare`` (create the worktree for a tag).
Suite sweeps are planned in ``backend/runner/sweep.py``.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
from pathlib import Path
from typing import Any

from backend.runner.config import REPO_ROOT, RunnerConfig

WORKTREE_SCRIPT = Path("scripts/run_worktree.sh")
SCORING_PATHS = ("harness", "tasks", "sandbox", "vulcanbench.toml", "pyproject.toml", "uv.lock")
TASK_TIMEOUT_S = 3 * 3600  # docs/DECISIONS.md: every suite carries a flat 3-hour task timeout

ENV_PASSTHROUGH = ("HOME", "PATH", "USER", "LANG", "TMPDIR")
# Where the subscription CLIs keep their login when the operator moved it; the
# harness reads these itself (harness/agent/cli_agents.py). Never the keys.
SUITE_ENV_PASSTHROUGH = (
    "CODEX_HOME",
    "CLAUDE_CONFIG_DIR",
    "GROK_HOME",
    "ZCODE_HOME",
    "ZCODE_STORAGE_DIR",
    "ZCODE_DATA_BASE_DIR",
)
# Detached children start from the backend's PATH, which may lack the agent
# CLIs (~/.local/bin) and node (~/.local/node/bin, which codex.js needs); the
# sweep scripts prepend the same directories.
PATH_EXTRAS = ("~/.local/bin", "~/.local/node/bin", "/opt/homebrew/bin", "/usr/local/bin")
_FORBIDDEN_ENV = re.compile(r"^(VULCANBENCH_.*|DATABASE_URL|.*_API_KEY|.*_TOKEN|.*SECRET.*)$")

_TAG_RE = re.compile(r"^bench/[A-Za-z0-9._-]+$")
_NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")


class LaunchError(Exception):
    def __init__(self, status: int, detail: str) -> None:
        super().__init__(detail)
        self.status = status
        self.detail = detail


def _git(cwd: Path, *args: str, timeout: float = 10) -> str | None:
    try:
        proc = subprocess.run(
            ["git", "-C", str(cwd), *args],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    return proc.stdout.strip() if proc.returncode == 0 else None


# ------------------------------------------------------------------ env --


def child_env(overrides: dict[str, str] | None = None, *, path_prepend: str = "") -> dict[str, str]:
    """The allowlisted environment a child gets. Never ``os.environ.copy()``."""
    env: dict[str, str] = {}
    for name in (*ENV_PASSTHROUGH, *SUITE_ENV_PASSTHROUGH):
        value = os.environ.get(name)
        if value:
            env[name] = value
    for name, value in (overrides or {}).items():
        if _FORBIDDEN_ENV.match(name):
            raise LaunchError(422, f"{name} may not be set on a launched run")
        env[name] = str(value)
    env.setdefault("PATH", "/usr/local/bin:/usr/bin:/bin")
    parts = env["PATH"].split(os.pathsep)
    for extra in PATH_EXTRAS:
        expanded = os.path.expanduser(extra)
        if expanded not in parts and os.path.isdir(expanded):
            parts.append(expanded)
    if path_prepend:
        parts = [path_prepend, *[p for p in parts if p != path_prepend]]
    env["PATH"] = os.pathsep.join(parts)
    for name in list(env):
        if _FORBIDDEN_ENV.match(name):
            del env[name]
    return env


# ------------------------------------------------------------- options --


def bench_tags(repo: Path | None = None) -> list[str]:
    out = _git(repo or REPO_ROOT, "tag", "-l", "bench/*", "--sort=-creatordate")
    return [t for t in (out or "").splitlines() if t.strip()]


def worktree_dir(cfg: RunnerConfig, tag: str) -> Path:
    return cfg.run_worktrees / tag.replace("/", "-")


def tag_commit(tag: str, repo: Path | None = None) -> str | None:
    return _git(repo or REPO_ROOT, "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}")


def worktree_status(cfg: RunnerConfig, tag: str) -> dict[str, Any]:
    """Everything the page shows about the worktree for a tag, plus the problems that block a launch."""
    wt = worktree_dir(cfg, tag)
    expected = tag_commit(tag)
    info: dict[str, Any] = {
        "tag": tag,
        "path": str(wt),
        "exists": wt.is_dir(),
        "tag_commit": expected[:12] if expected else None,
        "head": None,
        "at_tag": False,
        "dirty": [],
        "problems": [],
    }
    problems: list[str] = info["problems"]
    if not _TAG_RE.match(tag):
        problems.append("tag must look like bench/<label>")
    if expected is None:
        problems.append(f"no tag {tag} in {REPO_ROOT.name}")
    try:
        same_as_repo = wt.resolve() == REPO_ROOT.resolve()
    except OSError:
        same_as_repo = False
    if same_as_repo:
        problems.append("the worktree is the backend's own checkout")
    if not wt.is_dir():
        return info
    head = _git(wt, "rev-parse", "HEAD")
    info["head"] = head[:12] if head else None
    toplevel = _git(wt, "rev-parse", "--show-toplevel")
    if toplevel is None or Path(toplevel).resolve() != wt.resolve():
        problems.append("not a git checkout of its own")
        return info
    info["at_tag"] = bool(head and expected and head == expected)
    if not info["at_tag"]:
        problems.append(f"HEAD is not at {tag}")
    dirty = _git(wt, "status", "--porcelain", "--", *SCORING_PATHS)
    if dirty is None:
        problems.append("git status failed in the worktree")
    elif dirty:
        info["dirty"] = dirty.splitlines()[:20]
        problems.append("uncommitted changes under the scoring paths")
    if not (wt / ".venv" / "bin" / "python").exists():
        problems.append("no .venv in the worktree (run_worktree.sh did not finish)")
    return info


def validate_worktree(cfg: RunnerConfig, tag: str) -> list[str]:
    return list(worktree_status(cfg, tag)["problems"])


def options(cfg: RunnerConfig, tag: str | None = None) -> dict[str, Any]:
    """The tag and worktree part of the Launch page's options; sweep.py adds the suite part."""
    tags = bench_tags()
    chosen = tag if tag in tags else (tags[0] if tags else None)
    return {
        "tags": tags,
        "tag": chosen,
        "worktree": worktree_status(cfg, chosen) if chosen else None,
        "run_worktrees": str(cfg.run_worktrees),
    }


# ---------------------------------------------------------------- plans --


_HASHED_KEYS = ("kind", "argv", "cwd", "env", "path_prepend", "prep_argv", "prep_env", "job_name")


def plan_hash(spec: dict[str, Any]) -> str:
    """Hash of what will actually run; the confirm click sends it back."""
    material: dict[str, Any] = {k: spec.get(k) for k in _HASHED_KEYS}
    material["jobs"] = [
        {k: j.get(k) for k in _HASHED_KEYS} for j in (spec.get("jobs") or []) if isinstance(j, dict)
    ]
    return hashlib.sha256(json.dumps(material, sort_keys=True).encode()).hexdigest()[:16]


def build_prepare_plan(cfg: RunnerConfig, tag: str) -> dict[str, Any]:
    """A queue step that only runs ``run_worktree.sh <tag>``: no model calls, no quota."""
    if not _TAG_RE.match(tag):
        raise LaunchError(422, "tag must look like bench/<label>")
    if tag_commit(tag) is None:
        raise LaunchError(422, f"no tag {tag}")
    wt_info = worktree_status(cfg, tag)
    if wt_info["exists"]:
        raise LaunchError(
            409,
            f"{wt_info['path']} already exists"
            + (
                "; it has problems: " + "; ".join(wt_info["problems"])
                if wt_info["problems"]
                else ""
            ),
        )
    if wt_info["problems"]:
        raise LaunchError(409, "; ".join(wt_info["problems"]))
    spec: dict[str, Any] = {
        "kind": "prepare",
        "job_name": "prepare-" + tag.replace("/", "-"),
        "tag": tag,
        "tag_commit": wt_info["tag_commit"],
        "worktree": wt_info["path"],
        "needs_prep": True,
        "argv": None,
        "cwd": str(REPO_ROOT),
        "env": {},
        "env_passthrough": [n for n in ENV_PASSTHROUGH if os.environ.get(n)],
        "prep_argv": ["bash", str(REPO_ROOT / WORKTREE_SCRIPT), tag],
        "prep_env": {"RUN_WORKTREES": str(cfg.run_worktrees)},
        "estimate": {
            "billing": "none: git worktree add, make setup and the suite-lock check, no model calls",
            "infra_retries": "none",
        },
        "usage_check_required": False,
        "warnings": [
            f"make setup takes minutes; the log lands in {wt_info['path']}/.venv-setup.log"
        ],
    }
    spec["hash"] = plan_hash(spec)
    return spec


def validate_before_launch(cfg: RunnerConfig, spec: dict[str, Any]) -> list[str]:
    """Re-check a prepare plan right before its child starts (confirm and launch time).

    Suite jobs are checked by ``sweep.validate_suite_launch``; ``sweep.validate_any``
    dispatches on the kind and sends only prepare specs here.
    """
    problems: list[str] = []
    if spec.get("kind") != "prepare":
        return ["unsupported kind"]
    tag = str(spec.get("tag") or "")
    if not _TAG_RE.match(tag) or tag_commit(tag) is None:
        problems.append(f"no tag {tag}")
    if Path(str(spec.get("worktree"))) != worktree_dir(cfg, tag):
        problems.append("worktree path does not match the tag")
    return problems
