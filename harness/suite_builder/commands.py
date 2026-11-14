"""``vulcanbench suite`` builder subcommands: access, prs list, task scaffold, task author.

Same conventions as :mod:`harness.suite_cli`: ``--json`` puts exactly one
document on stdout, errors are ``vulcanbench.suite.error.v1``. Exit codes:
0 ok, 1 failed (a provider or quota error during authoring), 2 refused, 3 not
ready (gh not logged in, harness not ready), 4 a gh or git failure, 5 not
found, 6 the authoring session made no accepted change, 7 the builder is
locked or the password is wrong, 8 the task directory exists.
"""

from __future__ import annotations

import argparse
import getpass
import json
import sys
from pathlib import Path
from typing import Any

from harness.console import add_command, add_group
from harness.suite import resolve_suites_home
from harness.suite_builder import access
from harness.suite_builder.gh import GhError
from harness.suite_cli import (
    EXIT_LOCKED,
    EXIT_MISSING,
    EXIT_REFUSED,
    SuiteCommandError,
    _common,
    _finish,
    _guarded,
    _load_manifest,
    _require_builder,
)

_GH_EXIT = {"gh_missing": 3, "gh_auth": 3, "not_found": 5, "gh_failed": 4}


def _read_password(*, from_stdin: bool, confirm: bool = False) -> str:
    """One password from stdin (``--password-stdin``) or a terminal prompt; never from argv."""
    if from_stdin:
        line = sys.stdin.readline()
        return line[:-1] if line.endswith("\n") else line
    if not sys.stdin.isatty():
        raise SuiteCommandError(
            "no_terminal",
            "no terminal to ask for the password; pass --password-stdin",
            EXIT_REFUSED,
        )
    first = getpass.getpass("Suite builder password: ")
    if confirm and getpass.getpass("Again: ") != first:
        raise SuiteCommandError("password_mismatch", "the two passwords differ", EXIT_REFUSED)
    return first


# --- access ------------------------------------------------------------------


def suite_access(*, json_output: bool = False) -> None:
    """Show whether the suite builder is unlocked on this machine (read only)."""

    def body() -> None:
        st = access.status()
        if not st["configured"]:
            line = "locked: no suite builder password is set ([builder].password_hash)"
        elif st["unlocked"]:
            line = f"unlocked since {st['unlocked_at']}"
        else:
            line = "locked: run `vulcanbench suite unlock`"
        _finish(json_output, st, [line], 0)

    _guarded(json_output, body)


def suite_unlock(*, password_stdin: bool = False, json_output: bool = False) -> None:
    """Unlock the suite builder on this machine with the subscriber password.

    The password comes from a prompt, or one line on stdin with
    --password-stdin; it is never accepted as an argument and never stored.
    """

    def body() -> None:
        password = _read_password(from_stdin=password_stdin)
        try:
            st = access.unlock(password)
        except access.BuilderLocked as exc:
            raise SuiteCommandError(exc.code, exc.message, EXIT_LOCKED) from exc
        except ValueError as exc:
            raise SuiteCommandError("bad_hash", str(exc), EXIT_REFUSED) from exc
        _finish(json_output, st, ["unlocked"], 0)

    _guarded(json_output, body)


def suite_lock(*, json_output: bool = False) -> None:
    """Lock the suite builder on this machine again (removes the unlock record)."""

    def body() -> None:
        _finish(json_output, access.lock(), ["locked"], 0)

    _guarded(json_output, body)


def suite_password_hash(*, password_stdin: bool = False, json_output: bool = False) -> None:
    """Print the scrypt hash of a new builder password, for [builder].password_hash.

    For the owner: run it, paste the printed hash into vulcanbench.toml, and
    share the password with subscribers. The password itself is not printed.
    """

    def body() -> None:
        password = _read_password(from_stdin=password_stdin, confirm=not password_stdin)
        try:
            hashed = access.hash_password(password)
        except ValueError as exc:
            raise SuiteCommandError("weak_password", str(exc), EXIT_REFUSED) from exc
        _finish(
            json_output,
            {"schema": "vulcanbench.suite.password-hash.v1", "password_hash": hashed},
            [hashed],
            0,
        )

    _guarded(json_output, body)


# --- prs list ----------------------------------------------------------------


def _scaffolded_prs(suite_dir: Path) -> dict[int, str]:
    out: dict[int, str] = {}
    if not suite_dir.is_dir():
        return out
    for meta_path in suite_dir.glob("*/metadata.json"):
        try:
            meta = json.loads(meta_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        pr = (meta.get("builder") or {}).get("pr") if isinstance(meta, dict) else None
        if isinstance(pr, int):
            out[pr] = meta_path.parent.name
    return out


def _repo_for(
    suite_id: str | None, repo: str | None, suites_home: Path | None
) -> tuple[str, Path | None]:
    if suite_id is None:
        if not repo:
            raise SuiteCommandError(
                "missing_repo", "pass --repo owner/name or --suite <id>", EXIT_REFUSED
            )
        return repo, None
    suite_dir = resolve_suites_home(suites_home) / suite_id
    manifest = _load_manifest(suite_dir)
    raw = manifest.get("repo")
    block: dict[str, Any] = raw if isinstance(raw, dict) else {}
    found = (
        f"{block.get('owner')}/{block.get('name')}"
        if block.get("owner") and block.get("name")
        else None
    )
    if repo is None and found is None:
        raise SuiteCommandError(
            "missing_repo", f"suite {suite_id} names no repository", EXIT_REFUSED
        )
    return repo or str(found), suite_dir


def suite_prs_list(
    *,
    repo: str | None = None,
    suite_id: str | None = None,
    since: str | None = None,
    limit: int = 50,
    min_loc: int = 10,
    max_loc: int = 400,
    max_files: int = 20,
    suites_home: Path | None = None,
    json_output: bool = False,
) -> None:
    """List a repository's merged pull requests, scored as task candidates (no quota).

    With --suite the repository comes from the suite's suite.json, and pull
    requests that already have a task in it name that task.
    """

    def body() -> None:
        _require_builder()
        from harness.suite_builder.prs import list_prs  # noqa: PLC0415

        name, suite_dir = _repo_for(suite_id, repo, suites_home)
        try:
            prs = list_prs(
                name,
                since=since,
                limit=limit,
                min_loc=min_loc,
                max_loc=max_loc,
                max_files=max_files,
            )
        except ValueError as exc:
            raise SuiteCommandError("invalid_option", str(exc), EXIT_REFUSED) from exc
        except GhError as exc:
            raise SuiteCommandError(exc.code, exc.message, _GH_EXIT[exc.code]) from exc
        done = _scaffolded_prs(suite_dir) if suite_dir is not None else {}
        for pr in prs:
            pr["task_id"] = done.get(pr["number"])
        lines = [
            f"  #{p['number']:<6} {p['suitability']['score']:>3}  {p['title'][:70]}"
            + (f"  (task {p['task_id']})" if p["task_id"] else "")
            for p in prs
        ]
        _finish(
            json_output,
            {"schema": "vulcanbench.suite.prs.v1", "repo": name, "since": since, "prs": prs},
            [f"{len(prs)} merged pull request(s) in {name}", *lines],
            0,
        )

    _guarded(json_output, body)


# --- task scaffold / author --------------------------------------------------


def _csv(value: str | None) -> list[str]:
    return [v.strip() for v in (value or "").split(",") if v.strip()]


def suite_task_scaffold(
    *,
    suite_id: str,
    pr: int,
    repo: str | None = None,
    task_id: str | None = None,
    paths: str | None = None,
    exclude: str | None = None,
    strip_tests: str = "pr",
    snapshot: bool = False,
    license_note: str | None = None,
    clone_dir: Path | None = None,
    suites_home: Path | None = None,
    json_output: bool = False,
) -> None:
    """Scaffold a task from a merged pull request (no quota; needs gh and network).

    Writes repo/ at the pull request's base commit, a draft issue.md, the
    gold patch, metadata.json and the builder's build/ folder. The hidden
    tests come from `suite task author`.
    """

    def body() -> None:
        _require_builder()
        from harness.suite_builder.scaffold import ScaffoldError, scaffold_task  # noqa: PLC0415

        name, suite_dir = _repo_for(suite_id, repo, suites_home)
        assert suite_dir is not None
        try:
            report = scaffold_task(
                suite_dir=suite_dir,
                repo=name,
                pr_number=pr,
                task_id=task_id,
                paths=_csv(paths),
                exclude=_csv(exclude),
                strip_tests=strip_tests,
                snapshot=snapshot,
                license_note=license_note,
                clone_dir=clone_dir,
            )
        except ScaffoldError as exc:
            raise SuiteCommandError(exc.code, exc.message, exc.exit_code) from exc
        lines = [
            f"scaffolded {report['task_id']} at {report['base_commit'][:12]} ({report['base_commit_method']})"
        ]
        lines += [f"  warning {w}" for w in report["warnings"]]
        _finish(json_output, report, lines, 0)

    _guarded(json_output, body)


def suite_task_author(
    *,
    suite_id: str,
    task: str,
    harness: str,
    model: str,
    billing: str = "auto",
    effort: str | None = None,
    timeout: int = 3600,
    max_turns: int = 200,
    suites_home: Path | None = None,
    json_output: bool = False,
) -> None:
    """Run one model session that writes a scaffolded task's issue, hidden tests and metadata.

    Spends model quota through the chosen CLI harness, once; it never retries
    and never validates. Writes <task>/build/author.json.
    """

    def body() -> None:
        _require_builder()
        from harness.suite_builder.author import AuthorError, author_task  # noqa: PLC0415

        task_dir = resolve_suites_home(suites_home) / suite_id / task
        if not (task_dir / "metadata.json").is_file():
            raise SuiteCommandError(
                "not_found", f"no task {task!r} in suite {suite_id}", EXIT_MISSING
            )
        try:
            doc, code = author_task(
                task_dir=task_dir,
                harness=harness,
                model=model,
                billing=billing,
                effort=effort,
                timeout_s=timeout,
                max_turns=max_turns,
            )
        except AuthorError as exc:
            raise SuiteCommandError(exc.code, exc.message, exc.exit_code) from exc
        lines = [
            f"{doc['status']} {task}: {len(doc['changed']['accepted'])} accepted, "
            f"{len(doc['changed']['rejected'])} discarded change(s)"
        ]
        lines += [f"  warning {w}" for w in doc["warnings"]]
        _finish(json_output, doc, lines, code)

    _guarded(json_output, body)


# --- wiring ------------------------------------------------------------------


def _json_flag(p: argparse.ArgumentParser) -> None:
    p.add_argument(
        "--json", dest="json_output", action="store_true", help="One JSON document on stdout"
    )


def register_builder_commands(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> None:
    """Add the builder's commands to ``vulcanbench suite``."""
    p = add_command(subparsers, "access", suite_access)
    _json_flag(p)
    for name, handler in (("unlock", suite_unlock), ("password-hash", suite_password_hash)):
        p = add_command(subparsers, name, handler)
        p.add_argument(
            "--password-stdin",
            dest="password_stdin",
            action="store_true",
            help="Read the password from one line on stdin instead of a prompt",
        )
        _json_flag(p)
    p = add_command(subparsers, "lock", suite_lock)
    _json_flag(p)

    prs = add_group(subparsers, "prs", "Merged pull requests as task candidates")
    p = add_command(prs, "list", suite_prs_list)
    p.add_argument("--repo", dest="repo", default=None, help="owner/name")
    p.add_argument(
        "--suite", dest="suite_id", default=None, help="Take the repository from this suite"
    )
    p.add_argument("--since", dest="since", default=None, help="Merged on or after YYYY-MM-DD")
    p.add_argument(
        "--limit", dest="limit", type=int, default=50, help="At most this many (1 to 200)"
    )
    p.add_argument("--min-loc", dest="min_loc", type=int, default=10, help="Smallest source change")
    p.add_argument("--max-loc", dest="max_loc", type=int, default=400, help="Largest source change")
    p.add_argument("--max-files", dest="max_files", type=int, default=20, help="Most files changed")
    _common(p)

    task = add_group(subparsers, "task", "Build one task: scaffold, then author")
    p = add_command(task, "scaffold", suite_task_scaffold)
    p.add_argument("suite_id", metavar="id")
    p.add_argument("--pr", dest="pr", type=int, required=True, help="Merged pull request number")
    p.add_argument("--repo", dest="repo", default=None, help="owner/name (default: the suite's)")
    p.add_argument("--task-id", dest="task_id", default=None, help="Default pr-<n>-<title slug>")
    p.add_argument("--paths", dest="paths", default=None, help="Comma list: keep only these paths")
    p.add_argument("--exclude", dest="exclude", default=None, help="Comma list of globs to drop")
    p.add_argument(
        "--strip-tests",
        dest="strip_tests",
        default="pr",
        choices=("pr", "all", "none"),
        help="Remove the PR's test files (default), every test file, or none",
    )
    p.add_argument(
        "--snapshot", dest="snapshot", action="store_true", help="Pack repo/ as a tarball"
    )
    p.add_argument("--license-note", dest="license_note", default=None, help="Text for repo/NOTICE")
    p.add_argument(
        "--clone-dir", dest="clone_dir", type=Path, default=None, help="Use this local clone"
    )
    _common(p)

    p = add_command(task, "author", suite_task_author)
    p.add_argument("suite_id", metavar="id")
    p.add_argument("--task", dest="task", required=True, help="Task directory name")
    p.add_argument("--harness", dest="harness", required=True, help="claude-code, codex, ...")
    p.add_argument("--model", dest="model", required=True, help="Model id for that harness")
    p.add_argument("--billing", dest="billing", default="auto", help="auto|subscription")
    p.add_argument("--effort", dest="effort", default=None, help="Effort level")
    p.add_argument("--timeout", dest="timeout", type=int, default=3600, help="Session cap, seconds")
    p.add_argument("--max-turns", dest="max_turns", type=int, default=200, help="Turn cap")
    _common(p)
