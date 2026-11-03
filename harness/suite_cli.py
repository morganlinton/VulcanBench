"""``vulcanbench suite``: build, freeze and check suites outside the checkout.

A custom suite lives in a *suites home* (``~/vulcanbench-suites/<id>/`` by
default, ``[runner].suites_home`` in ``vulcanbench.toml``, ``--suites-home``
on every command). It is never committed into VulcanBench, so a slice of a
private repository stays private. Its tasks use the same format as every
in-tree suite (``metadata.json``, ``issue.md``, ``repo/`` or
``repo_snapshot.tar.gz``, hidden ``tests/``, ``gold_patch.diff``), which is the
only format ``harness.tasks`` and ``harness.validate`` understand.

The flow, one subcommand per step:

    vulcanbench suite init <id> --display-name NAME --repo owner/name
    vulcanbench suite validate <id> --task <task>      # writes <task>/build/validate.json
    vulcanbench suite add-task <id> --task <task>      # admits a PASS or WARN task
    vulcanbench suite freeze <id>                      # writes suite.lock.json
    vulcanbench suite check <id>                       # lock still matches the tasks
    vulcanbench run --suite <id> --suites-home <dir> ...

Per-task ``build/`` folders hold the builder's working files (the answer key
among them) and never ship: nothing under ``build/`` is in ``task_hash``, and
the agent workspace is prepared from ``repo/`` only.

Commands that create or change a suite (``init``, ``add-task``,
``remove-task``, ``validate``, ``freeze`` and the builder's ``prs`` and
``task`` commands) are password-gated: see :mod:`harness.suite_builder.access`
(exit 7 while locked). ``list``, ``show`` and ``check`` stay open.

Every subcommand takes ``--json``: stdout then holds exactly one JSON document
(``schema`` names its shape) and human text goes to stderr; an error is
``{"schema": "vulcanbench.suite.error.v1", "error": {"code", "message", ...}}``.
Exit codes: 0 ok, 1 failed (drift, a FAIL, an unvalidated task), 2 refused
(bad id, a collision, an empty suite), 3 Docker unavailable, 4 a SKIP without a
FAIL, 5 not found, 7 the builder is locked, 8 the directory already exists.

The GUI shells out to these commands; it detects the group at a bench tag by
grepping ``harness/cli.py`` for ``register_suite_commands``. A later builder
(``harness/suite_builder/``) registers more subcommands through
:func:`register_suite_commands`; keep this module to the manifest, the lock
and the validator.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from harness.console import add_command
from harness.suite import SUITE_ALIASES, SUITE_NAME_ALIASES, resolve_suites_home
from harness.suite_lock import freeze_dir, lock_digest, read_lock, verify_lock

MANIFEST_SCHEMA = "vulcanbench.suite.v1"
ERROR_SCHEMA = "vulcanbench.suite.error.v1"
VALIDATE_FILE_SCHEMA = "vulcanbench.suite.validate.v1"
ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{2,63}$")
REPO_RE = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
#: The checkout's own suites: an external id must not shadow one (or an alias).
REPO_TASKS = Path(__file__).resolve().parents[1] / "tasks"
#: Validation statuses that admit a task without ``--force``.
ACCEPTED_STATUSES = ("PASS", "WARN")
STAGES = ("missing", "scaffolded", "authored", "validated", "admitted")

EXIT_OK = 0
EXIT_FAIL = 1
EXIT_REFUSED = 2
EXIT_DOCKER = 3
EXIT_SKIP = 4
EXIT_MISSING = 5
EXIT_LOCKED = 7
EXIT_EXISTS = 8

SUITES_HOME_HELP = (
    "Where suites built outside the checkout live "
    "(default: [runner].suites_home in vulcanbench.toml, ~/vulcanbench-suites)"
)


class SuiteCommandError(Exception):
    """A subcommand refusal: an error code, a message and the exit code."""

    def __init__(self, code: str, message: str, exit_code: int, **extra: Any) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.exit_code = exit_code
        self.extra = extra

    def as_document(self) -> dict[str, Any]:
        return {
            "schema": ERROR_SCHEMA,
            "error": {"code": self.code, "message": self.message, **self.extra},
        }


# ---------------------------------------------------------------------------
# Output: one JSON document on stdout, or plain text.
# ---------------------------------------------------------------------------


def _finish(json_output: bool, document: dict[str, Any], lines: list[str], exit_code: int) -> None:
    """Print the result and exit. With ``--json`` the document owns stdout."""
    if json_output:
        print(json.dumps(document, indent=2))
        for line in lines:
            print(line, file=sys.stderr)
    else:
        for line in lines:
            print(line)
    if exit_code:
        raise SystemExit(exit_code)


def _guarded(json_output: bool, body: Callable[[], None]) -> None:
    try:
        body()
    except SuiteCommandError as exc:
        if json_output:
            print(json.dumps(exc.as_document(), indent=2))
            print(f"error {exc.message}", file=sys.stderr)
        else:
            print(f"error {exc.message}")
        raise SystemExit(exc.exit_code) from exc


def _require_builder() -> None:
    """The password gate for every command that creates or changes a suite (exit 7)."""
    from harness.suite_builder.access import BuilderLocked, require_unlocked  # noqa: PLC0415

    try:
        require_unlocked()
    except BuilderLocked as exc:
        raise SuiteCommandError(exc.code, exc.message, EXIT_LOCKED) from exc


# ---------------------------------------------------------------------------
# Manifest and task state on disk.
# ---------------------------------------------------------------------------


def _now() -> str:
    return datetime.now(UTC).isoformat(timespec="seconds")


def _manifest_path(suite_dir: Path) -> Path:
    return suite_dir / "suite.json"


def _load_manifest(suite_dir: Path) -> dict[str, Any]:
    path = _manifest_path(suite_dir)
    if not path.is_file():
        raise SuiteCommandError(
            "not_found", f"suite {suite_dir.name!r} not found: no {path}", EXIT_MISSING
        )
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise SuiteCommandError("bad_manifest", f"{path} is not a JSON object", EXIT_REFUSED)
    data.setdefault("tasks", [])
    return data


def _save_manifest(suite_dir: Path, data: dict[str, Any]) -> None:
    data["updated_at"] = _now()
    _manifest_path(suite_dir).write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def _read_json(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def task_state(suite_dir: Path, task_id: str, manifest: dict[str, Any]) -> dict[str, Any]:
    """A task's stage, derived from what is on disk.

    ``scaffolded`` once ``metadata.json`` exists, ``authored`` once
    ``build/author.json`` exists, ``validated`` when ``build/validate.json``
    says PASS or WARN, ``admitted`` when the manifest lists it; ``missing``
    when it is listed but has no directory.
    """
    task_dir = suite_dir / task_id
    admitted = task_id in (manifest.get("tasks") or [])
    validation = _read_json(task_dir / "build" / "validate.json")
    status = str(validation.get("status")) if validation else None
    if not (task_dir / "metadata.json").is_file():
        stage = "missing"
    elif admitted:
        stage = "admitted"
    elif status in ACCEPTED_STATUSES:
        stage = "validated"
    elif (task_dir / "build" / "author.json").is_file():
        stage = "authored"
    else:
        stage = "scaffolded"
    return {
        "task_id": task_id,
        "stage": stage,
        "admitted": admitted,
        "admitted_unvalidated": task_id in (manifest.get("admitted_unvalidated") or []),
        "validation": None
        if validation is None
        else {
            "status": status,
            "sandbox": validation.get("sandbox"),
            "checked_at": validation.get("checked_at"),
            "task_hash": validation.get("task_hash"),
        },
        "path": str(task_dir),
    }


def _task_ids_on_disk(suite_dir: Path, manifest: dict[str, Any]) -> list[str]:
    listed = [str(t) for t in (manifest.get("tasks") or [])]
    on_disk = [
        d.name
        for d in sorted(suite_dir.iterdir())
        if d.is_dir() and (d / "metadata.json").is_file()
    ]
    return sorted(set(listed) | set(on_disk))


def _lock_block(suite_dir: Path) -> dict[str, Any]:
    lock = read_lock(suite_dir)
    verification = verify_lock(suite_dir)
    return {
        "locked": lock is not None,
        "version": verification.version,
        "frozen_at": verification.frozen_at,
        "lock_digest": verification.lock_digest,
        "n_tasks": verification.n_tasks,
        "problems": [] if lock is None else list(verification.problems),
    }


def _readme(suite_id: str, display_name: str) -> str:
    return (
        f"# {display_name}\n\n"
        f"VulcanBench suite `{suite_id}`, built outside the VulcanBench checkout.\n\n"
        "This directory is a suite home entry: `suite.json` is the manifest, each\n"
        "subdirectory is one task in the VulcanBench Frontier v4 task format\n"
        "(`metadata.json`, `issue.md`, `repo/` or `repo_snapshot.tar.gz`, hidden\n"
        "`tests/`, `gold_patch.diff`), and `suite.lock.json` (written by\n"
        "`vulcanbench suite freeze`) pins the task set a run is scored against.\n\n"
        "Per-task `build/` folders hold the builder's working files, including the\n"
        "answer key (the upstream fix, the author's notes, validation results). They\n"
        "never ship: nothing under `build/` is part of the task hash, and the agent\n"
        "workspace is prepared from `repo/` only.\n\n"
        "Do not commit this directory into VulcanBench. Runs record its path and the\n"
        "lock digest under `source.suite` in every `summary.json`.\n"
    )


# ---------------------------------------------------------------------------
# Subcommands. Each is a keyword-argument function wired by add_command.
# ---------------------------------------------------------------------------


def suite_init(
    *,
    suite_id: str,
    display_name: str,
    repo: str,
    version: str = "0.1.0",
    suites_home: Path | None = None,
    json_output: bool = False,
) -> None:
    """Create <suites_home>/<id>/ with an empty manifest and a README."""

    def body() -> None:
        _require_builder()
        if not ID_RE.match(suite_id):
            raise SuiteCommandError(
                "invalid_id",
                f"suite id {suite_id!r} must match {ID_RE.pattern} "
                "(lower-case letters, digits and hyphens, 3 to 64 characters)",
                EXIT_REFUSED,
            )
        if not REPO_RE.match(repo):
            raise SuiteCommandError(
                "invalid_repo", f"--repo must be owner/name, got {repo!r}", EXIT_REFUSED
            )
        if (
            suite_id in SUITE_ALIASES
            or suite_id in SUITE_NAME_ALIASES
            or (REPO_TASKS / suite_id).is_dir()
        ):
            raise SuiteCommandError(
                "id_collides",
                f"suite id {suite_id!r} is a suite or alias in the checkout's tasks/; "
                "an in-tree suite always wins, so pick another id",
                EXIT_REFUSED,
            )
        home = resolve_suites_home(suites_home)
        suite_dir = home / suite_id
        if suite_dir.exists():
            raise SuiteCommandError(
                "exists", f"{suite_dir} already exists", EXIT_EXISTS, path=str(suite_dir)
            )
        owner, name = repo.split("/", 1)
        now = _now()
        manifest = {
            "id": suite_id,
            "display_name": display_name,
            "version": version,
            "repo": {
                "host": "github",
                "owner": owner,
                "name": name,
                "url": f"https://github.com/{owner}/{name}",
            },
            "created_at": now,
            "updated_at": now,
            "tasks": [],
            "builder": {"schema": MANIFEST_SCHEMA},
        }
        suite_dir.mkdir(parents=True)
        _manifest_path(suite_dir).write_text(
            json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
        )
        (suite_dir / "README.md").write_text(_readme(suite_id, display_name), encoding="utf-8")
        _finish(
            json_output,
            {
                "schema": "vulcanbench.suite.init.v1",
                "id": suite_id,
                "path": str(suite_dir),
                "suites_home": str(home),
                "manifest": manifest,
            },
            [f"created {suite_dir} ({display_name}, version {version})"],
            EXIT_OK,
        )

    _guarded(json_output, body)


def suite_list(*, suites_home: Path | None = None, json_output: bool = False) -> None:
    """List every suite in the suites home with its lock state."""

    def body() -> None:
        home = resolve_suites_home(suites_home)
        rows: list[dict[str, Any]] = []
        for manifest_path in sorted(home.glob("*/suite.json")):
            suite_dir = manifest_path.parent
            manifest = _read_json(manifest_path) or {}
            lock = _lock_block(suite_dir)
            rows.append(
                {
                    "id": str(manifest.get("id") or suite_dir.name),
                    "display_name": str(manifest.get("display_name") or suite_dir.name),
                    "version": manifest.get("version"),
                    "n_tasks": len(manifest.get("tasks") or []),
                    "locked": lock["locked"],
                    "lock_digest": lock["lock_digest"],
                    "drift": lock["problems"],
                    "path": str(suite_dir),
                }
            )
        lines: list[str] = []
        if rows:
            lines.append(f"suites home: {home}")
            lines.append(
                _table(
                    ("Id", "Name", "Version", "Tasks", "Lock", "Path"),
                    [
                        (
                            r["id"],
                            r["display_name"],
                            r["version"] or "-",
                            r["n_tasks"],
                            "drift" if r["drift"] else ("frozen" if r["locked"] else "unfrozen"),
                            r["path"],
                        )
                        for r in rows
                    ],
                )
            )
        else:
            lines.append(f"no suites under {home}")
        _finish(
            json_output,
            {"schema": "vulcanbench.suite.list.v1", "suites_home": str(home), "suites": rows},
            lines,
            EXIT_OK,
        )

    _guarded(json_output, body)


def _table(columns: tuple[str, ...], rows: list[tuple[Any, ...]]) -> str:
    from harness.console import render_table  # noqa: PLC0415, keep the module header small

    return render_table(columns, rows)


def suite_show(
    *, suite_id: str, suites_home: Path | None = None, json_output: bool = False
) -> None:
    """Show a suite's manifest, lock state and the stage of every task on disk."""

    def body() -> None:
        home = resolve_suites_home(suites_home)
        suite_dir = home / suite_id
        manifest = _load_manifest(suite_dir)
        tasks = [task_state(suite_dir, t, manifest) for t in _task_ids_on_disk(suite_dir, manifest)]
        lock = _lock_block(suite_dir)
        lines = [
            f"{manifest.get('display_name') or suite_id} ({suite_id}) "
            f"version {manifest.get('version') or '-'} at {suite_dir}",
            "lock: "
            + (
                "unfrozen"
                if not lock["locked"]
                else f"{lock['lock_digest']} frozen {lock['frozen_at']}"
                + (f", DRIFT: {'; '.join(lock['problems'])}" if lock["problems"] else "")
            ),
        ]
        if tasks:
            lines.append(
                _table(
                    ("Task", "Stage", "Validation"),
                    [
                        (
                            t["task_id"],
                            t["stage"],
                            (t["validation"] or {}).get("status") or "-",
                        )
                        for t in tasks
                    ],
                )
            )
        else:
            lines.append("no tasks yet")
        _finish(
            json_output,
            {
                "schema": "vulcanbench.suite.show.v1",
                "id": suite_id,
                "suites_home": str(home),
                "path": str(suite_dir),
                "manifest": manifest,
                "lock": lock,
                "tasks": tasks,
            },
            lines,
            EXIT_OK,
        )

    _guarded(json_output, body)


def suite_add_task(
    *,
    suite_id: str,
    task: str,
    force: bool = False,
    suites_home: Path | None = None,
    json_output: bool = False,
) -> None:
    """Admit a validated task into the suite's task list.

    Refuses a task whose ``build/validate.json`` is missing or not PASS/WARN;
    ``--force`` admits it anyway and records it under ``admitted_unvalidated``
    so the manifest says which tasks skipped the validator.
    """

    def body() -> None:
        _require_builder()
        home = resolve_suites_home(suites_home)
        suite_dir = home / suite_id
        manifest = _load_manifest(suite_dir)
        state = task_state(suite_dir, task, manifest)
        if state["stage"] == "missing":
            raise SuiteCommandError(
                "not_found", f"task {task!r} has no metadata.json under {suite_dir}", EXIT_MISSING
            )
        status = (state["validation"] or {}).get("status")
        validated = status in ACCEPTED_STATUSES
        if not validated and not force:
            raise SuiteCommandError(
                "not_validated",
                f"task {task!r} is not validated (status {status or 'none'}); run "
                f"`vulcanbench suite validate {suite_id} --task {task}` first, "
                "or pass --force to admit it unvalidated",
                EXIT_FAIL,
                validation_status=status,
            )
        tasks = [str(t) for t in manifest.get("tasks") or []]
        if task not in tasks:
            tasks.append(task)
        manifest["tasks"] = tasks
        unvalidated = [str(t) for t in manifest.get("admitted_unvalidated") or []]
        if validated:
            unvalidated = [t for t in unvalidated if t != task]
        elif task not in unvalidated:
            unvalidated.append(task)
        if unvalidated:
            manifest["admitted_unvalidated"] = unvalidated
        else:
            manifest.pop("admitted_unvalidated", None)
        _save_manifest(suite_dir, manifest)
        note = "" if validated else " (unvalidated, --force)"
        _finish(
            json_output,
            {
                "schema": "vulcanbench.suite.add-task.v1",
                "id": suite_id,
                "task_id": task,
                "validation_status": status,
                "tasks": tasks,
                "admitted_unvalidated": unvalidated,
            },
            [f"admitted {task} into {suite_id}{note}: {len(tasks)} task(s)"],
            EXIT_OK,
        )

    _guarded(json_output, body)


def suite_remove_task(
    *,
    suite_id: str,
    task: str,
    suites_home: Path | None = None,
    json_output: bool = False,
) -> None:
    """Drop a task from the suite's task list (its directory is left alone)."""

    def body() -> None:
        _require_builder()
        home = resolve_suites_home(suites_home)
        suite_dir = home / suite_id
        manifest = _load_manifest(suite_dir)
        tasks = [str(t) for t in manifest.get("tasks") or []]
        if task not in tasks:
            raise SuiteCommandError(
                "not_found", f"task {task!r} is not in suite {suite_id!r}", EXIT_MISSING
            )
        tasks = [t for t in tasks if t != task]
        manifest["tasks"] = tasks
        unvalidated = [str(t) for t in manifest.get("admitted_unvalidated") or [] if str(t) != task]
        if unvalidated:
            manifest["admitted_unvalidated"] = unvalidated
        else:
            manifest.pop("admitted_unvalidated", None)
        _save_manifest(suite_dir, manifest)
        _finish(
            json_output,
            {
                "schema": "vulcanbench.suite.remove-task.v1",
                "id": suite_id,
                "task_id": task,
                "tasks": tasks,
                "admitted_unvalidated": unvalidated,
            },
            [f"removed {task} from {suite_id}: {len(tasks)} task(s) left"],
            EXIT_OK,
        )

    _guarded(json_output, body)


def suite_freeze(
    *, suite_id: str, suites_home: Path | None = None, json_output: bool = False
) -> None:
    """Write suite.lock.json: the version and the task hash of every admitted task."""

    def body() -> None:
        _require_builder()
        home = resolve_suites_home(suites_home)
        suite_dir = home / suite_id
        manifest = _load_manifest(suite_dir)
        tasks = [str(t) for t in manifest.get("tasks") or []]
        if not tasks:
            raise SuiteCommandError(
                "empty_suite",
                f"suite {suite_id!r} has no admitted tasks; add one with "
                f"`vulcanbench suite add-task {suite_id} --task <task>` before freezing",
                EXIT_REFUSED,
                problems=["tasks list is empty"],
            )
        try:
            path = freeze_dir(suite_dir)
        except FileNotFoundError as exc:
            raise SuiteCommandError(
                "task_missing",
                f"suite {suite_id!r} lists a task with no directory: {exc}",
                EXIT_REFUSED,
                problems=[str(exc)],
            ) from exc
        lock = read_lock(suite_dir) or {}
        digest = lock_digest(lock)
        _finish(
            json_output,
            {
                "schema": "vulcanbench.suite.freeze.v1",
                "id": suite_id,
                "path": str(path),
                "version": lock.get("version"),
                "lock_digest": digest,
                "n_tasks": len(lock.get("tasks") or {}),
            },
            [f"froze {path} ({len(tasks)} task(s), {digest})"],
            EXIT_OK,
        )

    _guarded(json_output, body)


def suite_check(
    *, suite_id: str, suites_home: Path | None = None, json_output: bool = False
) -> None:
    """Verify that suite.lock.json still matches the tasks on disk (exit 1 on drift)."""

    def body() -> None:
        home = resolve_suites_home(suites_home)
        suite_dir = home / suite_id
        _load_manifest(suite_dir)
        verification = verify_lock(suite_dir)
        lines = [f"ok   {suite_id} {verification.lock_digest}"]
        if not verification.ok:
            lines = [f"FAIL {suite_id}", *(f"  {p}" for p in verification.problems)]
        _finish(
            json_output,
            {
                "schema": "vulcanbench.suite.check.v1",
                "id": suite_id,
                "ok": verification.ok,
                "problems": list(verification.problems),
                "lock_digest": verification.lock_digest,
                "version": verification.version,
                "frozen_at": verification.frozen_at,
                "n_tasks": verification.n_tasks,
            },
            lines,
            EXIT_OK if verification.ok else EXIT_FAIL,
        )

    _guarded(json_output, body)


BUILDER_CHECK_REASONS = {
    "no_answer_in_repo": "the fix is already in repo/ (a .git directory, or gold_patch.diff reverse-applies)",
    "gold_applies": "gold_patch.diff does not apply to repo/",
}


def builder_checks(task_dir: Path) -> dict[str, bool | None]:
    """Answer-leak checks the validator does not make; ``None`` where they do not apply.

    ``no_answer_in_repo``: ``repo/`` has no ``.git`` and the gold patch does not
    already reverse-apply (the starting state is really before the fix).
    ``gold_applies``: ``git apply --check`` of the gold patch against ``repo/``.
    """
    from harness.suite_builder.scaffold import gold_applies  # noqa: PLC0415

    repo_dir = task_dir / "repo"
    gold = task_dir / "gold_patch.diff"
    if not repo_dir.is_dir() or not gold.is_file():
        return {"no_answer_in_repo": None, "gold_applies": None}
    has_git = any(p.name == ".git" for p in repo_dir.rglob(".git"))
    applies = gold_applies(repo_dir, gold)
    already = gold_applies(repo_dir, gold, reverse=True)
    return {"no_answer_in_repo": not has_git and not already, "gold_applies": applies}


def suite_validate(
    *,
    suite_id: str,
    task: str | None = None,
    sandbox: str = "local",
    image: str | None = None,
    suites_home: Path | None = None,
    json_output: bool = False,
) -> None:
    """Run the task validator on every admitted task, or on --task, recording each result.

    Writes ``<task>/build/validate.json`` per task. Exit 0 when every task is
    PASS or WARN, 1 on any FAIL, 3 when Docker is unavailable, 4 when a task
    was skipped (a missing toolchain) and none failed.
    """

    def body() -> None:
        _require_builder()
        if sandbox not in {"local", "docker"}:
            raise SuiteCommandError(
                "invalid_sandbox", f"--sandbox must be local|docker, got {sandbox!r}", EXIT_REFUSED
            )
        home = resolve_suites_home(suites_home)
        suite_dir = home / suite_id
        manifest = _load_manifest(suite_dir)
        task_ids = [task] if task else [str(t) for t in manifest.get("tasks") or []]
        for tid in task_ids:
            if not (suite_dir / tid / "metadata.json").is_file():
                raise SuiteCommandError(
                    "not_found",
                    f"task {tid!r} has no metadata.json under {suite_dir}",
                    EXIT_MISSING,
                )
        if sandbox == "docker":
            from harness.sandbox.docker_executor import _docker_available  # noqa: PLC0415

            if not _docker_available():
                raise SuiteCommandError(
                    "docker_unavailable",
                    "--sandbox docker requires a running Docker daemon",
                    EXIT_DOCKER,
                )
        # harness.validate pulls in the sandbox and environment modules; load
        # it only here so `vulcanbench --help` stays fast.
        from harness.tasks import load_task, task_hash  # noqa: PLC0415
        from harness.validate import ValidateOptions, result_to_dict, validate_task  # noqa: PLC0415

        opts = ValidateOptions(sandbox=sandbox, image=image)
        results: list[dict[str, Any]] = []
        lines: list[str] = []
        for tid in task_ids:
            task_dir = suite_dir / tid
            res = validate_task(task_dir, opts)
            checks = builder_checks(task_dir)
            failed_checks = [name for name, ok in checks.items() if ok is False]
            if failed_checks:
                res.status = "FAIL"
                res.reasons = [*res.reasons, *(BUILDER_CHECK_REASONS[n] for n in failed_checks)]
            record = {
                "schema": VALIDATE_FILE_SCHEMA,
                **result_to_dict(res),
                "checks": checks,
                "sandbox": sandbox,
                "image": image,
                "task_hash": task_hash(load_task(tid, suite_dir)),
                "checked_at": _now(),
            }
            build = task_dir / "build"
            build.mkdir(exist_ok=True)
            (build / "validate.json").write_text(
                json.dumps(record, indent=2) + "\n", encoding="utf-8"
            )
            results.append(record)
            suffix = f", {'; '.join(res.reasons)}" if res.reasons else ""
            lines.append(f"  {res.status:4} {tid}{suffix}")
        counts = {
            key: sum(r["status"] == key.upper() for r in results)
            for key in ("pass", "skip", "warn", "fail")
        }
        if counts["fail"]:
            code = EXIT_FAIL
        elif counts["skip"]:
            code = EXIT_SKIP
        else:
            code = EXIT_OK
        lines.append(
            f"{counts['pass']} passed, {counts['skip']} skipped, "
            f"{counts['warn']} warned, {counts['fail']} failed"
        )
        _finish(
            json_output,
            {
                "schema": "vulcanbench.suite.validate-report.v1",
                "id": suite_id,
                "sandbox": sandbox,
                "image": image,
                "results": results,
                "counts": counts,
            },
            lines,
            code,
        )

    _guarded(json_output, body)


# ---------------------------------------------------------------------------
# Wiring.
# ---------------------------------------------------------------------------


def _common(p: argparse.ArgumentParser) -> None:
    p.add_argument(
        "--suites-home", dest="suites_home", type=Path, default=None, help=SUITES_HOME_HELP
    )
    p.add_argument(
        "--json",
        dest="json_output",
        action="store_true",
        help="One JSON document on stdout (human text on stderr)",
    )


def register_suite_commands(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> None:
    """Wire ``vulcanbench suite <command>``; the extension point for more subcommands."""
    p = add_command(subparsers, "init", suite_init)
    p.add_argument("suite_id", metavar="id", help="Suite id: ^[a-z0-9][a-z0-9-]{2,63}$")
    p.add_argument("--display-name", dest="display_name", required=True, help="Human name")
    p.add_argument("--repo", dest="repo", required=True, help="Source repository, owner/name")
    p.add_argument("--version", dest="version", default="0.1.0", help="Suite version")
    _common(p)

    p = add_command(subparsers, "list", suite_list)
    _common(p)

    p = add_command(subparsers, "show", suite_show)
    p.add_argument("suite_id", metavar="id")
    _common(p)

    p = add_command(subparsers, "add-task", suite_add_task)
    p.add_argument("suite_id", metavar="id")
    p.add_argument("--task", dest="task", required=True, help="Task directory name")
    p.add_argument(
        "--force",
        dest="force",
        action="store_true",
        help="Admit an unvalidated task (recorded under admitted_unvalidated)",
    )
    _common(p)

    p = add_command(subparsers, "remove-task", suite_remove_task)
    p.add_argument("suite_id", metavar="id")
    p.add_argument("--task", dest="task", required=True, help="Task directory name")
    _common(p)

    p = add_command(subparsers, "freeze", suite_freeze)
    p.add_argument("suite_id", metavar="id")
    _common(p)

    p = add_command(subparsers, "check", suite_check)
    p.add_argument("suite_id", metavar="id")
    _common(p)

    p = add_command(subparsers, "validate", suite_validate)
    p.add_argument("suite_id", metavar="id")
    p.add_argument(
        "--task", dest="task", default=None, help="One task instead of every admitted one"
    )
    p.add_argument("--sandbox", dest="sandbox", default="local", help="local|docker")
    p.add_argument("--image", dest="image", default=None, help="Docker image for --sandbox docker")
    _common(p)

    from harness.suite_builder.commands import register_builder_commands  # noqa: PLC0415

    register_builder_commands(subparsers)
