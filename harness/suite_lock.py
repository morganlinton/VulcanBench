"""Suite locks: a published suite's scoring-relevant content cannot drift.

Freezing a suite writes ``suite.lock.json`` next to its ``suite.json``: the
suite's version (from the manifest) and the ``task_hash`` of every task in it.
``check`` fails if a locked suite no longer matches, so CI rejects a PR that
edits a published task in place. Cosmetic metadata is outside ``task_hash``
and stays editable; the prompt, repo, tests, verifier, gold patch or the task
set are not.

Harbor-native tasks (``task.toml`` + ``instruction.md``, as in Frontier v5) are
hashed by ``harbor_task_hash``: the prompt, ``task.toml`` without its
descriptive ``[metadata]`` table, and the ``environment/``, ``tests/`` and
``solution/`` trees. Their task version and determinism checklist are locked
alongside the hash.

A suite can also name private tasks (suite.json ``private_tasks``) whose
directories live in a private companion repo and never in this one. The lock
records their hash; ``freeze`` needs the private checkout (``--private-root``
or ``VULCANBENCH_PRIVATE_TASKS``, pointing at its ``tasks/`` directory), and
``check`` verifies them only where that checkout is present.

A suite.json ``pins`` object (reference models, CLI versions, image ids) is
copied into the lock verbatim and must not change without a version bump.

Changing a frozen suite on purpose means bumping ``version`` in its suite.json
and re-freezing, in the same PR. Results are comparable only within a version
(tasks/coding-intelligence-index-v4/CHARTER.md). See docs/HOW_WE_WORK.md.

Two forms of every operation exist. The *in-tree* form takes a suite name
under ``tasks/`` (``freeze``, ``check_suite``, ``compute_lock``) and is what
CI and ``make freeze-suite`` call. The *directory* form takes the suite
directory itself (``freeze_dir``, ``check_dir``, ``compute_lock_dir``,
``verify_lock``) and serves suites built outside the checkout
(``vulcanbench suite ...``). Both write the same lock file shape.

A suite outside the checkout has no git history to vouch for it, so its lock
is the only thing that ties a result to a task set: ``lock_digest`` is a
content digest of the lock (version plus the sorted task hashes, independent
of file formatting) that every run summary records, and ``require_locked``
refuses to run an external suite whose lock is missing or no longer matches
the tasks on disk. There is deliberately no override flag and no environment
escape hatch (docs/DECISIONS.md, 2026-11-02).

    python -m harness.suite_lock check
    python -m harness.suite_lock freeze coding-intelligence-index-v4
    python -m harness.suite_lock --private-root ../vulcanbench-frontier-private/tasks freeze frontier-v5
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import tomllib
from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from harness.suite import DEFAULT_TASKS_BASE, Suite, load_suite, load_suite_dir
from harness.tasks import _hash_dir, load_task, task_hash

LOCK_NAME = "suite.lock.json"
PRIVATE_ENV = "VULCANBENCH_PRIVATE_TASKS"
#: Lock keys besides ``tasks`` that are part of its content (Harbor suites only).
_CONTENT_EXTRAS = ("task_meta", "private_tasks", "pins")


class SuiteLockError(RuntimeError):
    """An external suite was asked to run without a lock that matches its tasks."""


def suite_manifest(tasks_root: Path) -> dict[str, Any]:
    manifest = tasks_root / "suite.json"
    if not manifest.is_file():
        return {}
    data = json.loads(manifest.read_text(encoding="utf-8"))
    return data if isinstance(data, dict) else {}


def suite_version(tasks_root: Path) -> str | None:
    version = suite_manifest(tasks_root).get("version")
    return str(version) if version is not None else None


def is_harbor_task(root: Path) -> bool:
    return (root / "task.toml").is_file() and (root / "instruction.md").is_file()


def _harbor_config(root: Path) -> dict[str, Any]:
    return tomllib.loads((root / "task.toml").read_text(encoding="utf-8"))


def harbor_task_hash(root: Path) -> str:
    """A deterministic sha256 of a Harbor-native task's scoring-relevant definition.

    Covers the prompt (``instruction.md``), every ``task.toml`` setting except the
    descriptive ``[metadata]`` table (status notes, records and family notes change
    after a gate without changing the task), and the ``environment/``, ``tests/``
    and ``solution/`` trees. ``DESIGN.md`` and other top-level notes are outside it.
    """
    h = hashlib.sha256()
    h.update(b"harbor\0instruction\0")
    h.update((root / "instruction.md").read_bytes())
    config = _harbor_config(root)
    config.pop("metadata", None)
    h.update(b"\0task_toml\0")
    h.update(json.dumps(config, sort_keys=True).encode())
    h.update(b"\0")
    for sub in ("environment", "tests", "solution"):
        if (root / sub).is_dir():
            _hash_dir(h, sub, root / sub)
    return h.hexdigest()


def harbor_task_meta(root: Path) -> dict[str, Any]:
    config = _harbor_config(root)
    task = config.get("task") or {}
    metadata = config.get("metadata") or {}
    return {
        "version": task.get("version"),
        "checklist": metadata.get("vulcanbench_determinism_checklist"),
    }


def private_root_from_env() -> Path | None:
    raw = os.environ.get(PRIVATE_ENV)
    return Path(raw) if raw else None


def read_lock(suite_dir: Path) -> dict[str, Any] | None:
    """The parsed ``suite.lock.json`` of ``suite_dir``, or ``None`` when there is none."""
    path = suite_dir / LOCK_NAME
    if not path.is_file():
        return None
    data = json.loads(path.read_text(encoding="utf-8"))
    return data if isinstance(data, dict) else None


def lock_digest(lock: dict[str, Any]) -> str:
    """``sha256:<hex>`` over the lock's *content*: its version and sorted task hashes.

    Computed from canonical JSON of ``{"version": ..., "tasks": {...sorted}}``,
    never from the file's bytes, so re-serialising the lock (indentation, key
    order, the ``frozen_at`` date or the note) leaves the digest unchanged
    while any task hash or the version changes it. A Harbor suite's
    ``task_meta``, ``private_tasks`` and ``pins`` join the digest when the
    lock has them, so a lock without them digests exactly as before.
    """
    tasks = lock.get("tasks") or {}
    canonical: dict[str, Any] = {
        "version": lock.get("version"),
        "tasks": {str(k): str(v) for k, v in sorted(tasks.items())},
    }
    for key in _CONTENT_EXTRAS:
        if key in lock:
            canonical[key] = lock[key]
    payload = json.dumps(canonical, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def lock_for(suite: Suite, private_root: Path | None = None) -> dict[str, Any]:
    """The lock content for a loaded suite: ``{suite, version, tasks, ...}``.

    Harbor-native tasks are hashed with :func:`harbor_task_hash` and add
    ``task_meta``; suite.json ``private_tasks`` and ``pins`` are carried over
    (a private task's ``hash`` is ``None`` when ``private_root`` cannot see
    it). Raises ``FileNotFoundError`` naming the task when a listed task has
    no directory on disk.
    """
    manifest = suite_manifest(suite.tasks_root)
    hashes: dict[str, str] = {}
    task_meta: dict[str, dict[str, Any]] = {}
    for tid in suite.task_ids:
        root = suite.tasks_root / tid
        if is_harbor_task(root):
            hashes[tid] = harbor_task_hash(root)
            task_meta[tid] = harbor_task_meta(root)
            continue
        try:
            hashes[tid] = task_hash(load_task(tid, suite.tasks_root))
        except FileNotFoundError as exc:
            raise FileNotFoundError(f"task missing: {tid} ({root})") from exc
    private: dict[str, dict[str, Any]] = {}
    for tid, source in (manifest.get("private_tasks") or {}).items():
        entry: dict[str, Any] = dict(source)
        proot = private_root / suite.name / tid if private_root is not None else None
        if proot is not None and is_harbor_task(proot):
            entry["hash"] = harbor_task_hash(proot)
            entry.update(harbor_task_meta(proot))
        else:
            entry["hash"] = None
        private[tid] = entry
    lock: dict[str, Any] = {
        "suite": suite.name,
        "version": suite.version if suite.version is not None else suite_version(suite.tasks_root),
        "tasks": hashes,
    }
    if task_meta:
        lock["task_meta"] = task_meta
    if private:
        lock["private_tasks"] = private
    if "pins" in manifest:
        lock["pins"] = manifest["pins"]
    return lock


def compute_lock(
    name: str, tasks_base: Path = DEFAULT_TASKS_BASE, private_root: Path | None = None
) -> dict[str, Any]:
    return lock_for(load_suite(name, tasks_base), private_root)


def compute_lock_dir(suite_dir: Path) -> dict[str, Any]:
    """Directory form of :func:`compute_lock` for a suite outside the checkout."""
    return lock_for(load_suite_dir(suite_dir, external=True))


def _write_lock(suite: Suite, lock: dict[str, Any]) -> Path:
    missing = [tid for tid, e in (lock.get("private_tasks") or {}).items() if e["hash"] is None]
    if missing:
        raise FileNotFoundError(
            f"private task(s) {', '.join(missing)} not found: pass --private-root (or set "
            f"{PRIVATE_ENV}) to the private repo's tasks/ directory"
        )
    out: dict[str, Any] = {
        "suite": lock["suite"],
        "version": lock["version"],
        "frozen_at": datetime.now(UTC).date().isoformat(),
        "note": (
            "Generated by python -m harness.suite_lock freeze. Do not edit by hand. "
            "To change this suite, bump version in suite.json and re-freeze in the "
            "same PR (docs/HOW_WE_WORK.md)."
        ),
    }
    out.update({k: v for k, v in lock.items() if k not in ("suite", "version")})
    path = suite.tasks_root / LOCK_NAME
    path.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
    return path


def freeze(
    name: str, tasks_base: Path = DEFAULT_TASKS_BASE, private_root: Path | None = None
) -> Path:
    suite = load_suite(name, tasks_base)
    return _write_lock(suite, lock_for(suite, private_root))


def freeze_dir(suite_dir: Path) -> Path:
    """Directory form of :func:`freeze`: write ``suite.lock.json`` into ``suite_dir``."""
    suite = load_suite_dir(suite_dir, external=True)
    return _write_lock(suite, lock_for(suite))


@dataclass
class LockDrift:
    suite: str
    problems: list[str] = field(default_factory=list)
    #: Private tasks the lock names that this checkout cannot see (not checked).
    unchecked: list[str] = field(default_factory=list)


def _check_harbor_extras(locked: dict[str, Any], current: dict[str, Any], drift: LockDrift) -> None:
    """Task meta, private tasks and pins: the parts of a lock only Harbor suites carry."""
    old_meta: dict[str, Any] = locked.get("task_meta") or {}
    for tid, meta in (current.get("task_meta") or {}).items():
        if tid in old_meta and old_meta[tid] != meta:
            drift.problems.append(f"task version or checklist changed: {tid}")
    old_private: dict[str, Any] = locked.get("private_tasks") or {}
    new_private: dict[str, Any] = current.get("private_tasks") or {}
    for tid in sorted(set(new_private) - set(old_private)):
        drift.problems.append(f"private task added: {tid}")
    for tid in sorted(set(old_private) - set(new_private)):
        drift.problems.append(f"private task removed: {tid}")
    for tid in sorted(set(old_private) & set(new_private)):
        entry = new_private[tid]
        if entry["hash"] is None:
            source = {k: v for k, v in entry.items() if k != "hash"}
            if any(old_private[tid].get(k) != v for k, v in source.items()):
                drift.problems.append(f"private task source changed: {tid}")
            else:
                drift.unchecked.append(tid)
        elif entry != old_private[tid]:
            drift.problems.append(f"private task changed: {tid}")
    if locked.get("pins") != current.get("pins"):
        drift.problems.append("pins changed in suite.json")


def _drift(suite: Suite, locked: dict[str, Any], private_root: Path | None = None) -> LockDrift:
    drift = LockDrift(suite=suite.name)
    try:
        current = lock_for(suite, private_root)
    except FileNotFoundError as exc:
        drift.problems.append(str(exc))
        return drift
    if locked.get("version") != current["version"]:
        drift.problems.append(
            f"version is {current['version']!r} in suite.json but {locked.get('version')!r} "
            "in the lock: re-freeze after a version bump"
        )
        return drift
    old: dict[str, str] = locked.get("tasks") or {}
    new: dict[str, str] = current["tasks"]
    for tid in sorted(set(new) - set(old)):
        drift.problems.append(f"task added: {tid}")
    for tid in sorted(set(old) - set(new)):
        drift.problems.append(f"task removed: {tid}")
    for tid in sorted(set(old) & set(new)):
        if old[tid] != new[tid]:
            drift.problems.append(f"task changed: {tid}")
    _check_harbor_extras(locked, current, drift)
    return drift


def check_suite(
    name: str, tasks_base: Path = DEFAULT_TASKS_BASE, private_root: Path | None = None
) -> LockDrift:
    suite = load_suite(name, tasks_base)
    locked = json.loads((suite.tasks_root / LOCK_NAME).read_text(encoding="utf-8"))
    return _drift(suite, locked, private_root)


def check_dir(suite_dir: Path) -> LockDrift:
    """Directory form of :func:`check_suite`; a missing lock is one problem, not a crash."""
    suite = load_suite_dir(suite_dir, external=True)
    locked = read_lock(suite_dir)
    if locked is None:
        return LockDrift(suite=suite.name, problems=[_missing_lock_problem(suite)])
    return _drift(suite, locked)


@dataclass
class LockVerification:
    """What :func:`verify_lock` found: the lock's identity and any drift."""

    suite: str
    version: str | None
    frozen_at: str | None
    lock_digest: str | None
    n_tasks: int
    problems: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.problems

    def as_dict(self) -> dict[str, Any]:
        return {
            "suite": self.suite,
            "version": self.version,
            "frozen_at": self.frozen_at,
            "lock_digest": self.lock_digest,
            "n_tasks": self.n_tasks,
            "ok": self.ok,
            "problems": list(self.problems),
        }


def _missing_lock_problem(suite: Suite) -> str:
    return (
        f"no {LOCK_NAME} in {suite.tasks_root}: the suite is not frozen; "
        f"run `vulcanbench suite freeze {suite.name}`"
    )


def _verify(suite: Suite) -> LockVerification:
    locked = read_lock(suite.tasks_root)
    if locked is None:
        return LockVerification(
            suite=suite.name,
            version=suite.version,
            frozen_at=None,
            lock_digest=None,
            n_tasks=len(suite.task_ids),
            problems=[_missing_lock_problem(suite)],
        )
    drift = _drift(suite, locked)
    raw_version = locked.get("version")
    return LockVerification(
        suite=suite.name,
        version=None if raw_version is None else str(raw_version),
        frozen_at=locked.get("frozen_at"),
        lock_digest=lock_digest(locked),
        n_tasks=len(locked.get("tasks") or {}),
        problems=list(drift.problems),
    )


def verify_lock(suite_dir: Path) -> LockVerification:
    """Verify the suite in ``suite_dir`` against its lock.

    A missing lock is a problem (the message says to run
    ``vulcanbench suite freeze <id>``); drift names the task that changed, was
    added or was removed. ``lock_digest`` is the digest of the lock as it is on
    disk, so a caller that sees ``ok`` can record it as the task set it ran.
    """
    return _verify(load_suite_dir(suite_dir, external=True))


def require_locked(suite: Suite) -> LockVerification:
    """Refuse to run an external suite whose lock is missing or does not match.

    Raises :class:`SuiteLockError` listing every problem. No flag or
    environment variable bypasses this: a result from a drifted suite would be
    tied to a task set that no longer exists.
    """
    verification = _verify(suite)
    if verification.ok:
        return verification
    shown = "\n  ".join(verification.problems)
    raise SuiteLockError(
        f"suite {suite.name!r} is not frozen to its tasks on disk:\n  {shown}\n"
        "Fix the task set, then `vulcanbench suite freeze "
        f"{suite.name}` (bump version in suite.json when a published task changed)."
    )


def locked_suites(tasks_base: Path = DEFAULT_TASKS_BASE) -> list[str]:
    return sorted(p.parent.name for p in tasks_base.glob(f"*/{LOCK_NAME}"))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m harness.suite_lock", description=__doc__)
    parser.add_argument("--tasks-base", type=Path, default=DEFAULT_TASKS_BASE)
    parser.add_argument(
        "--private-root",
        type=Path,
        default=None,
        help=f"the private repo's tasks/ directory (default: ${PRIVATE_ENV})",
    )
    sub = parser.add_subparsers(dest="cmd", required=True)
    p_check = sub.add_parser("check", help="verify locked suites (all, or the ones named)")
    p_check.add_argument("suites", nargs="*")
    p_freeze = sub.add_parser("freeze", help="write or refresh a suite's lock")
    p_freeze.add_argument("suite")
    args = parser.parse_args(argv)
    private_root = args.private_root or private_root_from_env()

    if args.cmd == "freeze":
        path = freeze(args.suite, args.tasks_base, private_root)
        print(f"froze {path}")
        return 0

    names = args.suites or locked_suites(args.tasks_base)
    failed = False
    for name in names:
        drift = check_suite(name, args.tasks_base, private_root)
        if drift.problems:
            failed = True
            print(f"FAIL {name}")
            for problem in drift.problems:
                print(f"  {problem}")
        else:
            note = (
                f" (private, not present here: {', '.join(drift.unchecked)})"
                if drift.unchecked
                else ""
            )
            print(f"ok   {name}{note}")
    if failed:
        print(
            "\nA frozen suite changed. If that is intended, bump version in its suite.json "
            "and run: python -m harness.suite_lock freeze <suite>\n"
            "If you changed nothing, look for ignored or untracked files inside the task "
            "dirs (git status --ignored tasks/<suite>); task_hash counts every file "
            "under repo/ and tests/ except local byproducts such as __pycache__ and "
            ".DS_Store. A fresh checkout or run worktree is the reference.",
            file=sys.stderr,
        )
    return 1 if failed else 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
