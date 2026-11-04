"""``suite task author``: one model session writes a scaffolded task's issue, tests and metadata.

The session runs through the same CLI harness adapters as benchmark runs
(claude-code, codex and the others; subscription billing), on this machine,
in a scratch copy of the task directory with network off. Afterwards only an
allowlist of changes is copied back: ``issue.md``, ``tests/``,
``gold_patch.diff``, ``build/author_notes.md``, deletions of test files under
``repo/``, and these ``metadata.json`` keys: :data:`ALLOWED_METADATA_KEYS`.
Every other change is discarded and listed under ``changed.rejected``.

It spends model quota, so the GUI starts it only behind a confirm click; it
never re-runs itself and never validates (``suite validate`` does that).
"""

from __future__ import annotations

import hashlib
import json
import shutil
import tempfile
import time
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from harness.suite_builder.gh import is_test_path
from harness.suite_builder.prompt import build_author_prompt
from harness.suite_builder.scaffold import DRAFT_MARKER, canary_guid

AUTHOR_SCHEMA = "vulcanbench.suite.author.v1"
ALLOWED_METADATA_KEYS = (
    "tests",
    "image",
    "setup",
    "setup_timeout_s",
    "test_timeout_s",
    "agent_hints",
    "category",
    "difficulty",
    "task_complexity",
    "decontamination_notes",
)
_ACCEPTED_FILES = ("issue.md", "gold_patch.diff", "build/author_notes.md")

EXIT_OK = 0
EXIT_PROVIDER = 1
EXIT_REFUSED = 2
EXIT_NOT_READY = 3
EXIT_NO_CHANGE = 6


class AuthorError(Exception):
    """A refusal before the session starts (no quota spent)."""

    def __init__(self, code: str, message: str, exit_code: int) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.exit_code = exit_code


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _snapshot(root: Path) -> dict[str, str]:
    return {
        p.relative_to(root).as_posix(): _digest(p)
        for p in root.rglob("*")
        if p.is_file() and not p.is_symlink() and ".git" not in p.relative_to(root).parts
    }


def _read_json(path: Path) -> dict[str, Any] | None:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def sync_back(  # noqa: PLR0912, one pass over the changed paths
    task_dir: Path, scratch: Path, before: dict[str, str]
) -> dict[str, list[str]]:
    """Copy allowed changes from ``scratch`` into ``task_dir``; return accepted and rejected paths."""
    after = _snapshot(scratch)
    changed = sorted(
        {rel for rel in before.keys() | after.keys() if before.get(rel) != after.get(rel)}
    )
    accepted: list[str] = []
    rejected: list[str] = []
    for rel in changed:
        if rel == "metadata.json":
            continue
        deleted = rel not in after
        ok = (
            rel in _ACCEPTED_FILES
            or rel.startswith("tests/")
            or (rel.startswith("repo/") and deleted and is_test_path(rel[len("repo/") :]))
        )
        if not ok:
            rejected.append(rel)
            continue
        target = task_dir / rel
        if deleted:
            target.unlink(missing_ok=True)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(scratch / rel, target)
        accepted.append(rel)
    if "metadata.json" in changed:
        original = _read_json(task_dir / "metadata.json") or {}
        edited = _read_json(scratch / "metadata.json")
        if edited is None:
            rejected.append("metadata.json")
        else:
            merged = dict(original)
            took = []
            for key in ALLOWED_METADATA_KEYS:
                if key in edited and edited[key] != original.get(key):
                    merged[key] = edited[key]
                    took.append(key)
                elif key not in edited and key in original and key != "tests":
                    merged.pop(key)
                    took.append(key)
            for key in sorted(set(edited) | set(original)):
                if key not in ALLOWED_METADATA_KEYS and edited.get(key) != original.get(key):
                    rejected.append(f"metadata.json:{key}")
            if took:
                (task_dir / "metadata.json").write_text(
                    json.dumps(merged, indent=2) + "\n", encoding="utf-8"
                )
                accepted.extend(f"metadata.json:{k}" for k in took)
    return {"accepted": accepted, "rejected": rejected}


def task_warnings(task_dir: Path, canary: str) -> list[str]:
    """What a reviewer should look at after authoring (not failures: validate decides those)."""
    warnings: list[str] = []
    issue = task_dir / "issue.md"
    if issue.is_file() and DRAFT_MARKER in issue.read_text(encoding="utf-8", errors="replace"):
        warnings.append("draft_issue: issue.md still carries the DRAFT line")
    meta = _read_json(task_dir / "metadata.json") or {}
    tests = meta.get("tests") if isinstance(meta.get("tests"), dict) else {}
    if not (tests or {}).get("fail_to_pass"):
        warnings.append("no_fail_to_pass: metadata.json tests.fail_to_pass is empty")
    tests_dir = task_dir / "tests"
    files = [p for p in tests_dir.rglob("*") if p.is_file()] if tests_dir.is_dir() else []
    if not files:
        warnings.append("no_hidden_tests: tests/ is empty")
    for f in files:
        try:
            text = f.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        if canary not in text:
            warnings.append(f"no_canary: {f.relative_to(task_dir).as_posix()}")
    gold = task_dir / "gold_patch.diff"
    if gold.is_file():
        for line in gold.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith("diff --git a/"):
                path = line.split(" b/", 1)[-1]
                if is_test_path(path):
                    warnings.append(f"gold_touches_tests: {path}")
    return warnings


def _resolve_spec(model: str, harness: str, billing: str) -> str:
    from harness.cli import _execution_spec  # noqa: PLC0415, harness.cli imports this package

    return _execution_spec(model, harness, billing)


def author_task(  # noqa: PLR0912, PLR0915, preconditions then one session then bookkeeping
    *,
    task_dir: Path,
    harness: str,
    model: str,
    billing: str = "auto",
    effort: str | None = None,
    timeout_s: int = 3600,
    max_turns: int = 200,
) -> tuple[dict[str, Any], int]:
    """Run one authoring session on ``task_dir``; returns ``(author.json document, exit code)``."""
    from harness.agent.cli_agents import get_cli_agent_adapter  # noqa: PLC0415
    from harness.agent.providers import ProviderError  # noqa: PLC0415
    from harness.effort import effort_config  # noqa: PLC0415
    from harness.settings import BlockedEffortError, check_effort_allowed  # noqa: PLC0415
    from harness.tracer.collector import TraceCollector  # noqa: PLC0415

    build = task_dir / "build"
    scaffold = _read_json(build / "scaffold.json")
    metadata = _read_json(task_dir / "metadata.json")
    if scaffold is None or metadata is None:
        raise AuthorError(
            "not_scaffolded",
            f"{task_dir} has no build/scaffold.json; scaffold it first",
            EXIT_REFUSED,
        )
    if (task_dir.parent / "suite.lock.json").is_file():
        raise AuthorError("suite_frozen", f"{task_dir.parent.name} is frozen", EXIT_REFUSED)
    if harness.strip().lower() == "vulcan":
        raise AuthorError(
            "unsupported_harness",
            "authoring runs through a CLI harness (claude-code, codex, ...), not the API loop",
            EXIT_REFUSED,
        )
    if not 60 <= timeout_s <= 4 * 3600:
        raise AuthorError("invalid_option", "--timeout must be 60 to 14400 seconds", EXIT_REFUSED)
    if not 1 <= max_turns <= 1000:
        raise AuthorError("invalid_option", "--max-turns must be 1 to 1000", EXIT_REFUSED)
    try:
        check_effort_allowed(effort)
        spec = _resolve_spec(model, harness, billing)
    except (BlockedEffortError, ValueError) as exc:
        raise AuthorError("invalid_option", str(exc), EXIT_REFUSED) from exc
    adapter = get_cli_agent_adapter(spec)
    effort_meta = effort_config(adapter.harness_id, effort) if effort else None
    preflight = adapter.preflight()
    if not preflight.ready:
        raise AuthorError(
            "harness_not_ready",
            f"{adapter.harness_id} is not ready: {preflight.detail or 'not logged in with a subscription'}",
            EXIT_NOT_READY,
        )

    canary = canary_guid()
    task_id = task_dir.name
    run_id = f"author-{uuid.uuid4().hex[:8]}"
    run_dir = build / "author-run"
    if run_dir.exists():
        shutil.rmtree(run_dir)
    run_dir.mkdir(parents=True)
    stream_log = run_dir / "cli-agent-stream.jsonl"
    collector = TraceCollector(run_dir, run_id, task_id, spec)
    started = datetime.now(UTC)
    t0 = time.monotonic()
    root = Path(tempfile.mkdtemp(prefix="vulcanbench-author-"))
    scratch = root / task_id
    shutil.copytree(
        task_dir, scratch, symlinks=True, ignore=shutil.ignore_patterns("author-run", "author.json")
    )
    before = _snapshot(scratch)
    prompt = build_author_prompt(metadata, scaffold, canary)
    outcome: Any = None
    error: str | None = None
    try:
        outcome = adapter.run_task(
            workspace=scratch,
            prompt=prompt,
            model=spec.partition(":")[2] or model,
            priced_spec=spec,
            max_turns=max_turns,
            collector=collector,
            stream_log_path=stream_log,
            timeout_s=float(timeout_s),
            network=False,
            max_run_cost=None,
            effort=effort_meta.provider_value if effort_meta and effort_meta.supported else None,
            agent_container=None,
        )
    except ProviderError as exc:
        from harness.redaction import redact  # noqa: PLC0415

        error = redact(str(exc))[:600]
    try:
        changed = sync_back(task_dir, scratch, before)
    finally:
        shutil.rmtree(root, ignore_errors=True)
    if changed["accepted"]:
        meta = _read_json(task_dir / "metadata.json") or {}
        builder = dict(meta.get("builder") or {})
        builder["status"] = "authored"
        meta["builder"] = builder
        (task_dir / "metadata.json").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    warnings = task_warnings(task_dir, canary)
    if outcome is not None and outcome.timed_out:
        warnings.append(f"timed_out: the session hit the {timeout_s} s cap")
    if changed["rejected"]:
        warnings.append(f"discarded {len(changed['rejected'])} change(s) outside the allowed paths")
    if error is not None:
        status, code = "failed", EXIT_PROVIDER
    elif changed["accepted"]:
        status, code = "ok", EXIT_OK
    else:
        status, code = "failed", EXIT_NO_CHANGE
    finished = datetime.now(UTC)
    doc: dict[str, Any] = {
        "schema": AUTHOR_SCHEMA,
        "task_id": task_id,
        "harness": adapter.harness_id,
        "model": model,
        "model_spec": spec,
        "billing": billing,
        "effort": effort,
        "started_at": started.isoformat(timespec="seconds"),
        "finished_at": finished.isoformat(timespec="seconds"),
        "duration_s": round(time.monotonic() - t0, 1),
        "exit": {
            "finished": bool(outcome.finished) if outcome is not None else False,
            "timed_out": bool(outcome.timed_out) if outcome is not None else False,
            "num_turns": outcome.num_turns if outcome is not None else None,
            "error": error,
        },
        "harness_version": getattr(outcome, "harness_version", None) or preflight.version,
        "tokens": {
            "prompt": getattr(outcome, "prompt_tokens", 0),
            "completion": getattr(outcome, "completion_tokens", 0),
        },
        "cost": {
            "cli_reported_usd": getattr(outcome, "cli_reported_cost_usd", None),
            "basis": getattr(outcome, "cost_basis", None),
        },
        "changed": changed,
        "warnings": warnings,
        "run_dir": str(run_dir),
        "stream_log": str(stream_log),
        "status": status,
    }
    (build / "author.json").write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    return doc, code
