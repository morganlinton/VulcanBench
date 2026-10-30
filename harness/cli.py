"""VulcanBench CLI entrypoint (argparse, stdlib only).

Commands: run, estimate, effort-sweep, compare, regrade, leaderboard, report,
calibrate, replay, validate-task, list-tasks, audit-runs, plus the ``harness``
and ``voice`` command groups. See ``vulcanbench --help``.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from dotenv import load_dotenv

from harness import __version__
from harness.agent.cli_agents import (
    CLI_AGENT_PROVIDERS,
    get_cli_agent_adapter,
    list_cli_agent_adapters,
)
from harness.agent.loop import run_agent
from harness.agent.providers import ProviderError
from harness.calibration import calibrate_tasks, calibration_to_markdown
from harness.compare import build_matrix
from harness.console import (
    add_command,
    add_group,
    echo,
    existing_dir,
    existing_path,
    print_table,
    run_parser,
)
from harness.cost_estimate import estimate_plan
from harness.effort import DEFAULT_SWEEP_EFFORTS, parse_efforts
from harness.leaderboard import aggregate_by_model, scan_leaderboard
from harness.pricing import is_priced
from harness.provenance import (
    ALLOW_DIRTY_ENV,
    DirtyTreeError,
    check_clean_for_suite,
    source_provenance,
)
from harness.regrade import find_run_dirs, regrade_run
from harness.report import build_report, to_markdown
from harness.sandbox.docker_executor import ResourceSpec, SandboxError
from harness.settings import BlockedEffortError, check_effort_allowed
from harness.suite import SUITE_ALIASES, load_suite, run_suite
from harness.tasks import list_task_ids
from harness.validate import main as validate_main
from harness.verifier import VerifierInfrastructureError
from harness.voice.cli import register_voice_commands

# Provider keys and harness settings are read straight from os.environ, so
# load the nearest .env first (see .env.example). Real environment variables
# win: load_dotenv does not override anything already set in the shell.
load_dotenv()

_TASKS_V1 = Path("tasks/v1")
_RUNS_DIR = Path("./runs")
_TASKS_BASE = Path("tasks")
_DEFAULT_EFFORTS = ",".join(DEFAULT_SWEEP_EFFORTS)


def _execution_spec(model: str, harness_name: str, billing: str) -> str:
    """Normalize explicit harness/access options to the legacy model spec."""
    harness_name = harness_name.strip().lower()
    billing = billing.strip().lower()
    if billing not in {"auto", "api", "subscription"}:
        raise ValueError("--billing must be auto|api|subscription")
    prefix = model.partition(":")[0].strip().lower() if ":" in model else ""
    if harness_name == "vulcan" and prefix in CLI_AGENT_PROVIDERS:
        harness_name = prefix
    if harness_name == "vulcan":
        if billing == "subscription":
            raise ValueError(
                "--billing subscription requires --harness "
                "claude-code|codex|cursor|grok-build|zcode|muse-code|devin|pi|dsh|mirror"
            )
        return model
    if harness_name not in CLI_AGENT_PROVIDERS:
        known = ", ".join(["vulcan", *sorted(CLI_AGENT_PROVIDERS)])
        raise ValueError(f"unknown --harness {harness_name!r}; known: {known}")
    if billing == "api":
        raise ValueError(
            f"--harness {harness_name} is subscription-backed; use --billing subscription"
        )
    if prefix in CLI_AGENT_PROVIDERS:
        if prefix != harness_name:
            raise ValueError(
                f"model spec selects {prefix!r} but --harness selects {harness_name!r}"
            )
        return model
    return f"{harness_name}:{model}"


def harness_list(
    *,
    json_output: bool = False,
) -> None:
    """List subscription harnesses and their adapter capabilities."""
    data = [adapter.capabilities().as_summary() for adapter in list_cli_agent_adapters()]
    if json_output:
        print(json.dumps(data, indent=2))
        return
    print_table(
        ("Harness", "Executable", "Events", "Tokens", "Effort", "Sandbox"),
        [
            (
                row["harness"],
                row["executable"],
                "yes" if row["structured_events"] else "no",
                "yes" if row["reports_tokens"] else "no",
                "yes" if row["supports_effort"] else "no",
                row["sandbox"],
            )
            for row in data
        ],
        title="VulcanBench subscription harnesses",
    )


def harness_doctor(
    *,
    name: str | None = None,
    json_output: bool = False,
) -> None:
    """Check installation and subscription auth without launching a model run."""
    try:
        adapters = [get_cli_agent_adapter(name)] if name else list_cli_agent_adapters()
    except ValueError as exc:
        echo(f"error {exc}")
        raise SystemExit(1) from exc
    data = [adapter.preflight().as_summary() for adapter in adapters]
    if json_output:
        print(json.dumps(data, indent=2))
    else:
        print_table(
            ("Harness", "Version", "Auth", "Plan", "Ready", "Detail"),
            [
                (
                    row["harness"],
                    str(row["version"] or "not installed"),
                    str(row["auth_mode"] or "none"),
                    str(row["plan_name"] or ""),
                    "yes" if row["ready"] else "no",
                    str(row["detail"] or ""),
                )
                for row in data
            ],
            title="VulcanBench harness doctor",
        )
    if any(not row["ready"] for row in data):
        raise SystemExit(2)


def run(  # noqa: PLR0912, PLR0915, CLI entry: option declarations + linear guards + dispatch
    *,
    task: str | None = None,
    suite: str | None = None,
    tasks_root: Path = _TASKS_V1,
    model: str,
    harness_name: str = "vulcan",
    billing: str = "auto",
    output_dir: Path = _RUNS_DIR,
    max_steps: int | None = None,
    judges: bool = True,
    judge_model: str | None = None,
    sandbox: str = "docker",
    image: str | None = None,
    network: bool = False,
    mem_floor: str | None = None,
    mem_ceiling: str = "2g",
    cpu_floor: float | None = None,
    cpu_ceiling: float = 2.0,
    pids_limit: int = 512,
    agent_container: bool = False,
    repeat: int = 1,
    max_concurrency: int = 1,
    max_cost: float | None = None,
    max_run_cost: float | None = None,
    only_missing: bool = False,
    timeout: float | None = None,
    override_budgets: bool = False,
    fail_under: float | None = None,
    effort: str | None = None,
    dry_run: bool = False,
    allow_dirty: bool = False,
    use_priors: bool = True,
    max_infra_retries: int = 2,
) -> None:
    """Run an agent against a task or a whole suite, recording full traces."""
    try:
        check_effort_allowed(effort)
    except BlockedEffortError as exc:
        echo(f"error {exc}")
        raise SystemExit(1) from exc
    try:
        model = _execution_spec(model, harness_name, billing)
    except ValueError as exc:
        echo(f"error {exc}")
        raise SystemExit(1) from exc
    if (task is None) == (suite is None):
        echo("error pass exactly one of --task or --suite")
        raise SystemExit(1)
    if repeat < 1:
        echo("error --repeat must be >= 1")
        raise SystemExit(1)
    if max_concurrency < 1:
        echo("error --max-concurrency must be >= 1")
        raise SystemExit(1)
    if fail_under is not None and not (math.isfinite(fail_under) and 0.0 <= fail_under <= 1.0):
        echo(f"error --fail-under must be a finite number in [0, 1], got {fail_under}")
        raise SystemExit(1)
    if max_cost is not None:
        if suite is None:
            echo("error --max-cost applies to suite runs (use --suite)")
            raise SystemExit(1)
        if not (math.isfinite(max_cost) and max_cost > 0):
            echo(f"error --max-cost must be a finite number > 0, got {max_cost}")
            raise SystemExit(1)
        judge = judge_model or model
        unpriced = [m for m, used in ((model, True), (judge, judges)) if used and not is_priced(m)]
        if unpriced:
            names = ", ".join(sorted(set(unpriced)))
            echo(f"error --max-cost needs priced models; no price for {names}")
            raise SystemExit(1)
    if max_run_cost is not None:
        if not (math.isfinite(max_run_cost) and max_run_cost > 0):
            echo(f"error --max-run-cost must be a finite number > 0, got {max_run_cost}")
            raise SystemExit(1)
        if not is_priced(model):
            echo(f"error --max-run-cost needs a priced model; no price for {model}")
            raise SystemExit(1)
    if only_missing and suite is None:
        echo("error --only-missing applies to suite runs (use --suite)")
        raise SystemExit(1)
    if sandbox not in {"local", "docker", "auto"}:
        echo(f"error --sandbox must be local|docker|auto, got {sandbox!r}")
        raise SystemExit(1)
    if task is not None and task not in list_task_ids(tasks_root):
        available = ", ".join(list_task_ids(tasks_root)) or "(none)"
        echo(f"unknown task {task!r}. Available: {available}")
        raise SystemExit(1)
    if dry_run:
        target = f"suite={suite}" if suite else f"task={task}"
        effort_note = f" effort={effort}" if effort else ""
        echo(f"dry-run would run {target} model={model} sandbox={sandbox}{effort_note}")
        if is_priced(model):
            try:
                if suite is not None:
                    task_ids = load_suite(suite).task_ids
                else:
                    assert task is not None  # one of --task/--suite is required
                    task_ids = [task]
                plan = estimate_plan(
                    models=[model],
                    task_ids=task_ids,
                    repeat=repeat,
                    judges=judges,
                    runs_dir=output_dir,
                    use_priors=use_priors,
                )
                _print_cost_estimate(plan)
            except ValueError as e:
                echo(f"cost estimate skipped: {e}")
        raise SystemExit(0)
    _guard_source_tree(suite_run=suite is not None, allow_dirty=allow_dirty)

    run_kwargs = {
        "max_steps": max_steps,
        "judges": judges,
        "judge_model": judge_model,
        "sandbox": sandbox,
        "image": image,
        "network": network,
        "timeout_s": timeout,
        "effort": effort,
        "max_run_cost": max_run_cost,
        "override_budgets": override_budgets,
        "agent_container": agent_container,
        "resources": ResourceSpec(
            mem_floor=mem_floor,
            mem_ceiling=mem_ceiling,
            cpu_floor=cpu_floor,
            cpu_ceiling=cpu_ceiling,
            pids_limit=pids_limit,
        ),
    }
    pass_at_1: float | None = None
    n_incomplete = 0  # errored + skipped runs (an incomplete suite)
    try:
        if suite is not None:
            pass_at_1, n_incomplete = _run_suite(
                suite,
                model,
                output_dir,
                run_kwargs,
                repeat,
                max_concurrency,
                max_cost,
                only_missing,
                max_infra_retries=max_infra_retries,
            )
        else:
            pass_at_1, n_incomplete = _run_single(
                task,  # type: ignore[arg-type]
                model,
                output_dir,
                run_kwargs,
                repeat,
                tasks_root=tasks_root,
            )
    except SandboxError as e:
        echo(f"sandbox error {e}")
        raise SystemExit(3) from e
    except ProviderError as e:
        echo(f"provider error {e}")
        raise SystemExit(2) from e
    except VerifierInfrastructureError as e:
        echo(f"verifier infrastructure error {e}")
        raise SystemExit(3) from e
    except (ValueError, FileNotFoundError) as e:
        echo(f"error {e}")
        raise SystemExit(1) from e

    if fail_under is not None:
        # Gate semantics: pass iff every unit ran AND pass@1 >= threshold.
        # An errored OR budget-skipped run, or an unavailable pass@1, fails
        # closed, a CI gate must never go green on a partial/unknown result.
        if n_incomplete:
            echo(f"FAIL {n_incomplete} run(s) errored or skipped, gate fails closed")
            raise SystemExit(4)
        if pass_at_1 is None or pass_at_1 < fail_under:
            echo(f"FAIL pass@1={pass_at_1} < --fail-under {fail_under}")
            raise SystemExit(4)
        echo(f"PASS pass@1={pass_at_1} >= --fail-under {fail_under}")


def _guard_source_tree(*, suite_run: bool, allow_dirty: bool) -> None:
    """Refuse a suite run from uncommitted scoring-relevant changes; warn on a task run."""
    if allow_dirty:
        # Exported so every run summary in this process records the override.
        os.environ[ALLOW_DIRTY_ENV] = "1"
    if suite_run:
        try:
            check_clean_for_suite(allow_dirty=allow_dirty)
        except DirtyTreeError as exc:
            echo(f"error {exc}")
            raise SystemExit(1) from exc
    elif source_provenance().dirty:
        echo(
            "warning uncommitted changes to scoring-relevant paths; "
            "this run records dirty: true and is not publishable"
        )


def _print_cost_estimate(plan: Any, *, json_output: bool = False) -> None:
    if json_output:
        print(json.dumps(plan.to_dict(), indent=2))
        return

    n_runs = sum(m.n_runs for m in plan.models)
    echo(
        f"\nCost estimate  {len(plan.task_ids)} tasks x {plan.repeat} repeat(s)"
        f" = {n_runs} run(s) per model" + ("  (judges on)" if plan.judges else "")
    )
    rows: list[tuple[str, ...]] = [
        (
            m.model,
            f"{m.provider}\n{m.env_var}",
            f"${m.low_usd:.2f}",
            f"${m.mid_usd:.2f}",
            f"${m.high_usd:.2f}",
            f"${m.recommended_usd:.2f}",
            m.confidence,
        )
        for m in plan.models
    ]
    if len(plan.models) > 1:
        rows.append(
            (
                "Total",
                "",
                f"${plan.low_usd:.2f}",
                f"${plan.mid_usd:.2f}",
                f"${plan.high_usd:.2f}",
                f"${plan.recommended_usd:.2f}",
                "",
            )
        )
    print_table(("Model", "Provider / env", "Low", "Mid", "High", "Load ≥", "Conf."), rows)
    for m in plan.models:
        for note in m.notes:
            echo(f"  • {m.model}: {note}")
    echo(
        "\nEstimates prefer local ./runs history; bundled priors fill gaps on fresh "
        "installs. Load the recommended amount per provider before starting."
    )


def estimate(
    *,
    task: str | None = None,
    suite: str | None = None,
    model: list[str],
    repeat: int = 1,
    judges: bool = True,
    runs_dir: Path = _RUNS_DIR,
    use_priors: bool = True,
    json_output: bool = False,
) -> None:
    """Estimate API spend before running a benchmark."""
    if (task is None) == (suite is None):
        echo("error pass exactly one of --task or --suite")
        raise SystemExit(1)
    if repeat < 1:
        echo("error --repeat must be >= 1")
        raise SystemExit(1)
    if task is not None and task not in list_task_ids():
        echo(f"unknown task {task!r}")
        raise SystemExit(1)

    try:
        if suite is not None:
            task_ids = load_suite(suite).task_ids
        else:
            assert task is not None  # one of --task/--suite is required
            task_ids = [task]
        plan = estimate_plan(
            models=model,
            task_ids=task_ids,
            repeat=repeat,
            judges=judges,
            runs_dir=runs_dir,
            use_priors=use_priors,
        )
    except (ValueError, FileNotFoundError) as e:
        echo(f"error {e}")
        raise SystemExit(1) from e

    _print_cost_estimate(plan, json_output=json_output)


def _summary_row(summary: dict[str, Any]) -> dict[str, Any]:
    """Build a leaderboard-shaped row from a run summary (for in-memory aggregation)."""
    sc = summary.get("scores", {})
    cli_agent = summary.get("cli_agent") or {}
    economics = summary.get("economics") or {}
    legacy_cost = summary.get("cost_usd")
    marginal_cash = economics.get("marginal_cash_usd")
    api_equivalent = economics.get("api_equivalent_cost_usd")
    if not economics and not cli_agent:
        marginal_cash = legacy_cost
        api_equivalent = legacy_cost
    elif not economics and cli_agent:
        api_equivalent = legacy_cost
    return {
        "run_id": summary.get("run_id"),
        "task_id": summary.get("task_id"),
        "model": summary.get("model"),
        "total": sc.get("total"),
        "functional": sc.get("functional"),
        "cost_usd": summary.get("cost_usd"),
        "execution_harness": cli_agent.get("harness") or "vulcan",
        "track": "subscription" if cli_agent else "api",
        "access_mode": economics.get("billing_mode"),
        "marginal_cash_usd": marginal_cash,
        "api_equivalent_cost_usd": api_equivalent,
        "duration_s": summary.get("duration_s"),
        "total_tokens": summary.get("total_tokens"),
        "effort": summary.get("effort"),
    }


def _run_single(
    task: str,
    model: str,
    output_dir: Path,
    run_kwargs: dict[str, Any],
    repeat: int,
    tasks_root: Path = Path("tasks/v1"),
) -> tuple[float | None, int]:
    """Run a single task ``repeat`` times; returns (pass@1, n_errors).

    A single run that fails raises out of here (caught by ``run``), so the
    error count is always 0 on this path.
    """
    summaries = []
    for _ in range(repeat):
        res = run_agent(
            task_id=task, model=model, output_dir=output_dir, tasks_root=tasks_root, **run_kwargs
        )
        summaries.append(res["summary"])

    agg = aggregate_by_model([_summary_row(s) for s in summaries])[0]
    if repeat == 1:
        scores = summaries[0].get("scores", {})
        economics = summaries[0].get("economics") or {}
        cash = economics.get("marginal_cash_usd")
        api_equivalent = economics.get("api_equivalent_cost_usd")
        echo(f"run complete {res['run_id']}")
        echo(
            f"functional={scores.get('functional')} quality={scores.get('quality')} "
            f"security={scores.get('security')} human_like={scores.get('human_like')} "
            f"total={scores.get('total')} marginal_cash=${cash} "
            f"api_equivalent=${api_equivalent}"
        )
        echo(f"replay: {res['replay']}")
    else:
        echo(f"{task} x{repeat} with {model}")
        echo(
            f"pass@1={agg['pass_at_1']}±{agg['pass_at_1_stderr']}  "
            f"pass@{repeat}={agg['pass_at_k']}  "
            f"avg_total={agg['avg_total']}±{agg['avg_total_stderr']}  cost=${agg['total_cost']}"
        )
    p1 = agg["pass_at_1"]
    return (float(p1) if p1 is not None else None), 0


def _run_suite(
    suite: str,
    model: str,
    output_dir: Path,
    run_kwargs: dict[str, Any],
    repeat: int,
    max_concurrency: int,
    max_cost: float | None = None,
    only_missing: bool = False,
    max_infra_retries: int = 2,
) -> tuple[float | None, int]:
    """Run a suite; returns (pass@1, n_incomplete) where n_incomplete counts
    errored + budget-skipped runs."""
    suffix = f" x{repeat}" if repeat > 1 else ""
    par = f" ({max_concurrency}-way parallel)" if max_concurrency > 1 else ""
    cap = f" (budget ${max_cost})" if max_cost is not None else ""
    resume = " (only-missing)" if only_missing else ""
    echo(f"running suite {suite}{suffix}{par}{cap}{resume} with {model} ...")
    result = run_suite(
        suite,
        model,
        output_dir=output_dir,
        repeat=repeat,
        max_concurrency=max_concurrency,
        max_cost=max_cost,
        only_missing=only_missing,
        max_infra_retries=max_infra_retries,
        **run_kwargs,
    )
    covered = result.get("covered_cached") or []
    if covered:
        echo(
            f"only-missing: reused {len(covered)} cached task(s), "
            f"ran {result.get('n_runs', 0)} new run(s)."
        )
    n_errors = len(result.get("errors") or [])
    n_skipped = result.get("n_skipped", 0)
    n_retried = result.get("n_infra_retries", 0)
    if n_retried:
        tasks = ", ".join(sorted({r["task_id"] for r in result.get("infra_retries") or []}))
        echo(f"retried {n_retried} run(s) after infrastructure errors: {tasks}")
    if n_errors:
        echo(f"{n_errors} run(s) errored")
    if n_skipped:
        echo(
            f"budget ${result.get('max_cost')} reached after "
            f"${result.get('budget_accounted_usd')} ({result.get('spend_basis')}); "
            f"skipped {n_skipped} run(s)"
        )
    for agg in result["aggregate"]:
        echo(
            f"{agg['model']}: pass@1={agg['pass_at_1']}±{agg['pass_at_1_stderr']} "
            f"pass@{result['repeat']}={agg['pass_at_k']} over {agg['n_tasks']} tasks "
            f"({agg['n_runs']} runs)  avg_total={agg['avg_total']} cost=${agg['total_cost']}"
        )
    aggregate = result["aggregate"]
    p1 = aggregate[0]["pass_at_1"] if aggregate else None
    return (float(p1) if p1 is not None else None), n_errors + n_skipped


def effort_sweep(  # noqa: PLR0912, PLR0915, CLI entry: validation + per-effort dispatch
    *,
    suite: str,
    model: str,
    efforts: str = _DEFAULT_EFFORTS,
    repeat: int = 1,
    output_dir: Path = _RUNS_DIR,
    max_steps: int | None = None,
    judges: bool = True,
    judge_model: str | None = None,
    sandbox: str = "docker",
    image: str | None = None,
    network: bool = False,
    max_concurrency: int = 1,
    max_cost: float | None = None,
    max_run_cost: float | None = None,
    only_missing: bool = False,
    timeout: float | None = None,
    dry_run: bool = False,
    allow_dirty: bool = False,
) -> None:
    """Run a suite across normalized reasoning-effort levels."""
    try:
        effort_list = parse_efforts(efforts)
        for level in effort_list:
            check_effort_allowed(level)
    except ValueError as e:
        echo(f"error {e}")
        raise SystemExit(1) from e

    if repeat < 1:
        echo("error --repeat must be >= 1")
        raise SystemExit(1)
    if max_concurrency < 1:
        echo("error --max-concurrency must be >= 1")
        raise SystemExit(1)
    if sandbox not in {"local", "docker", "auto"}:
        echo(f"error --sandbox must be local|docker|auto, got {sandbox!r}")
        raise SystemExit(1)
    if max_cost is not None and not (math.isfinite(max_cost) and max_cost > 0):
        echo(f"error --max-cost must be a finite number > 0, got {max_cost}")
        raise SystemExit(1)
    if max_run_cost is not None:
        if not (math.isfinite(max_run_cost) and max_run_cost > 0):
            echo(f"error --max-run-cost must be a finite number > 0, got {max_run_cost}")
            raise SystemExit(1)
        if not is_priced(model):
            echo(f"error --max-run-cost needs a priced model; no price for {model}")
            raise SystemExit(1)

    experiment_id = f"experiment-{uuid.uuid4().hex[:8]}"
    echo(f"effort sweep {suite} with {model}: {', '.join(effort_list)} x{repeat} ({experiment_id})")
    if dry_run:
        echo("dry-run would run one suite invocation per effort")
        raise SystemExit(0)
    _guard_source_tree(suite_run=True, allow_dirty=allow_dirty)

    suites: list[dict[str, Any]] = []
    started_at = datetime.now(UTC)
    try:
        for effort in effort_list:
            echo(f"effort {effort}")
            suites.append(
                run_suite(
                    suite,
                    model,
                    output_dir=output_dir,
                    repeat=repeat,
                    max_concurrency=max_concurrency,
                    max_cost=max_cost,
                    only_missing=only_missing,
                    effort=effort,
                    max_steps=max_steps,
                    judges=judges,
                    judge_model=judge_model,
                    sandbox=sandbox,
                    image=image,
                    network=network,
                    timeout_s=timeout,
                    max_run_cost=max_run_cost,
                    experiment_id=experiment_id,
                )
            )
    except SandboxError as e:
        echo(f"sandbox error {e}")
        raise SystemExit(3) from e
    except ProviderError as e:
        echo(f"provider error {e}")
        raise SystemExit(2) from e
    except VerifierInfrastructureError as e:
        echo(f"verifier infrastructure error {e}")
        raise SystemExit(3) from e
    except (ValueError, FileNotFoundError) as e:
        echo(f"error {e}")
        raise SystemExit(1) from e

    experiment = {
        "experiment_id": experiment_id,
        "suite": suite,
        "model": model,
        "efforts": effort_list,
        "repeat": repeat,
        "max_concurrency": max_concurrency,
        "max_cost_per_effort": max_cost,
        "started_at": started_at.isoformat(),
        "finished_at": datetime.now(UTC).isoformat(),
        "suites": suites,
    }
    experiment_dir = output_dir / experiment_id
    experiment_dir.mkdir(parents=True, exist_ok=True)
    (experiment_dir / "experiment.json").write_text(
        json.dumps(experiment, indent=2), encoding="utf-8"
    )
    echo(f"wrote {experiment_dir / 'experiment.json'}")


def leaderboard(  # noqa: PLR0912
    *,
    by: str = "model",
    format: str = "markdown",
    task: str | None = None,
    suite: str | None = None,
    track: str = "all",
) -> None:
    """Show API and subscription-product results without silently mixing them."""
    if track not in {"all", "api", "subscription"}:
        echo(f"error --track must be all|api|subscription, got {track!r}")
        raise SystemExit(1)
    rows = scan_leaderboard()
    if suite in SUITE_ALIASES:
        allowed = set(load_suite(suite).task_ids)
        rows = [r for r in rows if r.get("task_id") in allowed]
    elif suite:
        rows = [r for r in rows if r.get("suite") == suite]
    if track != "all":
        rows = [r for r in rows if r.get("track") == track]
    if by == "model":
        data: list[dict] = aggregate_by_model(rows)  # type: ignore[type-arg]
    else:
        data = rows
        if task:
            data = [r for r in data if r.get("task_id") == task]

    if format == "json":
        echo(json.dumps(data, indent=2))
        return
    if not data:
        echo(
            "no runs found in ./runs/ (try: vulcanbench run --task hello-world --model mock:synthetic)"
        )
        return

    table_rows: list[tuple[str, ...]] = []
    if by == "model":
        title = f"VulcanBench Leaderboard, by model ({track})"
        columns: tuple[str, ...] = (
            "Track",
            "Harness",
            "Model",
            "Tasks",
            "Runs",
            "pass@1 ± se",
            "pass@k",
            "AvgTotal",
            "Qual",
            "Sec",
            "Human",
            "Cash $",
            "API-eq $",
            "AvgTime",
        )
        for a in data:
            cash = (
                "?" if not a.get("marginal_cash_known") else f"{a.get('total_marginal_cash_usd')}"
            )
            api_equivalent = (
                "?"
                if not a.get("api_equivalent_known")
                else f"{a.get('total_api_equivalent_cost_usd')}"
            )
            table_rows.append(
                (
                    a["track"],
                    a["execution_harness"],
                    a["model"],
                    str(a["n_tasks"]),
                    str(a["n_runs"]),
                    f"{a['pass_at_1']} ± {a['pass_at_1_stderr']}",
                    str(a["pass_at_k"]),
                    str(a["avg_total"]),
                    str(a["avg_quality"]),
                    str(a["avg_security"]),
                    str(a["avg_human_like"]),
                    cash,
                    api_equivalent,
                    str(a["avg_duration_s"]),
                )
            )
    else:
        title = f"VulcanBench Leaderboard, by run ({track})"
        columns = (
            "Run ID",
            "Track",
            "Harness",
            "Task",
            "Model",
            "Total",
            "Functional",
            "Cash $",
            "API-eq $",
            "Time s",
        )
        for r in data:
            table_rows.append(
                (
                    r["run_id"],
                    str(r.get("track")),
                    str(r.get("execution_harness")),
                    r.get("task_id", "?"),
                    r.get("model", "?"),
                    str(r.get("total")),
                    str(r.get("functional")),
                    str(r.get("marginal_cash_usd")),
                    str(r.get("api_equivalent_cost_usd")),
                    str(r.get("duration_s")),
                )
            )
    print_table(columns, table_rows, title=title)


def report(
    *,
    suite: str | None = None,
    format: str = "md",
    output: Path | None = None,
    runs_dir: Path = _RUNS_DIR,
    tasks_root: Path = _TASKS_V1,
) -> None:
    """Generate a shareable results report (Markdown or JSON) from recorded runs."""
    if format not in {"md", "json"}:
        echo(f"error --format must be md|json, got {format!r}")
        raise SystemExit(1)
    rep = build_report(runs_dir=runs_dir, tasks_root=tasks_root, suite=suite)
    text = json.dumps(rep, indent=2) if format == "json" else to_markdown(rep)
    if output is not None:
        output.write_text(text, encoding="utf-8")
        echo(f"wrote {output} ({rep['totals']['n_runs']} runs)")
    else:
        # Report goes to stdout so it can be piped/redirected.
        print(text)


def calibrate(  # noqa: PLR0912
    *,
    suite: str | None = None,
    format: str = "table",
    output: Path | None = None,
    min_attempts: int = 5,
    min_models: int = 2,
    easy_min: float = 0.85,
    medium_min: float = 0.4,
    runs_dir: Path = _RUNS_DIR,
    tasks_root: Path = _TASKS_V1,
    include_mock: bool = False,
) -> None:
    """Calibrate empirical difficulty from recorded runs."""
    if format not in {"table", "md", "json"}:
        echo(f"error --format must be table|md|json, got {format!r}")
        raise SystemExit(1)
    if not (math.isfinite(easy_min) and math.isfinite(medium_min)):
        echo("error --easy-min and --medium-min must be finite")
        raise SystemExit(1)
    if not (0 <= medium_min < easy_min <= 1):
        echo(
            f"error require 0 <= medium_min < easy_min <= 1, "
            f"got medium_min={medium_min} easy_min={easy_min}"
        )
        raise SystemExit(1)
    if min_attempts < 1:
        echo(f"error --min-attempts must be >= 1, got {min_attempts}")
        raise SystemExit(1)
    if min_models < 1:
        echo(f"error --min-models must be >= 1, got {min_models}")
        raise SystemExit(1)

    rows = scan_leaderboard(runs_dir)
    if suite in SUITE_ALIASES:
        allowed = set(load_suite(suite).task_ids)
        rows = [r for r in rows if r.get("task_id") in allowed]
    elif suite:
        rows = [r for r in rows if r.get("suite") == suite]

    if not rows and not include_mock and format == "table":
        echo(
            f"no runs found in {runs_dir} "
            "(try: vulcanbench run --task hello-world --model openai:gpt-4o)"
        )
        return

    cal = calibrate_tasks(
        rows,
        tasks_root=tasks_root,
        min_attempts=min_attempts,
        min_models=min_models,
        easy_min=easy_min,
        medium_min=medium_min,
        include_mock=include_mock,
    )

    if format == "json":
        text = json.dumps(cal, indent=2)
    elif format == "md":
        text = calibration_to_markdown(cal)
    else:
        text = ""

    if format == "table":
        table_rows: list[tuple[str, ...]] = []
        for e in cal["tasks"]:
            sr = e["solve_rate"]
            se = e["solve_rate_stderr"]
            sr_str = f"{sr:.4f} ± {se:.4f}" if sr is not None else ""
            label = e["labeled_difficulty"] or ""
            emp = e["empirical_difficulty"] or ""
            if e["agreement"] is False:
                emp = f"{emp} (!)"  # the label and the data disagree
            table_rows.append(
                (
                    e["task_id"],
                    label,
                    str(e["n_attempts"]),
                    str(e["n_models"]),
                    sr_str,
                    emp,
                    e["status"],
                )
            )
        print_table(
            ("Task", "Label", "Attempts", "Models", "Solve rate ± se", "Empirical", "Status"),
            table_rows,
            title="VulcanBench Calibration",
        )
        s = cal["summary"]
        echo(
            f"{s['n_calibrated']} calibrated, {s['n_disagree']} disagreements, "
            f"{s['n_insufficient']} insufficient data (of {s['n_tasks']} tasks)"
        )
    elif output is not None:
        output.write_text(text, encoding="utf-8")
        echo(f"wrote {output}")
    else:
        print(text)


def replay(
    *,
    run_id: str,
    output: str = "html",
) -> None:
    p = Path("./runs") / run_id
    if output == "html":
        h = p / "replay.html"
        if h.exists():
            echo(f"replay ready: {h}")
            echo(h.read_text()[:500] + "... (full in file)")
        else:
            echo(f"no replay.html for {run_id}")
    else:
        s = p / "summary.json"
        echo(s.read_text() if s.exists() else "no summary")


def validate_task(
    *,
    path: Path,
    sandbox: str = "local",
    image: str | None = None,
) -> None:
    """Validate task(s): schema, gold-solves, fail-to-pass-real, determinism."""
    if sandbox not in {"local", "docker"}:
        echo(f"error --sandbox must be local|docker, got {sandbox!r}")
        raise SystemExit(1)
    argv = [str(path)]
    if sandbox != "local":
        argv.extend(["--sandbox", sandbox])
    if image is not None:
        argv.extend(["--image", image])
    raise SystemExit(validate_main(argv))


def list_tasks(
    *,
    category: str | None = None,
) -> None:
    tasks_dir = Path("tasks/v1")
    ids = [d.name for d in tasks_dir.iterdir() if d.is_dir()] if tasks_dir.exists() else []
    if category:
        ids = [i for i in ids if category in i]
    for i in ids:
        echo(i)
    if not ids:
        echo("no tasks in tasks/v1/ (hello-world synthetic is the demo seed)")


def regrade(
    *,
    target: Path,
    tasks_base: Path = _TASKS_BASE,
    sandbox: str = "docker",
    image: str | None = None,
    write: bool = True,
    json_output: bool = False,
) -> None:
    """Re-grade existing run(s) against the current task definition, at zero API cost.

    Rebuilds each run's workspace from the task base plus the captured agent patch,
    overlays the *current* hidden tests, and re-runs the verifier. Use after
    changing a task's tests or thresholds to re-score old runs without paying for
    new model calls.
    """
    if sandbox not in {"local", "docker"}:
        echo(f"error --sandbox must be local|docker, got {sandbox!r}")
        raise SystemExit(1)
    run_dirs = find_run_dirs(target)
    if not run_dirs:
        echo(f"error no runs (no summary.json) under {target}")
        raise SystemExit(1)

    records = []
    for rd in run_dirs:
        rec = regrade_run(rd, tasks_base=tasks_base, sandbox=sandbox, image=image, write=write)
        records.append(rec)

    if json_output:
        print(json.dumps(records, indent=2))
        raise SystemExit(0)

    rows: list[tuple[str, ...]] = []
    n_changed = 0
    for rec in records:
        if rec.get("error"):
            rows.append((str(rec.get("task_id") or rec["run_dir"]), "-", "-", "-", rec["error"]))
            continue
        old = rec["old_functional"]
        new = rec["new_functional"]
        delta = rec["delta"]
        if delta:
            n_changed += 1
        dstr = (
            ""
            if delta is None
            else (f"+{delta}" if delta > 0 else f"{delta}" if delta < 0 else "0")
        )
        rows.append(
            (
                rec["task_id"],
                "-" if old is None else f"{old:.3f}",
                f"{new:.3f}",
                dstr,
                rec.get("model") or "",
            )
        )
    print_table(("Task", "Old", "New", "Δ", "Note"), rows)
    echo(f"regraded {len(records)} run(s), {n_changed} score change(s), $0 API spend")


def compare(
    *,
    suite: str,
    runs_dir: Path = _RUNS_DIR,
    tasks_base: Path = _TASKS_BASE,
    incomplete: bool = False,
    json_output: bool = False,
) -> None:
    """Build a model x effort comparison for a frozen suite from cached runs only.

    Re-runs nothing: every cell comes from ``./runs``, filtered to the suite's
    current task hashes (stale runs, scored against an older task definition, are
    excluded). To add a model to the comparison, run just that one model against
    the suite, then re-run ``compare``: the baselines are read from cache.
    """
    try:
        matrix = build_matrix(suite, runs_dir=runs_dir, tasks_base=tasks_base)
    except FileNotFoundError as e:
        echo(f"error {e}")
        raise SystemExit(1) from e

    if json_output:
        print(json.dumps(matrix, indent=2))
        raise SystemExit(0)

    echo(f"{matrix['suite']} frozen @ {matrix['version']} · {matrix['n_tasks']} tasks")
    cells = matrix["cells"] if incomplete else [c for c in matrix["cells"] if c["complete"]]

    rows = []
    for c in cells:
        cov = f"{c['n_present']}/{c['n_total']}"
        cov_str = cov if c["complete"] else f"{cov} (incomplete)"
        rows.append(
            (
                f"{c['model']} ({c['effort']})",
                "-" if c["pass_at_1"] is None else f"{c['pass_at_1']:.3f}",
                "?" if c["cost_usd"] is None else f"${c['cost_usd']:.2f}",
                cov_str,
            )
        )
    print_table(("Model (effort)", "Pass@1", "Cost", "Coverage"), rows)

    if matrix["n_stale_excluded"]:
        echo(
            f"{matrix['n_stale_excluded']} stale run(s) excluded "
            f"(scored against an older task definition; re-run or `regrade`)."
        )

    # Show what a new column needs: incomplete cells and how to fill them.
    gaps = [c for c in matrix["cells"] if not c["complete"]]
    if gaps and not incomplete:
        echo("Incomplete cells hidden; pass --incomplete to see them.")
    for c in gaps if incomplete else []:
        miss = c["missing_tasks"]
        echo(f"incomplete {c['model']} ({c['effort']}): missing {len(miss)}/{c['n_total']} task(s)")
        if c["n_present"] == 0:
            eff = "" if c["effort"] == "-" else f" --effort {c['effort']}"
            echo(f"  fill: vulcanbench run --suite {suite} -m {c['model']}{eff} --sandbox docker")
        else:
            shown = ", ".join(miss[:6]) + (" …" if len(miss) > 6 else "")
            echo(f"  missing: {shown}")


def audit_runs(
    *,
    runs_dir: Path = _RUNS_DIR,
    tasks_base: Path = _TASKS_BASE,
    write: bool = True,
) -> None:
    """Audit CLI-harness runs for web and filesystem leakage.

    Two channels leak answers to an external harness. The web: v3 tasks derive
    from public merged PRs, so the upstream fix sits at a known URL. The
    filesystem: those harnesses run on the host, so an agent can read
    VulcanBench's own ``tasks/`` tree, including the gold patch and hidden
    tests. Each run gets an ``integrity_audit`` with both verdicts. It
    annotates summaries; it never rescores.
    """
    from harness.agent.run_audit import audit_run  # noqa: PLC0415

    meta_cache: dict[str, dict[str, Any]] = {}

    def metadata_for(task_id: str) -> dict[str, Any]:
        if task_id not in meta_cache:
            hits = list(tasks_base.glob(f"*/{task_id}/metadata.json"))
            meta_cache[task_id] = json.loads(hits[0].read_text()) if hits else {}
        return meta_cache[task_id]

    counts: dict[str, int] = {}
    contaminated_solved = 0
    audited = 0
    for sp in sorted(runs_dir.rglob("summary.json")):
        try:
            summary = json.loads(sp.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        if "cli_agent" not in summary:
            continue
        audit = audit_run(
            sp.parent / "cli-agent-stream.jsonl",
            metadata_for(summary["task_id"]),
            sp.parent / "workspace",
        )
        audited += 1
        for channel in ("web", "filesystem"):
            key = f"{channel}:{audit[channel]['verdict']}"
            counts[key] = counts.get(key, 0) + 1
        if audit["contaminated"] and (summary.get("scores") or {}).get("functional") == 1.0:
            contaminated_solved += 1
        if write:
            summary["integrity_audit"] = audit
            summary.pop("web_audit", None)
            sp.write_text(json.dumps(summary, indent=2), encoding="utf-8")

    echo(f"audited {audited} CLI-harness run(s){'' if write else ' (dry run)'}")
    for key in sorted(counts):
        echo(f"  {key}: {counts[key]}")
    if contaminated_solved:
        echo(
            f"{contaminated_solved} solved run(s) are contaminated "
            "(upstream access or solution retrieval); exclude or asterisk them in reports"
        )


def build_parser() -> argparse.ArgumentParser:  # noqa: PLR0915, one add_argument per option
    """The ``vulcanbench`` argument parser: one sub-parser per command."""
    parser = argparse.ArgumentParser(
        prog="vulcanbench",
        description="VulcanBench v1 - reproducible LLM benchmarking harness for SWE tasks",
    )
    parser.add_argument(
        "--version",
        "-v",
        action="version",
        version=f"vulcanbench {__version__}",
        help="Show version and exit",
    )
    commands = parser.add_subparsers(dest="_command", metavar="<command>")

    p = add_command(commands, "run", run)
    p.add_argument("--task", "-t", dest="task", default=None, help="Task ID e.g. swe-001")
    p.add_argument(
        "--suite", dest="suite", default=None, help="Run a whole suite, e.g. v1 (tasks/v1)"
    )
    p.add_argument(
        "--tasks-root",
        dest="tasks_root",
        type=Path,
        default=_TASKS_V1,
        help="Task definitions root for --task runs (suites derive their own)",
    )
    p.add_argument(
        "--model",
        "-m",
        dest="model",
        required=True,
        help="Model id. Use provider:model for Vulcan's API loop, or a bare model with --harness claude-code|codex|cursor|grok-build|zcode|muse-code|devin|pi|dsh|mirror.",
    )
    p.add_argument(
        "--harness",
        dest="harness_name",
        default="vulcan",
        help="Execution harness: vulcan|claude-code|codex|cursor|grok-build|zcode|muse-code|devin|pi|dsh|mirror",
    )
    p.add_argument(
        "--billing",
        dest="billing",
        default="auto",
        help="Access/billing mode: auto|api|subscription",
    )
    p.add_argument(
        "--output-dir",
        "-o",
        dest="output_dir",
        type=Path,
        default=_RUNS_DIR,
        help="Where to write trace/replay",
    )
    p.add_argument(
        "--max-steps",
        dest="max_steps",
        type=int,
        default=None,
        help="Cap agent steps (default: per-task metadata / scale tier)",
    )
    p.add_argument(
        "--judges",
        dest="judges",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Run the human_like LLM judge ensemble",
    )
    p.add_argument(
        "--judge-model",
        dest="judge_model",
        default=None,
        help="Model for the judge ensemble and agentic grader (default: same as --model)",
    )
    p.add_argument(
        "--sandbox",
        dest="sandbox",
        default="docker",
        help="Where tools run: local|docker|auto (default docker; agents execute model-written shell commands, so opt into 'local' deliberately)",
    )
    p.add_argument(
        "--image",
        dest="image",
        default=None,
        help="Docker image for the sandbox (default: per task or vulcanbench/sandbox:base)",
    )
    p.add_argument(
        "--network",
        dest="network",
        action=argparse.BooleanOptionalAction,
        default=False,
        help="Allow network in the docker sandbox",
    )
    p.add_argument(
        "--mem-floor",
        dest="mem_floor",
        default=None,
        help="Guaranteed sandbox memory (docker mem_reservation, e.g. 2g). Floor of the resource band; see the infrastructure-noise methodology.",
    )
    p.add_argument(
        "--mem-ceiling",
        dest="mem_ceiling",
        default="2g",
        help="Hard sandbox memory kill threshold (docker mem_limit)",
    )
    p.add_argument(
        "--cpu-floor",
        dest="cpu_floor",
        type=float,
        default=None,
        help="Guaranteed sandbox CPUs under contention (docker cpu_shares weight)",
    )
    p.add_argument(
        "--cpu-ceiling",
        dest="cpu_ceiling",
        type=float,
        default=2.0,
        help="Hard sandbox CPU cap (docker nano_cpus)",
    )
    p.add_argument(
        "--pids-limit",
        dest="pids_limit",
        type=int,
        default=512,
        help="Hard sandbox process-count cap",
    )
    p.add_argument(
        "--agent-container",
        dest="agent_container",
        action=argparse.BooleanOptionalAction,
        default=False,
        help="Run the subscription CLI agent inside a container (codex only for now): Harbor-style execution with the resource band applied to the agent phase too. Requires the vulcanbench/agent-codex image.",
    )
    p.add_argument(
        "--repeat",
        dest="repeat",
        type=int,
        default=1,
        help="Run each task N times (for pass@k / stderr)",
    )
    p.add_argument(
        "--max-concurrency",
        dest="max_concurrency",
        type=int,
        default=1,
        help="Run suite tasks in parallel (suite runs only)",
    )
    p.add_argument(
        "--max-cost",
        dest="max_cost",
        type=float,
        default=None,
        help="Suite budget cap: metered cash for APIs, API-equivalent value for subscriptions",
    )
    p.add_argument(
        "--max-run-cost",
        dest="max_run_cost",
        type=float,
        default=None,
        help="Per-run pricing ceiling: metered cash for APIs, API-equivalent value for subscriptions (records cost_capped; partial result is still graded)",
    )
    p.add_argument(
        "--only-missing",
        dest="only_missing",
        action="store_true",
        help="Suite runs: skip tasks that already have enough fresh cached runs for this model+effort, running only the gaps (resume a partial column)",
    )
    p.add_argument(
        "--max-infra-retries",
        dest="max_infra_retries",
        type=int,
        default=2,
        help="Suite runs: how many times a unit whose failure was environmental (provider stall, sandbox) is re-queued before it counts as errored (default 2; 0 never retries)",
    )
    p.add_argument(
        "--timeout",
        dest="timeout",
        type=float,
        default=None,
        help="Per-run wall-clock budget in seconds (abort if exceeded)",
    )
    p.add_argument(
        "--override-budgets",
        dest="override_budgets",
        action="store_true",
        help="Treat --timeout/--max-steps as exact budgets, even ABOVE task defaults (ablation mode; runs are marked and not comparable to capped runs)",
    )
    p.add_argument(
        "--fail-under",
        dest="fail_under",
        type=float,
        default=None,
        help="Exit non-zero (4) if pass@1 is below this threshold (CI gate)",
    )
    p.add_argument(
        "--effort",
        dest="effort",
        default=None,
        help="Normalized reasoning effort: low|medium|high|extra-high|max (levels under [effort].blocked in vulcanbench.toml are refused)",
    )
    p.add_argument(
        "--dry-run", dest="dry_run", action="store_true", help="Plan only, do not launch sandbox"
    )
    p.add_argument(
        "--allow-dirty",
        dest="allow_dirty",
        action="store_true",
        help="Let a suite run start with uncommitted changes to harness/, tasks/ or sandbox/ (experiments only; the run records that it was dirty)",
    )
    p.add_argument(
        "--priors",
        dest="use_priors",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Use bundled cost priors when local ./runs history is missing (dry-run estimate)",
    )

    p = add_command(commands, "estimate", estimate)
    p.add_argument("--task", "-t", dest="task", default=None, help="Single task ID")
    p.add_argument("--suite", dest="suite", default=None, help="Suite name, e.g. v1-compare")
    p.add_argument(
        "--model",
        "-m",
        dest="model",
        action="append",
        required=True,
        help="provider:model (repeat for multiple models)",
    )
    p.add_argument("--repeat", dest="repeat", type=int, default=1, help="Planned repeats per task")
    p.add_argument(
        "--judges",
        dest="judges",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Include judge ensemble cost (~3x agent tokens)",
    )
    p.add_argument(
        "--runs-dir",
        dest="runs_dir",
        type=Path,
        default=_RUNS_DIR,
        help="Local runs dir for historical costs",
    )
    p.add_argument(
        "--priors",
        dest="use_priors",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Use bundled cost priors when local ./runs history is missing",
    )
    p.add_argument(
        "--json", dest="json_output", action="store_true", help="Emit machine-readable JSON"
    )

    p = add_command(commands, "effort-sweep", effort_sweep)
    p.add_argument("--suite", dest="suite", required=True, help="Suite to sweep, e.g. v1")
    p.add_argument(
        "--model", "-m", dest="model", required=True, help="provider:model e.g. openai:gpt-5.1"
    )
    p.add_argument(
        "--efforts",
        dest="efforts",
        default=_DEFAULT_EFFORTS,
        help="Comma-separated normalized efforts; default low,medium,high",
    )
    p.add_argument(
        "--repeat", dest="repeat", type=int, default=1, help="Run each task N times per effort"
    )
    p.add_argument(
        "--output-dir",
        "-o",
        dest="output_dir",
        type=Path,
        default=_RUNS_DIR,
        help="Where to write traces and experiment.json",
    )
    p.add_argument(
        "--max-steps",
        dest="max_steps",
        type=int,
        default=None,
        help="Cap agent steps (default: per-task metadata / scale tier)",
    )
    p.add_argument(
        "--judges",
        dest="judges",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Run the human_like LLM judge ensemble",
    )
    p.add_argument(
        "--judge-model",
        dest="judge_model",
        default=None,
        help="Model for the judge ensemble and agentic grader (default: same as --model)",
    )
    p.add_argument(
        "--sandbox",
        dest="sandbox",
        default="docker",
        help="Where tools run: local|docker|auto (default docker; agents execute model-written shell commands, so opt into 'local' deliberately)",
    )
    p.add_argument(
        "--image",
        dest="image",
        default=None,
        help="Docker image for the sandbox (default: per task or vulcanbench/sandbox:base)",
    )
    p.add_argument(
        "--network",
        dest="network",
        action=argparse.BooleanOptionalAction,
        default=False,
        help="Allow network in the docker sandbox",
    )
    p.add_argument(
        "--max-concurrency",
        dest="max_concurrency",
        type=int,
        default=1,
        help="Run suite tasks in parallel for each effort",
    )
    p.add_argument(
        "--max-cost",
        dest="max_cost",
        type=float,
        default=None,
        help="Per-effort cap: metered cash for APIs, API-equivalent value for subscriptions",
    )
    p.add_argument(
        "--max-run-cost",
        dest="max_run_cost",
        type=float,
        default=None,
        help="Per-run pricing ceiling: metered cash for APIs, API-equivalent value for subscriptions (records cost_capped; partial result is still graded)",
    )
    p.add_argument(
        "--only-missing",
        dest="only_missing",
        action="store_true",
        help="Skip tasks that already have enough fresh cached runs per effort, running only the gaps (resume a partial sweep)",
    )
    p.add_argument(
        "--timeout",
        dest="timeout",
        type=float,
        default=None,
        help="Per-run wall-clock budget in seconds (abort if exceeded)",
    )
    p.add_argument(
        "--dry-run", dest="dry_run", action="store_true", help="Plan only, do not launch sandbox"
    )
    p.add_argument(
        "--allow-dirty",
        dest="allow_dirty",
        action="store_true",
        help="Start with uncommitted changes to harness/, tasks/ or sandbox/ (experiments only; every run records that it was dirty)",
    )

    p = add_command(commands, "compare", compare)
    p.add_argument("--suite", dest="suite", required=True, help="Suite to compare, e.g. v2")
    p.add_argument(
        "--runs-dir",
        "-o",
        dest="runs_dir",
        type=Path,
        default=_RUNS_DIR,
        help="Where cached runs live",
    )
    p.add_argument(
        "--tasks-base",
        dest="tasks_base",
        type=Path,
        default=_TASKS_BASE,
        help="Root holding task suites",
    )
    g = p.add_mutually_exclusive_group()
    g.add_argument(
        "--incomplete",
        dest="incomplete",
        action="store_true",
        default=False,
        help="Also show cells missing some tasks",
    )
    g.add_argument(
        "--complete-only", dest="incomplete", action="store_false", help="opposite of --incomplete"
    )
    p.add_argument(
        "--json", dest="json_output", action="store_true", help="Emit the matrix as JSON"
    )

    p = add_command(commands, "regrade", regrade)
    p.add_argument(
        "target",
        type=existing_path,
        help="A run directory, or a directory of runs (regrades every run under it)",
    )
    p.add_argument(
        "--tasks-base",
        dest="tasks_base",
        type=Path,
        default=_TASKS_BASE,
        help="Root holding task suites (tasks/<suite>/<id>)",
    )
    p.add_argument(
        "--sandbox",
        dest="sandbox",
        default="docker",
        help="Where to run verifiers: local|docker (docker matches runs)",
    )
    p.add_argument(
        "--image", dest="image", default=None, help="Docker sandbox image (default: per-task image)"
    )
    p.add_argument(
        "--write",
        dest="write",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Write regrade.json into each run dir",
    )
    p.add_argument(
        "--json", dest="json_output", action="store_true", help="Emit the records as JSON"
    )

    p = add_command(commands, "leaderboard", leaderboard)
    p.add_argument("--by", dest="by", default="model", help="Aggregate view: model|run")
    p.add_argument("--format", "-f", dest="format", default="markdown", help="markdown|json")
    p.add_argument("--task", dest="task", default=None, help="Filter by task id (run view)")
    p.add_argument("--suite", dest="suite", default=None, help="Filter by suite")
    p.add_argument(
        "--track", dest="track", default="all", help="Result track: all|api|subscription"
    )

    p = add_command(commands, "report", report)
    p.add_argument("--suite", dest="suite", default=None, help="Limit the report to one suite")
    p.add_argument("--format", "-f", dest="format", default="md", help="md|json")
    p.add_argument(
        "--output",
        "-o",
        dest="output",
        type=Path,
        default=None,
        help="Write to a file (default: stdout)",
    )
    p.add_argument(
        "--runs-dir", dest="runs_dir", type=Path, default=_RUNS_DIR, help="Where runs live"
    )
    p.add_argument(
        "--tasks-root", dest="tasks_root", type=Path, default=_TASKS_V1, help="Task definitions"
    )

    p = add_command(commands, "calibrate", calibrate)
    p.add_argument("--suite", dest="suite", default=None, help="Filter by suite")
    p.add_argument("--format", "-f", dest="format", default="table", help="table|md|json")
    p.add_argument(
        "--output",
        "-o",
        dest="output",
        type=Path,
        default=None,
        help="Write to a file (default: stdout)",
    )
    p.add_argument(
        "--min-attempts",
        dest="min_attempts",
        type=int,
        default=5,
        help="Minimum attempts for calibration",
    )
    p.add_argument(
        "--min-models",
        dest="min_models",
        type=int,
        default=2,
        help="Minimum distinct models for calibration",
    )
    p.add_argument(
        "--easy-min", dest="easy_min", type=float, default=0.85, help="Solve rate >= this is easy"
    )
    p.add_argument(
        "--medium-min",
        dest="medium_min",
        type=float,
        default=0.4,
        help="Solve rate >= this is medium",
    )
    p.add_argument(
        "--runs-dir", dest="runs_dir", type=Path, default=_RUNS_DIR, help="Where runs live"
    )
    p.add_argument(
        "--tasks-root", dest="tasks_root", type=Path, default=_TASKS_V1, help="Task definitions"
    )
    p.add_argument(
        "--include-mock", dest="include_mock", action="store_true", help="Include mock:model runs"
    )

    p = add_command(commands, "replay", replay)
    p.add_argument("run_id", help="Run ID from ./runs/")
    p.add_argument("--output", "-o", dest="output", default="html", help="html|json")

    p = add_command(commands, "validate-task", validate_task)
    p.add_argument("path", type=existing_dir, help="Path to a task dir or tasks/v1")
    p.add_argument(
        "--sandbox",
        dest="sandbox",
        default="local",
        help="Where to run verifiers: local|docker (docker matches benchmark runs)",
    )
    p.add_argument(
        "--image", dest="image", default=None, help="Docker sandbox image when --sandbox docker"
    )

    p = add_command(commands, "list-tasks", list_tasks)
    p.add_argument("--category", dest="category", default=None, help="Filter category")

    p = add_command(commands, "audit-runs", audit_runs)
    p.add_argument(
        "runs_dir",
        type=Path,
        nargs="?",
        default=_RUNS_DIR,
        help="Directory of runs to audit (recursive)",
    )
    p.add_argument(
        "--tasks-base",
        dest="tasks_base",
        type=Path,
        default=_TASKS_BASE,
        help="Root holding task suites (tasks/<suite>/<id>)",
    )
    g = p.add_mutually_exclusive_group()
    g.add_argument(
        "--write",
        dest="write",
        action="store_true",
        default=True,
        help="Persist web_audit into each run's summary.json",
    )
    g.add_argument("--dry-run", dest="write", action="store_false", help="opposite of --write")

    harness_commands = add_group(
        commands, "harness", "Inspect subscription-backed execution harnesses"
    )
    p = add_command(harness_commands, "list", harness_list)
    p.add_argument(
        "--json", dest="json_output", action="store_true", help="Emit machine-readable JSON"
    )
    p = add_command(harness_commands, "doctor", harness_doctor)
    p.add_argument("name", nargs="?", default=None, help="Harness name; omit to check all")
    p.add_argument(
        "--json", dest="json_output", action="store_true", help="Emit machine-readable JSON"
    )

    register_voice_commands(
        add_group(commands, "voice", "Voice Eval Suite: text-vs-audio delta measurement")
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Console-script entry point: parse ``argv`` (default ``sys.argv[1:]``) and dispatch."""
    return run_parser(build_parser(), argv)


if __name__ == "__main__":
    raise SystemExit(main())
