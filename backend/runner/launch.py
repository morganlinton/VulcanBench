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
  and ``*_TOKEN`` never pass. Credential files are not opened; only their
  presence is reported.
- Gate runs are exactly ``bash <worktree>/scripts/frontier-v5/run_gate.sh
  <ref> <task>`` with the ``GATE_*`` knobs in the environment.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from backend.runner.config import REPO_ROOT, RunnerConfig

GATE_SCRIPT = Path("scripts/frontier-v5/run_gate.sh")
WORKTREE_SCRIPT = Path("scripts/run_worktree.sh")
SCORING_PATHS = ("harness", "tasks", "sandbox", "vulcanbench.toml", "pyproject.toml", "uv.lock")
TASK_TIMEOUT_S = 3 * 3600  # docs/DECISIONS.md: every suite carries a flat 3-hour task timeout

ENV_PASSTHROUGH = ("HOME", "PATH", "USER", "LANG", "TMPDIR")
GATE_ENV_PASSTHROUGH = ("CLAUDE_GATE_ENV", "FABLE_AUTH")
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
GATE_ENV_KNOBS = ("GATE_OUT", "GATE_ATTEMPTS", "GATE_CONCURRENCY", "GATE_JOB_SUFFIX")
# Detached children start from the backend's PATH, which may lack the agent
# CLIs (~/.local/bin) and node (~/.local/node/bin, which codex.js needs); the
# sweep scripts prepend the same directories.
PATH_EXTRAS = ("~/.local/bin", "~/.local/node/bin", "/opt/homebrew/bin", "/usr/local/bin")
_FORBIDDEN_ENV = re.compile(r"^(VULCANBENCH_.*|DATABASE_URL|.*_API_KEY|.*_TOKEN|.*SECRET.*)$")

_TAG_RE = re.compile(r"^bench/[A-Za-z0-9._-]+$")
_NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
_SUFFIX_RE = re.compile(r"^(-[A-Za-z0-9._-]+)?$")


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
    for name in (*ENV_PASSTHROUGH, *GATE_ENV_PASSTHROUGH, *SUITE_ENV_PASSTHROUGH):
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

_refs_cache: dict[str, tuple[float, list[dict[str, Any]]]] = {}


def gate_refs(script: Path | None = None) -> list[dict[str, Any]]:
    """``run_gate.sh --refs``: the reference models the script knows."""
    script = script or REPO_ROOT / GATE_SCRIPT
    key = str(script)
    hit = _refs_cache.get(key)
    if hit and time.monotonic() - hit[0] < 300:
        return hit[1]
    refs: list[dict[str, Any]] = []
    try:
        proc = subprocess.run(
            ["bash", str(script), "--refs"],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
            env=child_env(),
        )
    except (OSError, subprocess.TimeoutExpired):
        proc = None
    if proc is not None and proc.returncode == 0:
        for line in proc.stdout.splitlines():
            try:
                obj: Any = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(obj, dict) and "name" in obj:
                refs.append(obj)
    _refs_cache[key] = (time.monotonic(), refs)
    return refs


def bench_tags(repo: Path | None = None) -> list[str]:
    out = _git(repo or REPO_ROOT, "tag", "-l", "bench/*", "--sort=-creatordate")
    return [t for t in (out or "").splitlines() if t.strip()]


def worktree_dir(cfg: RunnerConfig, tag: str) -> Path:
    return cfg.run_worktrees / tag.replace("/", "-")


def tag_commit(tag: str, repo: Path | None = None) -> str | None:
    return _git(repo or REPO_ROOT, "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}")


def script_features(script: Path) -> dict[str, bool]:
    """What this checkout's run_gate.sh supports (older tags predate the GUI changes)."""
    try:
        text = script.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return {"present": False, "status_file": False, "preflight_only": False, "refs": False}
    return {
        "present": True,
        "status_file": "write_status" in text,
        "preflight_only": "GATE_PREFLIGHT_ONLY" in text,
        "refs": "--refs" in text,
    }


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
        "script": script_features(wt / GATE_SCRIPT),
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


def _task_language(text: str) -> str | None:
    m = re.search(r'^vulcanbench_language\s*=\s*"([^"]+)"', text, re.MULTILINE)
    return m.group(1) if m else None


def gate_tasks(cfg: RunnerConfig, tag: str) -> list[dict[str, Any]]:
    """Frontier v5 tasks at this tag: from the worktree when it exists, else from git."""
    out: list[dict[str, Any]] = []
    wt = worktree_dir(cfg, tag)
    root = wt / "tasks" / "frontier-v5"
    if root.is_dir():
        for d in sorted(root.iterdir()):
            toml = d / "task.toml"
            if d.is_dir() and toml.is_file():
                try:
                    lang = _task_language(toml.read_text(encoding="utf-8", errors="replace"))
                except OSError:
                    lang = None
                out.append(_task_entry(d.name, lang))
        return out
    listing = _git(REPO_ROOT, "ls-tree", "--name-only", f"refs/tags/{tag}", "tasks/frontier-v5/")
    for line in (listing or "").splitlines():
        name = line.rstrip("/").rsplit("/", 1)[-1]
        if not name or not _NAME_RE.match(name):
            continue
        text = _git(REPO_ROOT, "show", f"refs/tags/{tag}:tasks/frontier-v5/{name}/task.toml")
        if text is None:
            continue
        out.append(_task_entry(name, _task_language(text)))
    return out


def _task_entry(name: str, lang: str | None) -> dict[str, Any]:
    return {
        "name": name,
        "language": lang,
        "default_concurrency": 2 if lang in {"python", "javascript"} else 1,
    }


def credentials_present() -> dict[str, bool]:
    """Existence checks only, the same ones run_gate.sh makes. Nothing is opened."""
    gate_env = os.environ.get("CLAUDE_GATE_ENV") or "~/.config/vulcanbench/claude-gate.env"
    return {
        "codex_auth": Path("~/.codex/auth.json").expanduser().is_file(),
        "claude_gate_env": Path(gate_env).expanduser().is_file(),
    }


def options(cfg: RunnerConfig, tag: str | None = None) -> dict[str, Any]:
    tags = bench_tags()
    chosen = tag if tag in tags else (tags[0] if tags else None)
    return {
        "kinds": ["gate"],
        "refs": gate_refs(),
        "tags": tags,
        "tag": chosen,
        "worktree": worktree_status(cfg, chosen) if chosen else None,
        "tasks": gate_tasks(cfg, chosen) if chosen else [],
        "attempts": [1, 2, 3],
        "concurrency": [1, 2],
        "gate_effort": "medium",
        "credentials": credentials_present(),
        "gate_out": str(cfg.gate_out),
        "run_worktrees": str(cfg.run_worktrees),
    }


# ---------------------------------------------------------------- plans --


@dataclass(frozen=True)
class GateForm:
    ref: str
    task: str
    tag: str
    attempts: int = 3
    concurrency: int | None = None
    suffix: str = ""


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


def build_gate_plan(cfg: RunnerConfig, form: GateForm) -> dict[str, Any]:  # noqa: PLR0912, PLR0915
    refs = {r["name"]: r for r in gate_refs()}
    if form.ref not in refs:
        raise LaunchError(422, f"unknown reference {form.ref!r}; run_gate.sh knows {sorted(refs)}")
    if not _NAME_RE.match(form.task):
        raise LaunchError(422, "task name has characters run_gate.sh would not accept")
    if form.attempts not in (1, 2, 3):
        raise LaunchError(422, "attempts must be 1, 2 or 3 (GATE_ATTEMPTS)")
    if form.concurrency not in (None, 1, 2):
        raise LaunchError(422, "concurrency must be 1 or 2 (GATE_CONCURRENCY)")
    if not _SUFFIX_RE.match(form.suffix):
        raise LaunchError(422, "suffix must be empty or -<letters, digits, dots, dashes>")
    if not _TAG_RE.match(form.tag):
        raise LaunchError(422, "tag must look like bench/<label>")
    if tag_commit(form.tag) is None:
        raise LaunchError(422, f"no tag {form.tag}")

    tasks = {t["name"]: t for t in gate_tasks(cfg, form.tag)}
    if form.task not in tasks:
        raise LaunchError(422, f"{form.tag} has no tasks/frontier-v5/{form.task}")
    task = tasks[form.task]
    ref = refs[form.ref]
    job_name = f"{form.ref}-{form.task}{form.suffix}"
    if (cfg.gate_jobs_dir / job_name).exists():
        raise LaunchError(
            409, f"{cfg.gate_jobs_dir / job_name} exists; run_gate.sh would refuse. Use a suffix."
        )

    wt_info = worktree_status(cfg, form.tag)
    wt = Path(wt_info["path"])
    needs_prep = not wt_info["exists"]
    if not needs_prep and wt_info["problems"]:
        raise LaunchError(409, "worktree not usable: " + "; ".join(wt_info["problems"]))
    if needs_prep and wt_info["problems"]:
        raise LaunchError(409, "; ".join(wt_info["problems"]))

    env: dict[str, str] = {"GATE_OUT": str(cfg.gate_out), "GATE_ATTEMPTS": str(form.attempts)}
    concurrency = form.concurrency or int(task["default_concurrency"])
    if form.concurrency is not None:
        env["GATE_CONCURRENCY"] = str(form.concurrency)
    if form.suffix:
        env["GATE_JOB_SUFFIX"] = form.suffix
    argv = ["bash", str(wt / GATE_SCRIPT), form.ref, form.task]
    prep_argv = ["bash", str(REPO_ROOT / WORKTREE_SCRIPT), form.tag] if needs_prep else None
    prep_env = {"RUN_WORKTREES": str(cfg.run_worktrees)} if needs_prep else None

    rounds = math.ceil(form.attempts / concurrency)
    credentials = credentials_present()
    cred_key = "codex_auth" if ref.get("agent") == "codex" else "claude_gate_env"
    warnings: list[str] = []
    if not credentials[cred_key]:
        warnings.append(
            "credential file missing: "
            + ("~/.codex/auth.json" if cred_key == "codex_auth" else "CLAUDE_GATE_ENV")
            + " (run_gate.sh will refuse to start)"
        )
    if needs_prep:
        warnings.append(
            f"{wt} does not exist yet; run_worktree.sh will create it first (make setup takes minutes)"
        )
    features = wt_info["script"]
    if not needs_prep and not features["status_file"]:
        warnings.append(
            "this tag's run_gate.sh predates the status file; the GUI falls back to process and container checks"
        )
    if not needs_prep and not features["preflight_only"] and ref.get("agent") == "claude-code":
        warnings.append(
            "this tag's run_gate.sh has no stand-alone usage check; its own pre-flight runs at start"
        )

    spec: dict[str, Any] = {
        "kind": "gate",
        "job_name": job_name,
        "ref": form.ref,
        "agent": ref.get("agent"),
        "model": ref.get("model"),
        "harness_version": ref.get("version"),
        "effort": "medium",
        "task": form.task,
        "task_language": task["language"],
        "tag": form.tag,
        "tag_commit": wt_info["tag_commit"],
        "worktree": str(wt),
        "needs_prep": needs_prep,
        "attempts": form.attempts,
        "concurrency": concurrency,
        "concurrency_overridden": form.concurrency is not None,
        "suffix": form.suffix,
        "argv": argv,
        "cwd": str(wt),
        "env": env,
        "env_passthrough": [
            n for n in (*ENV_PASSTHROUGH, *GATE_ENV_PASSTHROUGH) if os.environ.get(n)
        ],
        "prep_argv": prep_argv,
        "prep_env": prep_env,
        "job_dir": str(cfg.gate_jobs_dir / job_name),
        "status_file": str(cfg.gate_out / f"{job_name}.status.json"),
        "script_features": features,
        "estimate": {
            "attempts": form.attempts,
            "concurrency": concurrency,
            "task_timeout_s": TASK_TIMEOUT_S,
            "max_wall_s": rounds * TASK_TIMEOUT_S,
            "billing": (
                "ChatGPT subscription (codex login)"
                if ref.get("agent") == "codex"
                else "Claude subscription or API key from CLAUDE_GATE_ENV"
            ),
            "early_exit": "two solves end the job early",
            "infra_retries": "none: an infrastructure failure stops the job",
        },
        "usage_check_required": ref.get("agent") == "claude-code"
        and (needs_prep or features["preflight_only"]),
        "warnings": warnings,
    }
    spec["hash"] = plan_hash(spec)
    return spec


def validate_before_launch(cfg: RunnerConfig, spec: dict[str, Any]) -> list[str]:
    """Re-check a plan right before its child starts (confirm and launch time)."""
    problems: list[str] = []
    if spec.get("kind") == "prepare":
        tag = str(spec.get("tag") or "")
        if not _TAG_RE.match(tag) or tag_commit(tag) is None:
            problems.append(f"no tag {tag}")
        if Path(str(spec.get("worktree"))) != worktree_dir(cfg, tag):
            problems.append("worktree path does not match the tag")
        return problems
    if spec.get("kind") != "gate":
        problems.append("unsupported kind")
    tag = str(spec.get("tag") or "")
    problems.extend(validate_worktree(cfg, tag))
    if Path(str(spec.get("worktree"))) != worktree_dir(cfg, tag):
        problems.append("worktree path does not match the tag")
    argv = spec.get("argv") or []
    if not isinstance(argv, list) or len(argv) != 4 or argv[0] != "bash":
        problems.append("argv is not a run_gate.sh invocation")
    elif Path(argv[1]) != Path(str(spec.get("worktree"))) / GATE_SCRIPT:
        problems.append("argv does not run the worktree's run_gate.sh")
    elif "--allow-dirty" in argv:
        problems.append("argv contains --allow-dirty")
    if Path(str(spec.get("job_dir"))).exists():
        problems.append("job directory already exists")
    for name in spec.get("env") or {}:
        if name not in GATE_ENV_KNOBS:
            problems.append(f"unexpected env override {name}")
    return problems


# ------------------------------------------------------------ pre-flight --


def run_usage_preflight(
    cfg: RunnerConfig, spec: dict[str, Any], *, timeout: float = 180
) -> dict[str, Any]:
    """``GATE_PREFLIGHT_ONLY=1 run_gate.sh <ref> <task>``: one short model turn, on request only."""
    if spec.get("agent") != "claude-code":
        return {
            "status": "not_applicable",
            "would_start": True,
            "note": "no usage pre-flight for this agent",
        }
    wt = Path(str(spec["worktree"]))
    script = wt / GATE_SCRIPT if (wt / GATE_SCRIPT).is_file() else REPO_ROOT / GATE_SCRIPT
    if not script_features(script)["preflight_only"]:
        return {
            "status": "unavailable",
            "would_start": True,
            "note": f"{script} has no pre-flight-only mode",
        }
    env = child_env({"GATE_PREFLIGHT_ONLY": "1", "GATE_OUT": str(cfg.gate_out)})
    try:
        proc = subprocess.run(
            ["bash", str(script), str(spec["ref"]), str(spec["task"])],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
            env=env,
            cwd=str(script.parent.parent.parent),
        )
    except subprocess.TimeoutExpired:
        return {"status": "error", "would_start": False, "note": "pre-flight timed out"}
    except OSError as exc:
        return {
            "status": "error",
            "would_start": False,
            "note": f"pre-flight failed to start: {exc}",
        }
    last = ""
    for line in proc.stdout.splitlines():
        if line.strip().startswith("{"):
            last = line.strip()
    try:
        data: Any = json.loads(last) if last else None
    except json.JSONDecodeError:
        data = None
    if not isinstance(data, dict):
        stderr = proc.stderr.strip().splitlines()
        return {
            "status": "error",
            "would_start": False,
            "note": (
                stderr[-1] if stderr else f"pre-flight exited {proc.returncode} without a report"
            )[:300],
            "exit_code": proc.returncode,
        }
    data["exit_code"] = proc.returncode
    data["script"] = str(script)
    return data
