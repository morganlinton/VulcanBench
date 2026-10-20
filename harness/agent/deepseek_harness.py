"""DeepSeek Harness (dsh) adapter: DeepSeek's own agent, API-metered, audited usage.

DeepSeek Harness (``@deepseek-ai/dsh``) is DeepSeek's open-source agent harness.
VulcanBench runs its one-shot ``headless`` profile against a pinned install,
inside the same macOS outer sandbox as Muse Code and Pi, with a per-run
``DSH_HOME`` so no saved setting, credential or prior session leaks in.

Run settings (owner decisions 2026-10-19, docs/DECISIONS.md):

- dsh's own sandbox and approvals are off (``danger-full-access``); the outer
  sandbox is the boundary, as for Muse Code, because sandbox-exec cannot nest.
- Web tools are off, as for every harness on the board.
- Session-log upload (``dsh_session_log``, default on) and OpenTelemetry
  feedback upload are off. The model never sees either, so behaviour is
  unchanged; task content still reaches DeepSeek in the model requests.
- Session logs are written uncompressed so the receipts are auditable.
- OpenTelemetry upload is off through ``DSH_TELEMETRY_MODE=DISABLED``, the
  variable the shipped config reads, so that entry's config stays as shipped.

Token use is read from dsh's persisted ``assistant/message`` events, each of
which carries the call's usage and time; cost is metered per call with
DeepSeek's peak multiplier (``pricing.cost_usd_at``).

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
from contextlib import suppress
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from harness.agent.cli_agents import (
    CliAgentOutcome,
    HarnessCapabilities,
    HarnessPreflight,
    _subscription_env,
)
from harness.agent.muse_code import boundary_profile
from harness.agent.providers import ProviderError
from harness.pricing import cost_usd, cost_usd_at
from harness.redaction import sanitize

VERSION = "0.2.1-rc.2"
# VulcanBench model spec -> DeepSeek API model id (identical today).
MODELS = {"deepseek-flash": "deepseek-flash", "deepseek-v4-pro": "deepseek-v4-pro"}
PROVIDER = "deepseek-official"
# dsh's DeepSeek route accepts off, low, high and max.
EFFORTS = frozenset({"low", "high", "max"})
API_KEY_ENV = "DEEPSEEK_API_KEY"
# Failures that are the account or the API, not the model's work.
INFRA_CODES = ("AUTH", "QUOTA", "RATE_LIMIT", "SERVER", "TRANSPORT", "MISSING_CREDENTIAL")


def benchmark_patch(model: str, effort: str) -> str:
    """The cordis patch overlay that fixes model, effort and the data settings."""
    return f"""# VulcanBench overlay for one dsh headless run.
- id: agent-default-model
  config:
    provider: {PROVIDER}
    model: {model}
- id: llm-deepseek
  config:
    apiKeyEnv: {API_KEY_ENV}
    reasoningEffort: {effort}
- id: session-log-deepseek
  config:
    enabled: false
- id: tool-web
  disabled: true
# A patch row replaces the entry's whole config, so the shipped root is restated.
- id: session-persistence-jsonl
  config:
    root: !!js dshHomePath('sessions')
    compression: none
"""


def tree_sha256(root: Path) -> str:
    """Content hash of every file under ``root``, by sorted relative path."""
    digest = hashlib.sha256()
    for path in sorted(p for p in root.rglob("*") if p.is_file() and not p.is_symlink()):
        rel = path.relative_to(root).as_posix().encode()
        with path.open("rb") as source:
            digest.update(rel + b"\0" + hashlib.file_digest(source, "sha256").digest())
    return digest.hexdigest()


def pinned_install() -> tuple[Path, Path, str]:
    """Node executable, the dsh entry point and the verified tree hash of the install."""
    node = Path(os.environ.get("VULCANBENCH_DSH_NODE", ""))
    prefix = Path(os.environ.get("VULCANBENCH_DSH_PREFIX", ""))
    expected = os.environ.get("VULCANBENCH_DSH_SHA256", "")
    if (
        not str(node)
        or not node.is_absolute()
        or not node.is_file()
        or not os.access(node, os.X_OK)
    ):
        raise ProviderError("Set VULCANBENCH_DSH_NODE to an absolute, executable node binary")
    if not str(prefix) or not prefix.is_absolute() or not prefix.is_dir():
        raise ProviderError("Set VULCANBENCH_DSH_PREFIX to the absolute dsh install prefix")
    package = prefix / "node_modules/@deepseek-ai/dsh"
    try:
        version = json.loads((package / "package.json").read_text())["version"]
    except (OSError, KeyError, json.JSONDecodeError) as exc:
        raise ProviderError("No dsh package under VULCANBENCH_DSH_PREFIX") from exc
    if version != VERSION:
        raise ProviderError(f"dsh {version} installed, adapter pins {VERSION}")
    actual = tree_sha256(prefix / "node_modules")
    if len(expected) != 64 or actual != expected:
        raise ProviderError("dsh install tree SHA256 mismatch")
    return node.resolve(), (package / "lib/bin.js").resolve(), actual


def sandbox_context(scratch: Path) -> str:
    return (
        "Execution environment: an outer OS sandbox is active. Write temporary files inside "
        f"the workspace or $TMPDIR ({scratch / 'tmp'}). Global /tmp and /var/tmp are not "
        "available. Other runs' temporary files and the benchmark checkout are inaccessible. "
        'For mktemp, supply an explicit template such as "$TMPDIR/probe.XXXXXX".\n\n'
    )


def _zero() -> dict[str, Any]:
    return dict(
        input=0,
        output=0,
        cacheRead=0,
        cacheWrite=0,
        reasoning=0,
        calls=0,
        retried_attempts=0,
        attempts_without_usage=0,
        unmetered_title_calls=0,
        peak_calls=0,
        metered_usd=0.0,
    )


def _stream_usage(data: dict[str, Any]) -> Any:
    """The last ``usage`` chunk of a settled stream, as dsh's own JSON projection reads it."""
    for record in reversed(data.get("stream") or []):
        chunk = record.get("chunk") if isinstance(record, dict) else None
        if (
            record.get("type") == "chunk"
            and isinstance(chunk, dict)
            and chunk.get("type") == "usage"
        ):
            return chunk.get("usage")
    return None


def collect_usage(paths: list[Path], model: str, priced_spec: str) -> dict[str, Any]:  # noqa: PLR0912, one branch per receipt guard
    """Sum billed usage across dsh's session logs, subagents included.

    A committed ``assistant/message`` carries its call's usage (or, when
    interrupted, its stream's last usage chunk) and must come from
    ``PROVIDER``/``model``. A retried ``assistant/attempt`` keeps its usage only
    in its stream; it is billed, so it counts. An attempt that reports no usage
    is counted under ``attempts_without_usage``, and dsh's session-title call,
    which records no usage, under ``unmetered_title_calls``. ``input`` excludes cache reads,
    which DeepSeek reports separately (Messages protocol semantics).
    """
    totals = _zero()
    seen: set[tuple[str, int]] = set()
    for path in sorted(paths):
        with path.open() as source:
            for line in source:
                row = json.loads(line)
                kind = row.get("type")
                if kind == "session/title-llm-request":
                    # dsh titles each session with one small call (input capped at
                    # 4 KiB) and records no usage for it; counted, disclosed, unpriced.
                    route = (row.get("data") or {}).get("route") or {}
                    if route.get("provider") != PROVIDER or route.get("model") != model:
                        raise ProviderError(f"dsh title call on an unexpected route: {route!r}")
                    totals["unmetered_title_calls"] += 1
                    continue
                if kind not in {"assistant/message", "assistant/attempt"}:
                    continue
                key = (str(path), row.get("seq"))
                if not isinstance(row.get("seq"), int) or key in seen:
                    raise ProviderError("dsh assistant event without a unique sequence number")
                seen.add(key)
                data = row.get("data") or {}
                if kind == "assistant/message":
                    origin = (data.get("message") or {}).get("source") or {}
                    if origin.get("provider") != PROVIDER or origin.get("model") != model:
                        raise ProviderError(
                            f"dsh model mismatch: {origin.get('provider')!r}/{origin.get('model')!r}"
                        )
                usage = data.get("usage") if kind == "assistant/message" else None
                if usage is None:
                    usage = _stream_usage(data)
                if usage is None:
                    if kind == "assistant/message" and not data.get("interrupted"):
                        raise ProviderError("dsh assistant message has no usage receipt")
                    totals["attempts_without_usage"] += 1
                    continue
                if not isinstance(usage, dict):
                    raise ProviderError("dsh usage receipt is not an object")
                receipt = {}
                for wire, name in (
                    ("inputTokens", "input"),
                    ("outputTokens", "output"),
                    ("cacheReadTokens", "cacheRead"),
                    ("cacheWriteTokens", "cacheWrite"),
                    ("reasoningTokens", "reasoning"),
                ):
                    value = usage.get(wire, 0)
                    if type(value) is not int or value < 0:
                        raise ProviderError("Invalid dsh token count")
                    receipt[name] = value
                at = datetime.fromtimestamp(row["time"] / 1000, UTC)
                prompt = receipt["input"] + receipt["cacheRead"] + receipt["cacheWrite"]
                cached = receipt["cacheRead"]
                spent = cost_usd_at(
                    priced_spec, prompt, receipt["output"], at, cached_input_tokens=cached
                )
                if spent is None:
                    raise ProviderError(f"No price for {priced_spec}")
                if spent != cost_usd(
                    priced_spec, prompt, receipt["output"], cached_input_tokens=cached
                ):
                    totals["peak_calls"] += 1
                totals["metered_usd"] += spent
                totals["calls" if kind == "assistant/message" else "retried_attempts"] += 1
                for name, value in receipt.items():
                    totals[name] += value
    totals["metered_usd"] = round(totals["metered_usd"], 6)
    return totals


class DeepSeekHarnessAdapter:
    harness_id = "dsh"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            self.harness_id,
            "DeepSeek Harness",
            "dsh",
            True,
            True,
            True,
            True,
            False,
            "macOS outer sandbox; isolated writes; repository read deny",
        )

    def preflight(self) -> HarnessPreflight:
        try:
            pinned_install()
        except ProviderError as exc:
            return HarnessPreflight(self.harness_id, False, None, False, None, detail=str(exc))
        has_key = bool(os.environ.get(API_KEY_ENV))
        return HarnessPreflight(
            self.harness_id,
            True,
            VERSION,
            has_key,
            "api-key" if has_key else None,
            detail="API key present; live authentication is checked by the run",
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
        del kwargs, max_turns
        if agent_container:
            raise ProviderError("dsh runs locally under the outer sandbox, not in a container")
        if network:
            raise ProviderError("dsh runs keep web tools off; network tasks are not supported")
        if max_run_cost is not None:
            raise ProviderError("dsh runs are bounded by wall clock, not a live cost cap")
        if model not in MODELS:
            raise ProviderError(f"dsh model {model!r} is not configured for VulcanBench")
        if effort not in EFFORTS:
            raise ProviderError("dsh requires an explicit low, high or max effort")
        if timeout_s is None or timeout_s <= 0:
            raise ProviderError("dsh requires a positive wall-clock timeout")
        key = os.environ.get(API_KEY_ENV)
        if not key:
            raise ProviderError(f"dsh needs {API_KEY_ENV}")
        node, entry, tree_sha = pinned_install()
        api_model = MODELS[model]
        workspace = Path(workspace).resolve()
        scratch = Path(tempfile.mkdtemp(prefix="vb-dsh-session-")).resolve()
        (scratch / "tmp").mkdir()
        home = scratch / "dsh-home"
        patch = benchmark_patch(api_model, effort)
        patch_path = scratch / "vulcanbench.patch.yml"
        patch_path.write_text(patch)
        env = _subscription_env()
        env.update(
            {
                API_KEY_ENV: key,
                "DSH_HOME": str(home),
                "DSH_PERMISSION_MODE": "danger-full-access",
                "DSH_TELEMETRY_MODE": "DISABLED",
                "TMPDIR": str(scratch / "tmp"),
                "TMP": str(scratch / "tmp"),
                "TEMP": str(scratch / "tmp"),
            }
        )
        profile = boundary_profile(workspace, scratch)
        profile_path = scratch / "boundary.sb"
        profile_path.write_text(profile)
        cmd = [
            "/usr/bin/sandbox-exec",
            "-f",
            str(profile_path),
            str(node),
            str(entry),
            "--profile",
            "headless",
            "--patch",
            str(patch_path),
            "--json",
            "-",
        ]
        collector.record(
            "cli_agent_start",
            {
                "harness": self.harness_id,
                "argv": cmd,
                "harness_version": VERSION,
                "install_tree_sha256": tree_sha,
                "node": str(node),
                "requested_model": model,
                "api_model": api_model,
                "requested_effort": effort,
                "billing": "api",
                "settings_patch": patch,
                "max_turns_note": "dsh has no step cap; the wall-clock timeout bounds the run",
            },
        )
        outcome = CliAgentOutcome(
            harness=self.harness_id,
            requested_model=model,
            session_id=None,
            harness_version=VERSION,
            auth_method="api-key",
            billing="api",
            cost_basis="metered-api-pricing",
            execution_boundary=(
                "macOS outer sandbox; dsh sandbox and approvals off; workspace/session writes; "
                "shared-temp/checkout read deny; explicit TMPDIR context; web tools off; "
                "session-log and telemetry upload off; shell network not isolated; API key in "
                "the agent environment"
            ),
        )
        proc = subprocess.Popen(
            cmd,
            cwd=workspace,
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
        )
        errors: list[str] = []
        assert proc.stdin is not None and proc.stdout is not None and proc.stderr is not None
        stdout, stderr = proc.stdout, proc.stderr
        # The task goes in on stdin so it never appears in the process table.
        proc.stdin.write(sandbox_context(scratch) + prompt)
        proc.stdin.close()

        def drain() -> None:
            for line in stderr:
                errors.append(line)

        reader = threading.Thread(target=drain, daemon=True)
        reader.start()

        def kill() -> None:
            with suppress(ProcessLookupError):
                os.killpg(proc.pid, signal.SIGKILL)

        def on_timeout() -> None:
            outcome.timed_out = True
            kill()

        timer = threading.Timer(timeout_s, on_timeout)
        timer.daemon = True
        timer.start()
        final: dict[str, Any] | None = None
        failure: dict[str, Any] | None = None
        try:
            with Path(stream_log_path or scratch / "stdout.jsonl").open("w") as log:
                for line in stdout:
                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    log.write(json.dumps(sanitize(event)) + "\n")
                    log.flush()
                    kind = event.get("type")
                    if kind == "session":
                        outcome.session_id = event.get("sessionId") or event.get("id")
                    elif kind == "text" and event.get("text"):
                        collector.record("assistant_text", {"text": event["text"]})
                    elif kind == "final":
                        final = event
                    elif kind == "error":
                        failure = event
            proc.wait()
        finally:
            timer.cancel()
            if proc.poll() is None:
                kill()
                proc.wait()
            reader.join(timeout=5)
        logs = sorted((home / "sessions").rglob("*.jsonl"))
        if stream_log_path:
            archive = Path(stream_log_path).parent / "dsh-session-logs"
            archive.mkdir(exist_ok=True)
            for path in logs:
                target = archive / path.relative_to(home / "sessions")
                target.parent.mkdir(parents=True, exist_ok=True)
                with path.open() as source, target.open("w") as destination:
                    for line in source:
                        destination.write(json.dumps(sanitize(json.loads(line))) + "\n")
            (archive / "stderr.txt").write_text(str(sanitize("".join(errors))))
            (archive / "boundary.sb").write_text(profile)
            (archive / "vulcanbench.patch.yml").write_text(patch)
        usage = collect_usage(logs, api_model, priced_spec)
        outcome.prompt_tokens = usage["input"] + usage["cacheRead"] + usage["cacheWrite"]
        outcome.completion_tokens = usage["output"]
        outcome.cached_input_tokens = usage["cacheRead"]
        outcome.reasoning_output_tokens = usage["reasoning"]
        outcome.num_turns = usage["calls"]
        outcome.reported_model = model if usage["calls"] else None
        outcome.model_identity_confidence = (
            "harness-recorded" if usage["calls"] else "requested-only"
        )
        outcome.cli_reported_cost_usd = usage["metered_usd"] if usage["calls"] else None
        outcome.finished = bool(
            final is not None and proc.returncode == 0 and not outcome.timed_out
        )
        outcome.subtype = "completed" if outcome.finished else f"exit-{proc.returncode}"
        collector.record("dsh_usage", usage)
        inactive = "did not activate" in "".join(errors)
        if inactive or (outcome.finished and not usage["calls"]):
            # A plugin that failed to load (e.g. session persistence) silently drops
            # receipts, so the run cannot be audited and must not be scored.
            raise ProviderError(
                "dsh run has no auditable usage: "
                + ("a plugin did not activate" if inactive else "no session-log receipts")
            )
        if not (outcome.timed_out or outcome.finished):
            detail = str(failure or "".join(errors)[-2000:])
            infra = any(f"{code}:" in detail or f'"{code}"' in detail for code in INFRA_CODES)
            raise ProviderError(
                f"dsh {'API failure' if infra else 'failed'}, exit={proc.returncode}: {detail[:500]}"
            )
        collector.record("cli_agent_result", outcome.summary())
        return outcome
