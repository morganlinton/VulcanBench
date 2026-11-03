"""Suite definition and runner.

A *suite* is a named set of tasks (e.g. ``v1`` → the tasks under ``tasks/v1/``).
``run_suite`` runs a model against every task in the suite, tags each run so the
leaderboard can group it, and writes an aggregate ``suite.json``.

Suites can also live *outside* the checkout, in a suites home
(``~/vulcanbench-suites/<suite-id>/`` by default, ``[runner].suites_home`` in
``vulcanbench.toml``). Those are built from a user's own repository with
``vulcanbench suite ...`` and are never committed here. ``load_suite`` resolves
an in-tree name first and the suites home second; an external suite only runs
when its ``suite.lock.json`` matches the tasks on disk (``harness.suite_lock``).
"""

from __future__ import annotations

import json
import os
import uuid
from concurrent.futures import FIRST_COMPLETED, ThreadPoolExecutor, wait
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from harness.agent.cli_agents import is_cli_agent_spec
from harness.agent.loop import run_agent
from harness.agent.providers import NonRetryableProviderError, ProviderError
from harness.leaderboard import aggregate_by_model, scan_leaderboard
from harness.provenance import source_provenance
from harness.sandbox.docker_executor import SandboxError
from harness.settings import runner_settings
from harness.task_metadata import repo_scale
from harness.tasks import list_task_ids, load_task
from harness.verifier import VerifierInfrastructureError

DEFAULT_TASKS_BASE = Path("tasks")
#: Environment override for ``[runner].suites_home`` (the GUI's convention for
#: every ``[runner]`` key).
SUITES_HOME_ENV = "VULCANBENCH_RUNNER_SUITES_HOME"
SUITE_ALIASES = {
    "v1-micro": "v1",
    "v1-large": "v1",
    "v1-rust": "v1",
    "v1-compare": "v1",
    "v1-diamond": "v1",
    "v1-carbyne": "v1",
}

#: Plain directory aliases: alternate CLI names for a whole suite, with none of
#: the v1 tier/filter semantics. ``--suite cii`` loads ``tasks/cii-v1`` as-is.
SUITE_NAME_ALIASES = {
    "cii": "cii-v1",
    "coding-intelligence-index": "cii-v1",
    # The frontier suite's directory was renamed from its internal build id
    # (cii-v2) to its public name on 2026-09-01. The old id stays as an alias
    # so historical command lines, scripts, and notes keep resolving.
    "cii-v2": "coding-intelligence-index-v4",
    "cii-v4": "coding-intelligence-index-v4",
}


#: Failures that say nothing about the model under test: a stalled or refused
#: provider call, a sandbox that would not start. Scoring these as a task failure
#: (or dropping the task from the suite) attributes an infrastructure problem to
#: the model, so they are retried instead.
INFRASTRUCTURE_ERRORS = (ProviderError, SandboxError, VerifierInfrastructureError)


def is_infrastructure_error(exc: BaseException) -> bool:
    """Whether ``exc`` is an environment failure rather than a graded outcome.

    A task the model genuinely fails still *returns* a result with a low score;
    it does not raise. Anything reaching here is the harness failing to obtain a
    verdict at all.
    """
    return isinstance(exc, INFRASTRUCTURE_ERRORS) and not isinstance(exc, NonRetryableProviderError)


@dataclass
class Suite:
    name: str
    tasks_root: Path
    task_ids: list[str]
    #: Human-facing suite name from suite.json ``display_name`` (e.g. "VulcanBench
    #: Coding Intelligence Index"); falls back to ``name`` when the manifest has none.
    display_name: str = ""
    #: True when the suite was loaded from a suites home rather than ``tasks/``.
    external: bool = False
    #: The suites home an external suite came from (None for in-tree suites).
    suites_home: Path | None = None
    #: ``version`` from suite.json, when the manifest has one.
    version: str | None = None

    def __post_init__(self) -> None:
        if not self.display_name:
            self.display_name = self.name

    @property
    def suite_dir(self) -> Path:
        """The directory holding ``suite.json`` and the task directories."""
        return self.tasks_root

    def provenance(self) -> dict[str, Any]:
        """The ``source.suite`` block every run summary records.

        Names the suite, where it was loaded from and, when it is frozen, the
        lock it was frozen to: ``lock_digest`` is the content digest of
        ``suite.lock.json`` (:func:`harness.suite_lock.lock_digest`), so two
        summaries with the same digest were scored against the same task set.
        Lock fields are ``None`` for a suite without a lock.
        """
        from harness.suite_lock import lock_digest, read_lock  # noqa: PLC0415, import cycle

        lock = read_lock(self.tasks_root)
        return {
            "kind": "external" if self.external else "in-tree",
            "id": self.name,
            "display_name": self.display_name,
            "suites_home": str(self.suites_home) if self.suites_home is not None else None,
            "suite_dir": str(self.tasks_root),
            "lock_version": None if lock is None else lock.get("version"),
            "lock_digest": None if lock is None else lock_digest(lock),
            "frozen_at": None if lock is None else lock.get("frozen_at"),
            "n_tasks": len(self.task_ids),
        }


def resolve_suites_home(explicit: Path | str | None = None) -> Path:
    """Where external suites live: the flag, else the environment, else the settings file.

    ``explicit`` is a ``--suites-home`` value. Otherwise ``VULCANBENCH_RUNNER_SUITES_HOME``
    wins over ``[runner].suites_home`` in ``vulcanbench.toml`` (default
    ``~/vulcanbench-suites``). The result is ``~``-expanded and absolute.
    """
    value: str | None = None if explicit is None else str(explicit)
    if not value:
        value = os.environ.get(SUITES_HOME_ENV) or None
    if not value:
        value = str(runner_settings()["suites_home"])
    return Path(value).expanduser().absolute()


def load_suite_dir(
    suite_dir: Path,
    *,
    name: str | None = None,
    external: bool = False,
    suites_home: Path | None = None,
) -> Suite:
    """Load the suite whose directory is ``suite_dir``: the one reader of ``suite.json``.

    In-tree (``external=False``) the manifest is optional and pins a subset
    (``full`` or ``tasks``); without one, every task directory is in. For an
    external suite the manifest is required and its ``tasks`` list is the
    suite: an empty list is an empty suite, never every subdirectory.
    """
    manifest = suite_dir / "suite.json"
    data: dict[str, Any] = {}
    if manifest.is_file():
        raw = json.loads(manifest.read_text(encoding="utf-8"))
        if isinstance(raw, dict):
            data = raw
    elif external:
        raise FileNotFoundError(f"external suite has no manifest: {manifest}")
    elif not suite_dir.is_dir():
        raise FileNotFoundError(f"suite directory not found: {suite_dir}")

    if external:
        task_ids = [str(t) for t in (data.get("tasks") or [])]
    else:
        task_ids = list(data.get("full") or data.get("tasks") or list_task_ids(suite_dir))
    raw_display = data.get("display_name")
    display_name = raw_display if isinstance(raw_display, str) else ""
    raw_version = data.get("version")
    version = None if raw_version is None else str(raw_version)
    raw_id = data.get("id")
    suite_name = name or (raw_id if isinstance(raw_id, str) and raw_id else suite_dir.name)
    return Suite(
        name=suite_name,
        tasks_root=suite_dir,
        task_ids=task_ids,
        display_name=display_name,
        external=external,
        suites_home=suites_home,
        version=version,
    )


def _scale_filter(tasks_root: Path, tier: str) -> list[str]:
    """Filter task ids by ``repo_scale`` tier (micro | large)."""
    out: list[str] = []
    for task_id in list_task_ids(tasks_root):
        meta = load_task(task_id, tasks_root).metadata
        scale = repo_scale(meta)
        if (tier == "micro" and scale in ("micro", "small")) or (
            tier == "large" and scale in ("medium", "large")
        ):
            out.append(task_id)
    return out


def _language_filter(tasks_root: Path, language: str) -> list[str]:
    """Filter task ids by presence of ``language`` in metadata.languages."""
    out: list[str] = []
    for task_id in list_task_ids(tasks_root):
        meta = load_task(task_id, tasks_root).metadata
        langs = meta.get("languages", [])
        if language in langs:
            out.append(task_id)
    return out


def load_suite(
    name: str,
    tasks_base: Path = DEFAULT_TASKS_BASE,
    *,
    suites_home: Path | None = None,
) -> Suite:
    """Load a suite by name.

    The task set is every task directory under ``tasks/<name>/``, unless a
    ``tasks/<name>/suite.json`` manifest pins a subset/order. Aliases ``v1-micro``
    and ``v1-large`` read ``tasks/v1/suite.json`` keys ``micro`` / ``large``.
    ``v1-rust`` filters by language ``rust`` in metadata.languages. ``v1-diamond``
    and ``v1-carbyne`` read the ``diamond`` / ``carbyne`` keys: rubric-graded
    mergeability tiers (carbyne is the harder one, the naive solution is subtly
    wrong). Run them with a ``--judge-model`` different from the model under test
    to avoid self-grading.

    Resolution order: an alias or ``tasks/<name>`` in the checkout first, then
    ``<suites_home>/<name>/suite.json`` (see :func:`resolve_suites_home`). An
    in-tree suite always wins over an external one of the same id.
    """
    if name in SUITE_ALIASES:
        tasks_root = tasks_base / SUITE_ALIASES[name]
        if name == "v1-rust":
            task_ids = list(_language_filter(tasks_root, "rust"))
            return Suite(name=name, tasks_root=tasks_root, task_ids=task_ids)
        manifest = tasks_root / "suite.json"
        if manifest.exists():
            data = json.loads(manifest.read_text(encoding="utf-8"))
            if name == "v1-compare":
                task_ids = list(data.get("compare") or [])
            elif name == "v1-diamond":
                task_ids = list(data.get("diamond") or [])
            elif name == "v1-carbyne":
                task_ids = list(data.get("carbyne") or [])
            else:
                key = "micro" if name == "v1-micro" else "large"
                task_ids = list(data.get(key) or _scale_filter(tasks_root, key))
        else:
            key = "micro" if name == "v1-micro" else "large"
            task_ids = _scale_filter(tasks_root, key)
        return Suite(name=name, tasks_root=tasks_root, task_ids=task_ids)

    dir_name = SUITE_NAME_ALIASES.get(name, name)
    tasks_root = tasks_base / dir_name
    if tasks_root.is_dir():
        return load_suite_dir(tasks_root, name=dir_name)
    home = resolve_suites_home(suites_home)
    external_dir = home / name
    if (external_dir / "suite.json").is_file():
        return load_suite_dir(external_dir, name=name, external=True, suites_home=home)
    raise FileNotFoundError(
        f"suite {name!r} not found under {tasks_base} (in-tree) or {home} (suites home)"
    )


def run_suite(  # noqa: PLR0912, PLR0915, linear scheduler: validation + budget loop + summary
    name: str,
    model: str,
    output_dir: Path = Path("./runs"),
    tasks_base: Path = DEFAULT_TASKS_BASE,
    repeat: int = 1,
    max_concurrency: int = 1,
    max_cost: float | None = None,
    only_missing: bool = False,
    *,
    max_infra_retries: int = 2,
    suites_home: Path | None = None,
    **run_kwargs: Any,
) -> dict[str, Any]:
    """Run ``model`` against every task in the suite, ``repeat`` times each.

    With ``max_concurrency > 1`` the (task x repeat) units run on a thread pool.
    Each unit is a fully isolated :func:`run_agent` call (its own run dir, git
    workspace, collector and provider), so results are identical to sequential
    execution, only wall-clock differs. One unit raising does not sink the
    suite; its failure is recorded in ``errors``.

    ``max_cost`` is a metered-cash cap for API runs and an API-equivalent cap
    for subscription runs. Once the accumulated value reaches it, no *new* runs
    are launched and the remainder are recorded in ``skipped``. It is not a
    hard ceiling, runs already in flight finish, so the total can exceed
    ``max_cost`` by up to ``max_concurrency`` runs' worth. It **fails closed**:
    if a completed run reports no cost (e.g. an unpriced model), the budget
    cannot be guaranteed, so launching stops (``cost_unknown=True``) rather
    than silently running the whole suite.

    ``max_infra_retries`` re-queues a unit whose failure was environmental (a
    stalled provider call, a sandbox that would not start) rather than a verdict
    about the model, see :func:`is_infrastructure_error`. Without it a single
    stalled request silently shrinks the denominator: the suite still reports a
    tidy pass@1, computed over whichever tasks happened not to get unlucky, and a
    different task drops on each run. Retries obey the spend cap like any other
    unit and are counted in ``n_infra_retries``.

    ``suites_home`` is where an external suite is looked up when ``name`` is
    not in ``tasks/``. An external suite must be frozen and its lock must match
    the tasks on disk (:func:`harness.suite_lock.require_locked`); otherwise
    :class:`harness.suite_lock.SuiteLockError` is raised before any run starts.
    There is no override: edit the suite, re-freeze, then run.

    Returns the suite summary (also written to ``<output_dir>/<suite_id>/suite.json``).
    Extra keyword args (judges, sandbox, image, network, max_steps, …) pass
    through to :func:`run_agent`, as does ``suite_source`` (the suite's
    :meth:`Suite.provenance`), which every run summary records under
    ``source.suite``.
    """
    if repeat < 1:
        raise ValueError("repeat must be >= 1")
    if max_concurrency < 1:
        raise ValueError("max_concurrency must be >= 1")
    if max_cost is not None and max_cost <= 0:
        raise ValueError("max_cost must be > 0")
    suite = load_suite(name, tasks_base, suites_home=suites_home)
    if suite.external:
        from harness.suite_lock import require_locked  # noqa: PLC0415, import cycle

        require_locked(suite)
    suite_source = suite.provenance()
    suite_id = f"suite-{uuid.uuid4().hex[:8]}"
    started_at = datetime.now(UTC)

    # ``only_missing`` reuses cached, non-stale runs for this model+effort and
    # launches only enough runs to top each task up to ``repeat``: so a partly
    # finished column is resumed instead of re-run from scratch.
    covered: list[str] = []
    if only_missing:
        from harness.compare import fresh_run_counts  # noqa: PLC0415, avoid import cycle

        counts = fresh_run_counts(
            list(suite.task_ids),
            model,
            run_kwargs.get("effort"),
            output_dir,
            suite.tasks_root,
        )
        units = []
        for task_id in suite.task_ids:
            need = max(0, repeat - counts.get(task_id, 0))
            units.extend([task_id] * need)
            if need == 0:
                covered.append(task_id)
    else:
        units = [task_id for task_id in suite.task_ids for _ in range(repeat)]

    def _run_one(task_id: str) -> dict[str, Any]:
        res = run_agent(
            task_id=task_id,
            model=model,
            output_dir=output_dir,
            tasks_root=suite.tasks_root,
            suite=suite.name,
            suite_id=suite_id,
            suite_source=suite_source,
            **run_kwargs,
        )
        summary = res["summary"]
        cli_agent = summary.get("cli_agent") or {}
        economics = summary.get("economics") or {}
        return {
            "task_id": task_id,
            "run_id": res["run_id"],
            "total": summary.get("scores", {}).get("total"),
            "functional": summary.get("scores", {}).get("functional"),
            "cost_usd": summary.get("cost_usd"),
            "execution_harness": cli_agent.get("harness") or "vulcan",
            "track": "subscription" if cli_agent else "api",
            "economics": economics,
            "duration_s": summary.get("duration_s"),
            "effort": summary.get("effort"),
            "experiment_id": summary.get("experiment_id"),
        }

    task_results: list[dict[str, Any]] = []
    errors: list[dict[str, Any]] = []
    infra_retries: list[dict[str, Any]] = []
    attempts: dict[str, int] = {}
    skipped: list[str] = []
    spent = 0.0
    spend_basis = (
        "api-equivalent pricing; not subscription cash or quota"
        if is_cli_agent_spec(model)
        else "metered API pricing"
    )
    cost_unknown = False  # a completed run reported no cost while a budget is set

    def _budget_reached() -> bool:
        # Fail CLOSED: if we can't price a completed run, we can't guarantee the
        # cap, so stop launching rather than silently running the whole suite.
        return max_cost is not None and (cost_unknown or spent >= max_cost)

    # One scheduler for both sequential (max_concurrency=1) and parallel: keep up
    # to max_concurrency runs in flight, and stop launching new ones once the
    # spend cap is reached. In-flight runs are always allowed to finish.
    pending = list(units)
    with ThreadPoolExecutor(max_workers=max_concurrency) as pool:
        in_flight: dict[Any, str] = {}
        while pending or in_flight:
            while pending and len(in_flight) < max_concurrency and not _budget_reached():
                task_id = pending.pop(0)
                in_flight[pool.submit(_run_one, task_id)] = task_id
            if pending and _budget_reached():
                skipped.extend(pending)
                pending = []
            if not in_flight:
                break
            done, _ = wait(set(in_flight), return_when=FIRST_COMPLETED)
            for fut in done:
                task_id = in_flight.pop(fut)
                try:
                    result = fut.result()
                    task_results.append(result)
                    cost = result.get("cost_usd")
                    if cost is None:
                        if max_cost is not None:
                            cost_unknown = True  # can't account this run -> fail closed
                    else:
                        spent += cost
                except Exception as e:  # contain one unit's failure; keep the suite going
                    attempts[task_id] = attempts.get(task_id, 1)
                    retryable = (
                        is_infrastructure_error(e) and attempts[task_id] <= max_infra_retries
                    )
                    if retryable and not _budget_reached():
                        attempts[task_id] += 1
                        infra_retries.append(
                            {"task_id": task_id, "attempt": attempts[task_id], "error": str(e)}
                        )
                        pending.append(task_id)  # re-queue rather than drop the data point
                    else:
                        errors.append({"task_id": task_id, "error": str(e)})

    # Deterministic order regardless of completion order under concurrency.
    task_results.sort(key=lambda r: (r["task_id"], r["run_id"]))

    # Aggregate only this invocation's runs (match suite_id, not just the name).
    suite_rows = [r for r in scan_leaderboard(output_dir) if r.get("suite_id") == suite_id]
    aggregate = aggregate_by_model(suite_rows, suite=suite.name)

    suite_summary = {
        "suite_id": suite_id,
        "suite": suite.name,
        "model": model,
        "effort": run_kwargs.get("effort"),
        "experiment_id": run_kwargs.get("experiment_id"),
        "repeat": repeat,
        "max_concurrency": max_concurrency,
        "max_cost": max_cost,
        "spent_usd": round(spent, 6),
        "budget_accounted_usd": round(spent, 6),
        "spend_basis": spend_basis,
        "cost_unknown": cost_unknown,
        "started_at": started_at.isoformat(),
        "finished_at": datetime.now(UTC).isoformat(),
        "n_tasks": len(suite.task_ids),
        "n_runs": len(task_results),
        "n_skipped": len(skipped),
        "n_infra_retries": len(infra_retries),
        "infra_retries": infra_retries,
        "only_missing": only_missing,
        "covered_cached": covered,
        "tasks": task_results,
        "errors": errors,
        "skipped": skipped,
        "aggregate": aggregate,
        "source": {**source_provenance().as_summary(), "suite": suite_source},
    }
    suite_dir = output_dir / suite_id
    suite_dir.mkdir(parents=True, exist_ok=True)
    (suite_dir / "suite.json").write_text(json.dumps(suite_summary, indent=2), encoding="utf-8")
    return suite_summary
