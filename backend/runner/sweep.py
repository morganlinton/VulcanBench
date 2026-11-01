"""Benchmark-a-model plans: a suite through ``vulcanbench run``, one job per effort level.

This is the flow the GUI is for: a new model comes out, the operator picks
it, a harness, a suite and the effort levels, reads the exact commands and
the estimate, and confirms. Each level becomes its own queued job (so each
shows its own progress) running

    <worktree>/.venv/bin/vulcanbench run --suite <suite> --model <m> --harness <h>
        --billing <b> --effort <level> --repeat <k> --sandbox <s> --no-judges
        --max-concurrency 1 --max-infra-retries 0 [--only-missing] -o <out>/<level>

from a run worktree pinned to a bench tag, exactly as the hand-run sweep
scripts under ``scripts/cii-v4-board/`` do, minus their retry loops.

What the plan checks without spending quota: the worktree (tag, clean
scoring paths, .venv), the suite's frozen task list at that tag,
``vulcanbench harness doctor <harness> --json`` (CLI installed, logged in)
and ``vulcanbench estimate --json`` (list-price cost; "no built-in pricing"
is the signal that the model has not been onboarded at this tag).

Progress for the Runs page comes from the output directory: a task is done
when a run dir holds ``summary.json``; the current task is the run dir whose
trace is still growing.
"""

from __future__ import annotations

import json
import os
import re
import statistics
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any

from backend.runner import launch
from backend.runner.config import REPO_ROOT, RunnerConfig
from backend.runner.discovery import _birth, _mtime, parse_dt, read_json
from backend.runner.launch import (
    ENV_PASSTHROUGH,
    SUITE_ENV_PASSTHROUGH,
    TASK_TIMEOUT_S,
    LaunchError,
    child_env,
    tag_commit,
    validate_worktree,
    worktree_dir,
    worktree_status,
)
from harness.settings import blocked_efforts

# ``harness.agent.cli_agents.CLI_AGENT_PROVIDERS`` plus Vulcan's own API loop.
# Listed here rather than imported so the backend never imports the agent
# adapters (tests assert the two lists agree).
CLI_HARNESSES = (
    "claude-code",
    "codex",
    "cursor",
    "grok-build",
    "zcode",
    "muse-code",
    "devin",
    "pi",
    "dsh",
    "mirror",
)
HARNESSES = (*CLI_HARNESSES, "vulcan")
EFFORT_ORDER = ("minimal", "low", "medium", "high", "extra-high", "max", "ultra")
BILLING_MODES = ("auto", "subscription", "api")
SANDBOXES = ("local", "docker")
# What every sweep script under scripts/cii-v4-board/ passes: CLI agents run
# the model's commands themselves, so the harness sandbox stays local; Vulcan's
# API loop executes model-written shell and defaults to docker.
SANDBOX_DEFAULTS = {h: "local" for h in CLI_HARNESSES} | {"vulcan": "docker"}
QUOTA_WAIT_S = 1800  # the sweep scripts' QUOTA_WAIT
QUOTA_CONTINUE_MAX = 48
MAX_REPEAT = 5

# Patterns the sweep scripts and the adapters treat as a subscription window
# or rate limit (not a model failure), and the ones that mean the login died.
QUOTA_RE = re.compile(
    r"usage limit|rate limit|limit reached|limit will reset|quota|too many requests|\b429\b",
    re.I,
)
AUTH_RE = re.compile(
    r"refresh token has expired|sign in again|not logged in|please log out|"
    r"authentication_failed|oauth access token is invalid|requires usage credits",
    re.I,
)
STALE_S = 15 * 60
_LABEL_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
_MODEL_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:/-]*$")
_RUN_ID_RE = re.compile(r"^(?P<task>.+)-[0-9a-f]{8}$")


@dataclass(frozen=True)
class SuiteForm:
    model: str
    harness: str
    suite: str
    tag: str
    efforts: tuple[str, ...]
    billing: str = "auto"
    repeat: int = 1
    only_missing: bool = True
    sandbox: str | None = None
    quota_continue: int = 0
    label: str = ""
    cli_bin_dir: str = ""


# ------------------------------------------------------------- options --


def efforts_offered() -> list[str]:
    """Effort levels in display order with ``[effort].blocked`` removed. Never shows ultra."""
    blocked = blocked_efforts()
    return [e for e in EFFORT_ORDER if e not in blocked]


def _git(cwd: Path, *args: str) -> str | None:
    try:
        proc = subprocess.run(
            ["git", "-C", str(cwd), *args], capture_output=True, text=True, timeout=10, check=False
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    return proc.stdout.strip() if proc.returncode == 0 else None


def _suite_entry(
    suite_id: str, lock: dict[str, Any], manifest: dict[str, Any] | None
) -> dict[str, Any]:
    tasks = lock.get("tasks")
    ids = sorted(tasks) if isinstance(tasks, dict) else list(tasks or [])
    display = str((manifest or {}).get("display_name") or suite_id)
    return {
        "id": suite_id,
        "name": display.split(" (", maxsplit=1)[0],
        "version": lock.get("version"),
        "frozen_at": lock.get("frozen_at"),
        "task_ids": [str(t) for t in ids],
        "n_tasks": len(ids),
    }


def suites_at(cfg: RunnerConfig, tag: str) -> list[dict[str, Any]]:
    """Frozen suites (``tasks/<suite>/suite.lock.json``) at this tag: from the worktree, else git."""
    out: list[dict[str, Any]] = []
    wt = worktree_dir(cfg, tag)
    root = wt / "tasks"
    if root.is_dir():
        for d in sorted(root.iterdir()):
            lock = read_json(d / "suite.lock.json")
            if d.is_dir() and lock is not None:
                out.append(_suite_entry(d.name, lock, read_json(d / "suite.json")))
        return out
    listing = _git(REPO_ROOT, "ls-tree", "--name-only", f"refs/tags/{tag}", "tasks/")
    for line in (listing or "").splitlines():
        name = line.rstrip("/").rsplit("/", 1)[-1]
        if not name or not _LABEL_RE.match(name):
            continue
        text = _git(REPO_ROOT, "show", f"refs/tags/{tag}:tasks/{name}/suite.lock.json")
        if text is None:
            continue
        try:
            lock = json.loads(text)
        except json.JSONDecodeError:
            continue
        manifest_text = _git(REPO_ROOT, "show", f"refs/tags/{tag}:tasks/{name}/suite.json")
        manifest: dict[str, Any] | None = None
        if manifest_text:
            try:
                manifest = json.loads(manifest_text)
            except json.JSONDecodeError:
                manifest = None
        if isinstance(lock, dict):
            out.append(_suite_entry(name, lock, manifest))
    return out


def harness_features(worktree: Path) -> dict[str, bool]:
    """What this tag's harness CLI supports (older tags predate the GUI changes)."""
    try:
        text = (worktree / "harness" / "cli.py").read_text(encoding="utf-8", errors="replace")
    except OSError:
        return {"present": False, "max_infra_retries": False, "doctor": False}
    return {
        "present": True,
        "max_infra_retries": "--max-infra-retries" in text,
        "doctor": "def harness_doctor" in text,
    }


def suite_options(cfg: RunnerConfig, tag: str | None) -> dict[str, Any]:
    return {
        "suites": suites_at(cfg, tag) if tag else [],
        "harnesses": list(HARNESSES),
        "efforts": efforts_offered(),
        "billing": list(BILLING_MODES),
        "sandboxes": list(SANDBOXES),
        "sandbox_defaults": dict(SANDBOX_DEFAULTS),
        "quota_wait_s": QUOTA_WAIT_S,
        "quota_continue_max": QUOTA_CONTINUE_MAX,
        "max_repeat": MAX_REPEAT,
        "harness_runs_dir": str(cfg.harness_runs_dir),
    }


# ----------------------------------------------------- quota-free checks --


def _venv_cli(worktree: Path) -> Path:
    return worktree / ".venv" / "bin" / "vulcanbench"


def _run_cli(
    worktree: Path, args: list[str], *, path_prepend: str = "", timeout: float = 90
) -> tuple[int | None, str, str]:
    cli = _venv_cli(worktree)
    if not cli.is_file():
        return None, "", f"{cli} is missing (run_worktree.sh did not finish)"
    try:
        proc = subprocess.run(
            [str(cli), *args],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
            cwd=str(worktree),
            env=child_env(path_prepend=path_prepend),
        )
    except subprocess.TimeoutExpired:
        return None, "", f"{cli.name} {args[0]} timed out after {timeout:.0f} s"
    except OSError as exc:
        return None, "", f"{cli.name} {args[0]} failed to start: {exc}"
    return proc.returncode, proc.stdout, proc.stderr


def _last_json(stdout: str) -> Any:
    text = stdout.strip()
    if not text:
        return None
    start = text.find("{") if text.lstrip().startswith("{") else text.find("[")
    if start < 0:
        return None
    try:
        return json.loads(text[start:])
    except json.JSONDecodeError:
        return None


def run_doctor(worktree: Path, harness: str, *, path_prepend: str = "") -> dict[str, Any]:
    """``vulcanbench harness doctor <harness> --json``: installed and logged in, no model call."""
    if harness == "vulcan":
        return {
            "status": "not_applicable",
            "note": "Vulcan's API loop reads the provider key from the worktree's .env at start",
        }
    if not harness_features(worktree)["doctor"]:
        return {"status": "unavailable", "note": "this tag's harness has no doctor command"}
    code, out, err = _run_cli(
        worktree, ["harness", "doctor", harness, "--json"], path_prepend=path_prepend, timeout=120
    )
    data = _last_json(out)
    rows = [r for r in data if isinstance(r, dict)] if isinstance(data, list) else []
    if not rows:
        note = (err.strip().splitlines() or out.strip().splitlines() or ["no report"])[-1]
        return {"status": "error", "note": note[:300], "exit_code": code}
    row = rows[0]
    return {
        "status": "ready" if row.get("ready") else "not_ready",
        "harness": row.get("harness"),
        "version": row.get("version"),
        "auth_mode": row.get("auth_mode"),
        "plan_name": row.get("plan_name"),
        "detail": row.get("detail"),
        "exit_code": code,
    }


def run_estimate(
    worktree: Path, suite: str, model_spec: str, repeat: int, *, path_prepend: str = ""
) -> dict[str, Any]:
    """``vulcanbench estimate --json`` for one model and suite (list prices, bundled priors)."""
    code, out, err = _run_cli(
        worktree,
        [
            "estimate",
            "--json",
            "--suite",
            suite,
            "--model",
            model_spec,
            "--repeat",
            str(repeat),
            "--no-judges",
        ],
        path_prepend=path_prepend,
    )
    data = _last_json(out)
    if not isinstance(data, dict) or not data.get("models"):
        line = (out.strip().splitlines() or err.strip().splitlines() or ["no estimate"])[-1]
        status = "no_pricing" if "no built-in pricing" in line else "unavailable"
        return {"status": status, "note": line[:300], "exit_code": code}
    m = data["models"][0]
    return {
        "status": "ok",
        "n_runs": m.get("n_runs"),
        "low_usd": m.get("low_usd"),
        "mid_usd": m.get("mid_usd"),
        "high_usd": m.get("high_usd"),
        "recommended_usd": m.get("recommended_usd"),
        "confidence": m.get("confidence"),
        "notes": list(m.get("notes") or []),
        "env_var": m.get("env_var"),
    }


# --------------------------------------------------------------- plans --


def model_spec(model: str, harness: str) -> str:
    return model if harness == "vulcan" else f"{harness}:{model}"


def sweep_name(form: SuiteForm, today: str) -> str:
    if form.label:
        return form.label
    model = re.sub(r"[^A-Za-z0-9.]+", "-", form.model.rsplit(":", 1)[-1]).strip("-").lower()
    suite = {"coding-intelligence-index-v4": "frontier-v4"}.get(form.suite, form.suite)
    return f"{model}-{suite}-{today}"


def _check_form(form: SuiteForm) -> None:  # noqa: PLR0912
    if not _MODEL_RE.match(form.model) or len(form.model) > 120:
        raise LaunchError(422, "model id has characters vulcanbench would not accept")
    if form.harness not in HARNESSES:
        raise LaunchError(422, f"unknown harness {form.harness!r}; one of {', '.join(HARNESSES)}")
    if form.harness == "vulcan" and ":" not in form.model:
        raise LaunchError(
            422, "the vulcan harness needs provider:model (for example openai:gpt-6.1-terra)"
        )
    if form.harness != "vulcan" and ":" in form.model:
        raise LaunchError(
            422, "give a bare model id with a CLI harness (the harness is the provider)"
        )
    if form.billing not in BILLING_MODES:
        raise LaunchError(422, "billing must be auto, subscription or api")
    if form.billing == "subscription" and form.harness == "vulcan":
        raise LaunchError(422, "subscription billing needs a CLI harness")
    if not form.efforts:
        raise LaunchError(422, "pick at least one effort level")
    offered = efforts_offered()
    for level in form.efforts:
        if level in blocked_efforts():
            raise LaunchError(
                422, f"effort {level!r} is blocked by vulcanbench.toml and never runs"
            )
        if level not in offered:
            raise LaunchError(422, f"unknown effort level {level!r}")
    if len(set(form.efforts)) != len(form.efforts):
        raise LaunchError(422, "an effort level is listed twice")
    if not 1 <= form.repeat <= MAX_REPEAT:
        raise LaunchError(422, f"repeat must be 1 to {MAX_REPEAT}")
    if form.sandbox not in (None, *SANDBOXES):
        raise LaunchError(422, "sandbox must be local or docker")
    if not 0 <= form.quota_continue <= QUOTA_CONTINUE_MAX:
        raise LaunchError(422, f"quota continuation must be 0 to {QUOTA_CONTINUE_MAX}")
    if form.label and (not _LABEL_RE.match(form.label) or len(form.label) > 40):
        raise LaunchError(422, "label: letters, digits, dots, dashes, at most 40")
    if form.cli_bin_dir:
        p = Path(form.cli_bin_dir).expanduser()
        if not p.is_absolute() or not p.is_dir():
            raise LaunchError(422, "CLI bin dir must be an existing absolute directory")
    if not re.match(r"^bench/[A-Za-z0-9._-]+$", form.tag):
        raise LaunchError(422, "tag must look like bench/<label>")
    if tag_commit(form.tag) is None:
        raise LaunchError(422, f"no tag {form.tag}")


def _level_argv(
    form: SuiteForm, *, cli: Path, level: str, out_dir: Path, sandbox: str, retries_flag: bool
) -> list[str]:
    argv = [
        str(cli),
        "run",
        "--suite",
        form.suite,
        "--model",
        form.model,
        "--harness",
        form.harness,
        "--billing",
        form.billing,
        "--effort",
        level,
        "--repeat",
        str(form.repeat),
        "--sandbox",
        sandbox,
        "--no-judges",
        "--max-concurrency",
        "1",
    ]
    if retries_flag:
        argv += ["--max-infra-retries", "0"]
    if form.only_missing:
        argv.append("--only-missing")
    argv += ["-o", str(out_dir)]
    return argv


def build_suite_plan(cfg: RunnerConfig, form: SuiteForm, *, today: str) -> dict[str, Any]:  # noqa: PLR0912
    _check_form(form)
    wt_info = worktree_status(cfg, form.tag)
    wt = Path(wt_info["path"])
    if not wt_info["exists"]:
        raise LaunchError(
            409,
            f"{wt} does not exist yet; prepare the worktree first (a quota-free queue step), "
            "then build the plan so the estimate and the doctor run against that tag",
        )
    if wt_info["problems"]:
        raise LaunchError(409, "worktree not usable: " + "; ".join(wt_info["problems"]))
    suites = {s["id"]: s for s in suites_at(cfg, form.tag)}
    if form.suite not in suites:
        raise LaunchError(
            422, f"{form.tag} has no frozen suite {form.suite!r} (tasks/<suite>/suite.lock.json)"
        )
    suite = suites[form.suite]
    name = sweep_name(form, today)
    out_root = cfg.harness_runs_dir / name
    sandbox = form.sandbox or SANDBOX_DEFAULTS[form.harness]
    features = harness_features(wt)
    spec_model = model_spec(form.model, form.harness)
    path_prepend = str(Path(form.cli_bin_dir).expanduser()) if form.cli_bin_dir else ""
    doctor = run_doctor(wt, form.harness, path_prepend=path_prepend)
    estimate = run_estimate(wt, form.suite, spec_model, form.repeat, path_prepend=path_prepend)

    warnings: list[str] = []
    if estimate["status"] == "no_pricing":
        warnings.append(
            f"{form.tag} has no list price for {spec_model}: the model is not onboarded at this tag "
            "(harness/pricing.py). Cost cannot be estimated and run summaries will carry no cost."
        )
    elif estimate["status"] != "ok":
        warnings.append(f"estimate unavailable: {estimate.get('note')}")
    if doctor["status"] == "not_ready":
        warnings.append(f"harness doctor says {form.harness} is not ready: {doctor.get('detail')}")
    elif doctor["status"] == "error":
        warnings.append(f"harness doctor failed: {doctor.get('note')}")
    if not features["max_infra_retries"]:
        warnings.append(
            "this tag's harness has no --max-infra-retries: it re-queues a unit up to 2 times "
            "after an infrastructure error on its own"
        )
    if out_root.exists():
        existing = sorted(p.name for p in out_root.iterdir() if p.is_dir())
        if form.only_missing:
            warnings.append(
                f"{out_root} exists ({', '.join(existing) or 'empty'}); tasks with a fresh result "
                "there are skipped (--only-missing), so this plan resumes that sweep"
            )
        else:
            warnings.append(
                f"{out_root} exists; without --only-missing every task runs again beside the old results"
            )
    if form.quota_continue:
        warnings.append(
            f"after a quota or rate-limit refusal the level waits {QUOTA_WAIT_S // 60} minutes and "
            f"continues with --only-missing, at most {form.quota_continue} time(s); no other failure continues"
        )
    if form.harness == "vulcan" and form.billing != "subscription":
        warnings.append("metered API billing: the provider key comes from the worktree's .env")

    jobs: list[dict[str, Any]] = []
    sweep_id = f"{name}-{tag_commit(form.tag) or 'x'}"[:80]
    for level in form.efforts:
        out_dir = out_root / level
        argv = _level_argv(
            form,
            cli=_venv_cli(wt),
            level=level,
            out_dir=out_dir,
            sandbox=sandbox,
            retries_flag=features["max_infra_retries"],
        )
        jobs.append(
            {
                "kind": "suite",
                "job_name": f"{name}-{level}",
                "sweep_id": sweep_id,
                "sweep_name": name,
                "model": form.model,
                "model_spec": spec_model,
                "harness": form.harness,
                "agent": form.harness,
                "billing": form.billing,
                "suite": form.suite,
                "suite_name": suite["name"],
                "task_ids": list(suite["task_ids"]),
                "n_tasks": suite["n_tasks"],
                "effort": level,
                "repeat": form.repeat,
                "only_missing": form.only_missing,
                "sandbox": sandbox,
                "tag": form.tag,
                "tag_commit": wt_info["tag_commit"],
                "worktree": str(wt),
                "needs_prep": False,
                "argv": argv,
                "argv_token": f"-o {out_dir}",
                "cwd": str(wt),
                "env": {},
                "path_prepend": path_prepend,
                "out_dir": str(out_dir),
                "quota_continue": {"max": form.quota_continue, "wait_s": QUOTA_WAIT_S, "used": 0},
                "harness_features": features,
            }
        )
    n_levels = len(form.efforts)
    runs_per_level = suite["n_tasks"] * form.repeat
    per_level_usd = estimate.get("recommended_usd") if estimate["status"] == "ok" else None
    spec: dict[str, Any] = {
        "kind": "sweep",
        "job_name": name,
        "sweep_id": sweep_id,
        "name": name,
        "model": form.model,
        "model_spec": spec_model,
        "harness": form.harness,
        "agent": form.harness,
        "billing": form.billing,
        "suite": form.suite,
        "suite_name": suite["name"],
        "n_tasks": suite["n_tasks"],
        "efforts": list(form.efforts),
        "repeat": form.repeat,
        "only_missing": form.only_missing,
        "sandbox": sandbox,
        "sandbox_default": SANDBOX_DEFAULTS[form.harness],
        "quota_continue": {"max": form.quota_continue, "wait_s": QUOTA_WAIT_S},
        "tag": form.tag,
        "tag_commit": wt_info["tag_commit"],
        "worktree": str(wt),
        "needs_prep": False,
        "cwd": str(wt),
        "env": {},
        "path_prepend": path_prepend,
        "env_passthrough": [n for n in passthrough_names() if os.environ.get(n)],
        "out_root": str(out_root),
        "jobs": jobs,
        "doctor": doctor,
        "estimate": {
            "status": estimate["status"],
            "note": estimate.get("note"),
            "confidence": estimate.get("confidence"),
            "notes": estimate.get("notes") or [],
            "per_level_usd": per_level_usd,
            "per_level_low_usd": estimate.get("low_usd"),
            "per_level_high_usd": estimate.get("high_usd"),
            "total_usd": round(per_level_usd * n_levels, 2) if per_level_usd is not None else None,
            "levels": n_levels,
            "runs_per_level": runs_per_level,
            "total_runs": runs_per_level * n_levels,
            "task_timeout_s": TASK_TIMEOUT_S,
            "max_wall_s": runs_per_level * n_levels * TASK_TIMEOUT_S,
            "billing": _billing_text(form),
            "infra_retries": (
                "none: --max-infra-retries 0, an infrastructure error leaves the task without a result"
                if features["max_infra_retries"]
                else "the harness at this tag re-queues infrastructure errors up to 2 times"
            ),
            "quota": (
                f"continues after a quota window, up to {form.quota_continue} time(s)"
                if form.quota_continue
                else "a quota window ends the level; continue by hand from the Launch page"
            ),
        },
        "harness_features": features,
        "usage_check_required": False,
        "warnings": warnings,
    }
    return spec


def _billing_text(form: SuiteForm) -> str:
    if form.billing == "subscription":
        return f"{form.harness} subscription login (no metered API)"
    if form.billing == "api":
        return "metered API key (list price in the estimate)"
    return "auto: the subscription login when the harness has one, else the API key"


def passthrough_names() -> tuple[str, ...]:
    return (*ENV_PASSTHROUGH, *SUITE_ENV_PASSTHROUGH)


def validate_any(cfg: RunnerConfig, spec: dict[str, Any]) -> list[str]:
    """Re-validate a job of any kind right before confirm or launch (the one dispatcher)."""
    kind = spec.get("kind")
    if kind == "suite":
        return validate_suite_launch(cfg, spec)
    if kind == "prepare":
        return launch.validate_before_launch(cfg, spec)
    return ["unsupported kind"]


def validate_suite_launch(cfg: RunnerConfig, spec: dict[str, Any]) -> list[str]:  # noqa: PLR0912
    """Re-check one level's job right before its child starts (confirm and launch time)."""
    problems: list[str] = []
    tag = str(spec.get("tag") or "")
    problems.extend(validate_worktree(cfg, tag))
    wt = Path(str(spec.get("worktree")))
    if wt != worktree_dir(cfg, tag):
        problems.append("worktree path does not match the tag")
    argv = [str(a) for a in (spec.get("argv") or [])]
    if len(argv) < 6 or Path(argv[0]) != _venv_cli(wt) or argv[1] != "run":
        problems.append("argv is not the worktree's vulcanbench run")
    if "--allow-dirty" in argv:
        problems.append("argv contains --allow-dirty")
    if "--effort" in argv:
        level = argv[argv.index("--effort") + 1] if argv.index("--effort") + 1 < len(argv) else ""
        if level in blocked_efforts():
            problems.append(f"effort {level!r} is blocked by vulcanbench.toml")
    else:
        problems.append("argv has no --effort")
    if "-o" in argv:
        out = Path(argv[argv.index("-o") + 1]) if argv.index("-o") + 1 < len(argv) else Path()
        try:
            out.resolve().relative_to(cfg.harness_runs_dir.resolve())
        except ValueError:
            problems.append("output directory is outside the runner's runs root")
        if Path(str(spec.get("out_dir"))) != out:
            problems.append("out_dir does not match argv")
    else:
        problems.append("argv has no -o")
    if "--judges" in argv:
        problems.append("argv enables the judge ensemble")
    for name in spec.get("env") or {}:
        problems.append(f"unexpected env override {name}")
    prepend = str(spec.get("path_prepend") or "")
    if prepend and not Path(prepend).is_dir():
        problems.append(f"CLI bin dir {prepend} is gone")
    return problems


# ------------------------------------------------------------ progress --


def _task_of(run_dir: Path) -> str:
    m = _RUN_ID_RE.match(run_dir.name)
    return m.group("task") if m else run_dir.name


def progress(spec: dict[str, Any], now: datetime) -> dict[str, Any]:  # noqa: PLR0912, PLR0915
    """What the output directory says about one level: done, passed, cost, the current task."""
    out_dir = Path(str(spec.get("out_dir") or ""))
    task_ids = [str(t) for t in (spec.get("task_ids") or [])]
    repeat = int(spec.get("repeat") or 1)
    total_runs = len(task_ids) * repeat
    done: dict[str, list[dict[str, Any]]] = {}
    unfinished: list[dict[str, Any]] = []
    if out_dir.is_dir():
        for d in sorted(out_dir.iterdir()):
            if not d.is_dir() or d.name.startswith("suite-"):
                continue
            summary = read_json(d / "summary.json")
            if summary is not None:
                task = str(summary.get("task_id") or _task_of(d))
                done.setdefault(task, []).append(summary)
                continue
            trace = d / "trace.jsonl"
            if not trace.is_file():
                continue
            started = _birth(trace) or _mtime(d)
            last = _mtime(trace)
            unfinished.append(
                {
                    "run_id": d.name,
                    "task_id": _task_of(d),
                    "started_at": started.isoformat() if started else None,
                    "elapsed_s": (now - started).total_seconds() if started else None,
                    "quiet_s": (now - last).total_seconds() if last else None,
                    "path": str(d),
                }
            )
    wanted = set(task_ids)
    runs_done = sum(min(len(v), repeat) for t, v in done.items() if t in wanted)
    tasks_done = sum(1 for t in done if t in wanted)
    passed = 0
    cost = 0.0
    cost_known = False
    durations: list[float] = []
    for task, summaries in done.items():
        if task not in wanted:
            continue
        for s in summaries[:repeat]:
            scores: dict[str, Any] = s["scores"] if isinstance(s.get("scores"), dict) else {}
            if scores.get("functional") == 1.0:
                passed += 1
            c = s.get("cost_usd")
            if isinstance(c, (int, float)):
                cost += float(c)
                cost_known = True
            dur = s.get("duration_s")
            if isinstance(dur, (int, float)) and dur > 0:
                durations.append(float(dur))
            elif parse_dt(s.get("started_at")) and parse_dt(s.get("finished_at")):
                durations.append(
                    (parse_dt(s["finished_at"]) - parse_dt(s["started_at"])).total_seconds()  # type: ignore[operator]
                )
    unfinished.sort(key=lambda u: u.get("started_at") or "", reverse=True)
    current = unfinished[0] if unfinished else None
    if current is not None and current.get("elapsed_s") is not None:
        current["budget_fraction"] = min(1.0, float(current["elapsed_s"]) / TASK_TIMEOUT_S)
    remaining = max(0, total_runs - runs_done)
    avg = statistics.fmean(durations) if durations else None
    eta_s = None
    if avg is not None and remaining:
        eta_s = remaining * avg
        if current is not None and current.get("elapsed_s") is not None:
            eta_s = max(0.0, eta_s - min(float(current["elapsed_s"]), avg))
    missing = [t for t in task_ids if t not in done]
    return {
        "out_dir": str(out_dir),
        "n_tasks": len(task_ids),
        "repeat": repeat,
        "total_runs": total_runs,
        "runs_done": runs_done,
        "tasks_done": tasks_done,
        "passed": passed,
        "pass_rate": (passed / runs_done) if runs_done else None,
        "cost_usd": round(cost, 4) if cost_known else None,
        "avg_run_s": avg,
        "eta_s": eta_s,
        "current": current,
        "stalled": bool(
            current and current.get("quiet_s") is not None and current["quiet_s"] > STALE_S
        ),
        "unfinished": len(unfinished),
        "missing": missing[:50],
        "complete": runs_done >= total_runs > 0,
    }


def classify_exit(spec: dict[str, Any], console_log: Path | None, now: datetime) -> str | None:
    """``"quota"`` or ``"auth"`` when the last run's output shows a refusal, else None."""
    texts: list[str] = []
    if console_log and console_log.is_file():
        texts.append(_tail(console_log))
    out_dir = Path(str(spec.get("out_dir") or ""))
    if out_dir.is_dir():
        recent = []
        for d in out_dir.iterdir():
            if not d.is_dir() or (d / "summary.json").exists():
                continue
            stream = d / "cli-agent-stream.jsonl"
            if stream.is_file():
                recent.append((stream.stat().st_mtime, stream))
        for _, stream in sorted(recent, reverse=True)[:3]:
            texts.append(_tail(stream))
    blob = "\n".join(texts)
    if AUTH_RE.search(blob):
        return "auth"
    if QUOTA_RE.search(blob):
        return "quota"
    return None


def _tail(path: Path, max_bytes: int = 32_000) -> str:
    try:
        with path.open("rb") as fh:
            fh.seek(0, os.SEEK_END)
            size = fh.tell()
            fh.seek(max(0, size - max_bytes))
            return fh.read().decode("utf-8", errors="replace")
    except OSError:
        return ""
