"""Pi coding agent adapter: API-metered runs with audited per-message usage.

Pi (``@earendil-works/pi-coding-agent``) is a small open-source terminal agent
that talks to any OpenAI Responses-compatible endpoint. VulcanBench runs it in
JSON mode against a pinned bundle and an operator-supplied provider config, so
the only moving parts are the model and Pi's own tool loop. Billing is the
provider API key, so runs are recorded as metered spend, not subscription use.

Imported lazily by cli_agents to avoid circular adapter registration.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import signal
import subprocess
import tempfile
import threading
import uuid
from contextlib import suppress
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
from harness.pricing import cost_usd
from harness.redaction import sanitize

# harness spec model ("provider/model") -> environment variables that may hold
# its API key. Pi reads the key from the variable named in models.json, which
# the adapter always exports as MUSE_API_KEY for Meta.
MODELS = {"meta/muse-wonder-1.0": ("MUSE_API_KEY", "META_MUSE_SPARK_API", "MODEL_API_KEY")}
CHILD_KEY_ENV = {"meta": "MUSE_API_KEY"}
DISABLED_RESOURCES = (
    "--no-session",
    "--no-context-files",
    "--no-extensions",
    "--no-skills",
    "--no-prompt-templates",
    "--offline",
)


def pinned_install() -> tuple[Path, Path, str, Path]:
    """Node executable, pinned Pi bundle, its SHA256 and the provider config dir."""
    node = Path(os.environ.get("VULCANBENCH_PI_NODE", ""))
    cli = Path(os.environ.get("VULCANBENCH_PI_CLI", ""))
    expected = os.environ.get("VULCANBENCH_PI_SHA256", "")
    config = Path(os.environ.get("VULCANBENCH_PI_CONFIG", ""))
    for label, path in (("VULCANBENCH_PI_NODE", node), ("VULCANBENCH_PI_CLI", cli)):
        if not str(path) or not path.is_absolute() or not path.is_file():
            raise ProviderError(f"Set {label} to an absolute file path")
    if not os.access(node, os.X_OK):
        raise ProviderError("VULCANBENCH_PI_NODE is not executable")
    actual = hashlib.sha256(cli.read_bytes()).hexdigest()
    if len(expected) != 64 or actual != expected:
        raise ProviderError("Pi bundle SHA256 mismatch")
    if not str(config) or not (config / "models.json").is_file():
        raise ProviderError("Set VULCANBENCH_PI_CONFIG to a directory holding models.json")
    return node.resolve(), cli.resolve(), actual, config.resolve()


def _version(node: Path, cli: Path) -> str | None:
    try:
        proc = subprocess.run(
            [str(node), str(cli), "--version"],
            capture_output=True,
            text=True,
            timeout=30,
            env={**_subscription_env(), "PI_OFFLINE": "1"},
            stdin=subprocess.DEVNULL,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    out = proc.stdout.strip()
    return f"Pi {out}" if proc.returncode == 0 and out else None


def sandbox_context(scratch: Path) -> str:
    return (
        "Execution environment: an outer OS sandbox is active. Write temporary files inside "
        f"the workspace or $TMPDIR ({scratch / 'tmp'}). Global /tmp and /var/tmp are not "
        "available. Other runs' temporary files and the benchmark checkout are inaccessible. "
        'For mktemp, supply an explicit template such as "$TMPDIR/probe.XXXXXX".\n\n'
    )


def add_usage(message: dict[str, Any], provider: str, model: str, totals: dict[str, Any]) -> None:
    """Fold one finished assistant message's usage into the run totals."""
    if message.get("provider") != provider or message.get("model") != model:
        raise ProviderError(
            f"Pi model mismatch: {message.get('provider')!r}/{message.get('model')!r}"
        )
    usage = message.get("usage")
    if not isinstance(usage, dict):
        raise ProviderError("Pi assistant message has no usage receipt")
    receipt = {}
    for name in ("input", "output", "cacheRead", "cacheWrite", "reasoning"):
        value = usage.get(name, 0)
        if type(value) is not int or value < 0:
            raise ProviderError("Invalid Pi token count")
        receipt[name] = value
    if receipt["reasoning"] > receipt["output"]:
        raise ProviderError("Inconsistent Pi token subsets")
    totals["calls"] += 1
    for name, value in receipt.items():
        totals[name] += value
    pi_cost = (usage.get("cost") or {}).get("total")
    if isinstance(pi_cost, (int, float)):
        totals["pi_cost_usd"] += float(pi_cost)


class PiAdapter:
    harness_id = "pi"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            self.harness_id,
            "Pi",
            "pi",
            True,
            True,
            True,
            True,
            False,
            "macOS outer sandbox; isolated writes; repository read deny",
        )

    def preflight(self) -> HarnessPreflight:
        try:
            node, cli, _, _ = pinned_install()
        except ProviderError as exc:
            return HarnessPreflight(self.harness_id, False, None, False, None, detail=str(exc))
        version = _version(node, cli)
        has_key = any(os.environ.get(k) for keys in MODELS.values() for k in keys)
        return HarnessPreflight(
            self.harness_id,
            bool(version),
            version,
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
        del kwargs
        if agent_container:
            raise ProviderError("Pi runs locally under the outer sandbox, not in a container")
        if model not in MODELS:
            raise ProviderError(f"Pi model {model!r} is not configured for VulcanBench")
        if effort is None:
            raise ProviderError("Pi requires an explicit thinking level")
        if timeout_s is None or timeout_s <= 0:
            raise ProviderError("Pi requires a positive wall-clock timeout")
        provider, _, provider_model = model.partition("/")
        key = next((os.environ[k] for k in MODELS[model] if os.environ.get(k)), None)
        if not key:
            raise ProviderError(f"Pi needs one of {', '.join(MODELS[model])} for {model}")
        checked = self.preflight()
        # HarnessPreflight.ready means subscription-ready; Pi is API-billed by design.
        if not (checked.available and checked.authenticated):
            raise ProviderError(f"Pi preflight failed: {checked.detail}")
        node, cli, bundle_sha256, config = pinned_install()
        config_sha256 = hashlib.sha256((config / "models.json").read_bytes()).hexdigest()
        workspace = Path(workspace).resolve()
        scratch = Path(tempfile.mkdtemp(prefix="vb-pi-session-")).resolve()
        (scratch / "tmp").mkdir()
        agent_dir = scratch / "agent"
        shutil.copytree(config, agent_dir)
        env = _subscription_env()
        env.update(
            {
                CHILD_KEY_ENV[provider]: key,
                "PI_CODING_AGENT_DIR": str(agent_dir),
                "PI_OFFLINE": "1",
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
            str(cli),
            "--mode",
            "json",
            *DISABLED_RESOURCES,
            "--provider",
            provider,
            "--model",
            provider_model,
            "--thinking",
            effort,
            sandbox_context(scratch) + prompt,
        ]
        session_id = str(uuid.uuid4())
        collector.record(
            "cli_agent_start",
            {
                "harness": self.harness_id,
                "argv": [*cmd[:-1], "<task prompt>"],
                "harness_version": checked.version,
                "bundle_sha256": bundle_sha256,
                "provider_config_sha256": config_sha256,
                "node": str(node),
                "requested_model": model,
                "requested_effort": effort,
                "billing": "api",
                "max_run_cost_usd": max_run_cost,
                "max_turns_note": "Pi has no step cap; the wall-clock timeout bounds the run",
            },
        )
        outcome = CliAgentOutcome(
            harness=self.harness_id,
            requested_model=model,
            session_id=session_id,
            harness_version=checked.version,
            auth_method="api-key",
            billing="api",
            cost_basis="metered-api-pricing",
            execution_boundary=(
                "macOS outer sandbox; workspace/session writes; shared-temp/checkout read deny; "
                "explicit TMPDIR context; no web tools; shell network not isolated; API key in "
                "the agent environment"
            ),
        )
        proc = subprocess.Popen(
            cmd,
            cwd=workspace,
            env=env,
            stdin=subprocess.DEVNULL,
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
            with suppress(ProcessLookupError):
                os.killpg(proc.pid, signal.SIGKILL)

        def on_timeout() -> None:
            outcome.timed_out = True
            kill()

        timer = threading.Timer(timeout_s, on_timeout)
        timer.daemon = True
        timer.start()
        totals: dict[str, Any] = dict(
            input=0, output=0, cacheRead=0, cacheWrite=0, reasoning=0, calls=0, pi_cost_usd=0.0
        )
        last_stop: str | None = None
        last_error = ""
        settled = False
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
                    if kind == "agent_settled":
                        settled = True
                    if kind != "message_end":
                        continue
                    message = event.get("message") or {}
                    if message.get("role") != "assistant":
                        continue
                    add_usage(message, provider, provider_model, totals)
                    last_stop = message.get("stopReason")
                    last_error = str(message.get("errorMessage") or "")
                    text = "".join(
                        part.get("text", "")
                        for part in message.get("content") or []
                        if isinstance(part, dict) and part.get("type") == "text"
                    )
                    if text:
                        collector.record("assistant_text", {"text": text})
                    if max_run_cost is not None:
                        spent = cost_usd(
                            priced_spec,
                            totals["input"] + totals["cacheRead"] + totals["cacheWrite"],
                            totals["output"],
                            cached_input_tokens=totals["cacheRead"],
                        )
                        if spent is not None and spent > max_run_cost:
                            outcome.cost_capped = True
                            kill()
            proc.wait()
        finally:
            timer.cancel()
            if proc.poll() is None:
                kill()
                proc.wait()
            reader.join(timeout=5)
        if stream_log_path:
            archive = Path(stream_log_path).parent / "pi-session"
            archive.mkdir(exist_ok=True)
            (archive / "stderr.txt").write_text(str(sanitize("".join(errors))))
            (archive / "boundary.sb").write_text(profile)
        outcome.prompt_tokens = totals["input"] + totals["cacheRead"] + totals["cacheWrite"]
        outcome.completion_tokens = totals["output"]
        outcome.cached_input_tokens = totals["cacheRead"]
        outcome.reasoning_output_tokens = totals["reasoning"]
        outcome.num_turns = totals["calls"]
        outcome.reported_model = model if totals["calls"] else None
        outcome.model_identity_confidence = (
            "provider-reported" if totals["calls"] else "requested-only"
        )
        outcome.cli_reported_cost_usd = round(totals["pi_cost_usd"], 6) if totals["calls"] else None
        outcome.finished = bool(settled and last_stop == "stop" and proc.returncode == 0)
        outcome.subtype = last_stop or "no-assistant-message"
        collector.record("pi_usage", totals)
        if not (outcome.timed_out or outcome.cost_capped or outcome.finished):
            detail = last_error or "".join(errors)[-2000:]
            raise ProviderError(
                f"Pi failed, exit={proc.returncode}, stop={last_stop}: {detail[:500]}"
            )
        collector.record("cli_agent_result", outcome.summary())
        return outcome
