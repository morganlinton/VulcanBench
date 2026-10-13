"""Muse Code subscription adapter with isolated sessions and audited usage.

Imported lazily by cli_agents to avoid circular adapter registration.
"""

from __future__ import annotations

import hashlib
import json
import os
import signal
import subprocess
import tempfile
import threading
import uuid
from collections.abc import Iterator
from contextlib import suppress
from pathlib import Path
from typing import Any

from harness.agent.cli_agents import (
    CliAgentOutcome,
    HarnessCapabilities,
    HarnessPreflight,
    SubscriptionQuotaError,
    _subscription_env,
    _version,
)
from harness.agent.providers import ProviderError
from harness.redaction import sanitize

# VulcanBench model spec -> (Muse Code --model value, --contributor-mode value).
# Spark 1.3 selects Contributor through its model id and predates the flag, so
# it passes no flag. Wonder has one id per model; Muse Code 1.4.6 selects the
# data mode per run with --contributor-mode, which the harness always passes
# explicitly so the CLI default never decides it. Meta serves Wonder Contributor
# only through Muse Code, not the Model API (dev.meta.ai pricing, 2026-10-12).
CLI_MODELS: dict[str, tuple[str, str | None]] = {
    "muse-spark-1.3": ("muse-spark-1.3", None),
    "muse-spark-1.3-contributor": ("muse-spark-1.3-contributor", None),
    "muse-wonder-1.0": ("muse-wonder-1.0", "off"),
    "muse-wonder-1.0-contributor": ("muse-wonder-1.0", "on"),
    "muse-wonder-full-1.0": ("muse-wonder-full-1.0", "off"),
    "muse-wonder-full-1.0-contributor": ("muse-wonder-full-1.0", "on"),
    # Billed to a Meta Model API key instead of the subscription. The API serves
    # Wonder Full not at all and Wonder Contributor not at all, so only Standard
    # Wonder 1.0 has an API-billed spec.
    "muse-wonder-1.0-api": ("muse-wonder-1.0", "off"),
}
MODELS = set(CLI_MODELS)
API_BILLED = {"muse-wonder-1.0-api"}
# Environment variables that may hold the Model API key, in lookup order. Muse
# Code reads META_API_KEY, which takes priority over the account login.
API_KEY_ENVS = ("MUSE_API_KEY", "META_MUSE_SPARK_API", "MODEL_API_KEY")
EFFORTS = {"minimal", "low", "medium", "high", "xhigh", "max", "ultra"}
# Muse Code aborts a run when the provider stream is silent this long. The
# 180 s default killed five lodgecore attempts on 2026-09-08 during Meta
# Contributor stalls; raised by protocol amendment. Scored results are
# unaffected because only completed runs are graded.
STREAM_IDLE_TIMEOUT_SECS = 900


def pinned_executable() -> tuple[Path, str]:
    """Require a content pin, never invoke Muse's auto-updating launcher."""
    name = os.environ.get("VULCANBENCH_MUSE_BINARY", "")
    expected = os.environ.get("VULCANBENCH_MUSE_SHA256", "")
    path = Path(name)
    if not name or not path.is_absolute() or path.is_symlink() or not path.is_file():
        raise ProviderError("Set VULCANBENCH_MUSE_BINARY to an absolute, non-symlink Muse binary")
    with path.open("rb") as source:
        magic = source.read(4)
        source.seek(0)
        actual = hashlib.file_digest(source, "sha256").hexdigest()
    if magic[:2] == b"#!":
        raise ProviderError("Muse pin must target the binary, not a launcher script")
    if len(expected) != 64 or actual != expected or not os.access(path, os.X_OK):
        raise ProviderError("Muse binary SHA256 mismatch or missing executable permission")
    return path.resolve(), actual


def boundary_profile(workspace: Path, scratch: Path) -> str:
    """Permit own scratch, but deny stale artifacts in shared temp trees."""
    workspace, scratch = workspace.resolve(), scratch.resolve()
    blocked = [
        Path(__file__).resolve().parents[2],
        Path.home() / ".codex",
        Path.home() / ".claude",
        Path.home() / ".local/share/muse/sessions",
    ]
    if any(
        workspace.is_relative_to(p.resolve()) or scratch.is_relative_to(p.resolve())
        for p in blocked
    ):
        raise ProviderError("Muse workspace and scratch must be outside protected directories")
    profile = "(version 1)\n(allow default)\n(deny file-write*)\n" + "".join(
        f"(allow file-write* (subpath {json.dumps(str(p.resolve()))}))\n"
        for p in [workspace, scratch, Path.home() / ".config/muse", Path.home() / ".cache/muse"]
    )
    profile += '(allow file-write* (literal "/dev/null") (literal "/dev/tty"))\n'
    profile += "".join(
        f"(deny file-read* file-write* (subpath {json.dumps(str(p.resolve()))}))\n" for p in blocked
    )
    for parent in {
        Path("/private/tmp"),
        Path("/private/var/tmp"),
        Path(tempfile.gettempdir()).resolve(),
    }:
        profile += (
            f"(deny file-read-data file-write* (require-all (subpath {json.dumps(str(parent))}) "
            f"(require-not (subpath {json.dumps(str(workspace))})) "
            f"(require-not (subpath {json.dumps(str(scratch))}))))\n"
        )
    return profile


def sandbox_context(scratch: Path) -> str:
    return (
        "Execution environment: an outer OS sandbox is active even though Muse's inner "
        "sandbox is disabled. Write temporary files inside the workspace or $TMPDIR "
        f"({scratch / 'tmp'}). Global /tmp and /var/tmp are not available. "
        "Do not use hard-coded global temporary paths. Other runs' temporary files, "
        "the benchmark checkout, and prior agent sessions are inaccessible. "
        'For mktemp, supply an explicit template such as "$TMPDIR/probe.XXXXXX".\n\n'
    )


def session_records(row: dict[str, Any]) -> Iterator[dict[str, Any]]:
    """Unpack retained records as well as ordinary session envelopes."""
    yield row
    frame = row.get("retained_frame") or {}
    if isinstance(frame, str):
        frame = row
    for child in frame.get("children", []):
        record = child.get("record_json")
        if record:
            yield from session_records(json.loads(record) if isinstance(record, str) else record)


def collect_usage(paths: list[Path], model: str) -> dict[str, int]:
    """Count unique completed model calls, including reminder/subagent calls.

    Session logs mirror run records. Never add goal_usage_attribution to the
    model_completed totals, since those are two views of the same request.
    """
    totals = dict(input_tokens=0, output_tokens=0, cached_tokens=0, reasoning_tokens=0, calls=0)
    seen: dict[tuple[str, str], dict[str, int]] = {}
    for path in sorted(paths):
        with path.open() as source:
            rows = (row for line in source for row in session_records(json.loads(line)))
            for row in rows:
                _add_usage(row, model, seen, totals)
    return totals


def _add_usage(
    row: dict[str, Any],
    model: str,
    seen: dict[tuple[str, str], dict[str, int]],
    totals: dict[str, int],
) -> None:
    payload = row.get("payload") or {}
    event = payload.get("event", {})
    if event.get("kind") != "model_completed":
        return
    # Record IDs are only unique within a run, not across subagents.
    record_id = payload.get("source_run_record_id") or row.get("id")
    if record_id is None:
        raise ProviderError("Muse completed call has no record identity")
    if not payload.get("run_id"):
        raise ProviderError(
            "Muse completed call has no run identity; refusing ambiguous accounting"
        )
    key = (payload["run_id"], str(record_id))
    if event.get("model") != model:
        raise ProviderError(f"Muse model mismatch: {event.get('model')!r}")
    usage = event.get("usage")
    if not isinstance(usage, dict) or not {"input_tokens", "output_tokens"} <= usage.keys():
        raise ProviderError("Muse completed call has no token receipt")
    receipt: dict[str, int] = {}
    for name in ("input_tokens", "output_tokens", "cached_tokens", "reasoning_tokens"):
        value = usage.get(name, 0)
        if type(value) is not int or value < 0:
            raise ProviderError("Invalid Muse token count")
        receipt[name] = value
    if (
        receipt["cached_tokens"] > receipt["input_tokens"]
        or receipt["reasoning_tokens"] > receipt["output_tokens"]
    ):
        raise ProviderError("Inconsistent Muse token subsets")
    if key in seen:
        if seen[key] != receipt:
            raise ProviderError("Conflicting Muse token receipts for one run record")
        return
    seen[key] = receipt
    totals["calls"] += 1
    for name, value in receipt.items():
        totals[name] += value


class MuseCodeAdapter:
    harness_id = "muse-code"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            self.harness_id,
            "Muse Code",
            "muse",
            True,
            True,
            True,
            True,
            False,
            "macOS outer sandbox; isolated writes; repository read deny",
        )

    def preflight(self) -> HarnessPreflight:
        try:
            executable, _ = pinned_executable()
        except ProviderError as exc:
            return HarnessPreflight(self.harness_id, False, None, False, None, detail=str(exc))
        version = _version(str(executable))
        auth = (
            Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))) / "muse/auth.json"
        )
        return HarnessPreflight(
            self.harness_id,
            bool(version),
            version,
            auth.is_file(),
            "subscription" if auth.is_file() else None,
            detail="Account credential present; live authentication is checked by the run",
        )

    def run_task(self, **kwargs: Any) -> CliAgentOutcome:
        return self._run(**kwargs)

    def _run(  # noqa: PLR0912, PLR0915 - one process lifecycle and receipt archive
        self,
        *,
        workspace: str | Path,
        prompt: str,
        model: str,
        priced_spec: Any,
        max_turns: int,
        collector: Any,
        stream_log_path: str | Path | None = None,
        timeout_s: float | None = None,
        network: bool = False,
        max_run_cost: float | None = None,
        effort: str | None = None,
        agent_container: Any = None,
        **kwargs: Any,
    ) -> CliAgentOutcome:
        del priced_spec, kwargs
        if agent_container or max_run_cost is not None:
            raise ProviderError(
                "Muse requires local execution and a wall-clock budget, not a live cost cap"
            )
        if model not in MODELS or effort not in EFFORTS:
            raise ProviderError("Muse requires a verified explicit model and effort")
        if model.endswith("-contributor") and effort == "max":
            # Documented for Spark 1.3 Contributor; Meta does not document max
            # under Contributor for Wonder either, so it stays excluded.
            raise ProviderError("Meta documents max reasoning as Standard-only, not Contributor")
        cli_model, contributor_mode = CLI_MODELS[model]
        api_key = None
        if model in API_BILLED:
            api_key = next((os.environ[k] for k in API_KEY_ENVS if os.environ.get(k)), None)
            if not api_key:
                raise ProviderError(f"API-billed Muse needs one of {', '.join(API_KEY_ENVS)}")
        if timeout_s is None or timeout_s <= 0:
            raise ProviderError("Muse requires a positive wall-clock timeout")
        checked = self.preflight()
        if not checked.ready:
            raise ProviderError(f"Muse preflight failed: {checked.detail}")
        executable, binary_sha256 = pinned_executable()
        workspace = Path(workspace).resolve()
        scratch = Path(tempfile.mkdtemp(prefix="vb-muse-session-")).resolve()
        (scratch / "tmp").mkdir()
        env = _subscription_env()
        env.update(
            XDG_DATA_HOME=str(scratch / "data"),
            MUSE_NO_AUTO_UPDATE="1",
            TMPDIR=str(scratch / "tmp"),
            TMP=str(scratch / "tmp"),
            TEMP=str(scratch / "tmp"),
            TBH_STREAM_IDLE_TIMEOUT_SECS=str(STREAM_IDLE_TIMEOUT_SECS),
        )
        if api_key:
            env["META_API_KEY"] = api_key
        session_id = str(uuid.uuid4())
        # Kernel denial covers all Muse tools, not just shell tools. The task
        # workspace is staged outside the checkout by VulcanBench.
        profile = boundary_profile(workspace, scratch)
        profile_path = scratch / "boundary.sb"
        profile_path.write_text(profile)
        cmd = [
            "/usr/bin/sandbox-exec",
            "-f",
            str(profile_path),
            str(executable),
            "exec",
            "--json",
            "--model",
            cli_model,
            *(("--contributor-mode", contributor_mode) if contributor_mode else ()),
            "--reasoning-effort",
            effort,
            "--workspace",
            str(workspace),
            "--session-id",
            session_id,
            "--disable-approval",
            "--disable-sandbox",
            "--no-foreign-personal-context",
            "--max-model-steps",
            str(max_turns),
        ]
        if not network:
            cmd.append("--disable-web-tools")
        cmd.append(sandbox_context(scratch) + prompt)
        collector.record(
            "cli_agent_start",
            {
                "harness": self.harness_id,
                "argv": [*cmd[:-1], "<task prompt>"],
                "harness_version": checked.version,
                "binary_sha256": binary_sha256,
                "requested_model": model,
                "cli_model": cli_model,
                "contributor_mode": contributor_mode,
                "pricing_tier": "contributor" if model.endswith("-contributor") else "standard",
                "billing": "api" if api_key else "subscription",
                "session_data_root": str(scratch / "data"),
                "requested_effort": effort,
                "stream_idle_timeout_secs": STREAM_IDLE_TIMEOUT_SECS,
            },
        )
        outcome = CliAgentOutcome(
            harness=self.harness_id,
            requested_model=model,
            session_id=session_id,
            harness_version=checked.version,
            auth_method="api-key" if api_key else "subscription",
            billing="api" if api_key else "subscription",
            cost_basis="metered-api-pricing" if api_key else "api-equivalent",
            execution_boundary="macOS outer sandbox; workspace/session writes; shared-temp/checkout/prior-session read deny; explicit TMPDIR context; web tools off unless network requested; shell network not isolated",
        )
        proc = subprocess.Popen(
            cmd,
            cwd=workspace,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
        )
        errors: list[str] = []
        assert proc.stdout is not None and proc.stderr is not None
        stdout, stderr = proc.stdout, proc.stderr

        def drain() -> None:
            for line in stderr:
                errors.append(line)

        reader = threading.Thread(target=drain, daemon=True)
        reader.start()

        def kill() -> None:
            outcome.timed_out = True
            with suppress(ProcessLookupError):
                os.killpg(proc.pid, signal.SIGKILL)

        timer = threading.Timer(timeout_s, kill)
        timer.daemon = True
        timer.start()
        terminal: dict[str, Any] | None = None
        try:
            with Path(stream_log_path or scratch / "stdout.jsonl").open("w") as log:
                for line in stdout:
                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    log.write(json.dumps(sanitize(event)) + "\n")
                    log.flush()
                    kind = event.get("payload_type", "")
                    if kind.startswith("run.terminal."):
                        terminal = event.get("payload", {})
                    if kind == "run.output.delta":
                        collector.record(
                            "assistant_text", {"text": event["payload"].get("text", "")}
                        )
            proc.wait()
        finally:
            timer.cancel()
            if proc.poll() is None:
                kill()
                proc.wait()
            reader.join(timeout=5)
        logs = sorted((scratch / "data/muse/sessions").rglob("session.jsonl"))
        if stream_log_path:
            archive = Path(stream_log_path).parent / "muse-session-logs"
            archive.mkdir(exist_ok=True)
            for path in logs:
                target = archive / path.relative_to(scratch / "data/muse/sessions")
                target.parent.mkdir(parents=True, exist_ok=True)
                with path.open() as source, target.open("w") as destination:
                    for line in source:
                        destination.write(json.dumps(sanitize(json.loads(line))) + "\n")
            (archive / "stderr.txt").write_text(str(sanitize("".join(errors))))
            (archive / "boundary.sb").write_text(profile)
        usage = collect_usage(logs, cli_model)
        outcome.prompt_tokens = usage["input_tokens"]
        outcome.completion_tokens = usage["output_tokens"]
        outcome.cached_input_tokens = usage["cached_tokens"]
        outcome.reasoning_output_tokens = usage["reasoning_tokens"]
        outcome.num_turns = usage["calls"]
        outcome.reported_model = model if usage["calls"] else None
        outcome.model_identity_confidence = (
            "provider-reported" if usage["calls"] else "requested-only"
        )
        outcome.finished = bool(
            terminal and terminal.get("terminal") == "completed" and proc.returncode == 0
        )
        outcome.subtype = terminal.get("terminal") if terminal else "no-terminal-event"
        collector.record("muse_usage", usage)
        if not outcome.timed_out and not outcome.finished:
            detail = str(terminal or "".join(errors)[-2000:])
            if any(
                s in detail.lower() for s in ("rate limit", "usage limit", "quota", "limit reached")
            ):
                raise SubscriptionQuotaError(f"Muse subscription limit: {detail[:500]}")
            raise ProviderError(f"Muse failed, exit={proc.returncode}: {detail[:500]}")
        if outcome.finished and not usage["calls"]:
            raise ProviderError("Muse completed without auditable usage")
        collector.record("cli_agent_result", outcome.summary())
        return outcome
