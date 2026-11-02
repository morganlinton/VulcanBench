"""Kimi Code CLI adapter: Moonshot's own agent, API-metered, audited usage.

Kimi Code CLI (``@moonshot-ai/kimi-code``, MoonshotAI/kimi-code, MIT) is
Moonshot AI's terminal agent; it replaced the archived ``kimi-cli``. VulcanBench
runs its one-shot print mode (``kimi -p``) against a pinned install, inside the
same macOS outer sandbox as dsh and Muse Code, with a per-run
``KIMI_CODE_HOME`` so no saved setting, credential or prior session leaks in.

Run settings (owner decisions 2026-11-02, docs/DECISIONS.md):

- Billing is the Moonshot Platform API key, pay as you go, against
  ``api.moonshot.ai/v1`` with the exact model id (never the Kimi Code
  subscription endpoint or its ``kimi-for-coding`` alias).
- Print mode runs in Kimi Code's "Never Ask" permission mode; the outer sandbox
  is the boundary, as for dsh, because sandbox-exec cannot nest.
- Web tools are off, as for every harness on the board.
- Telemetry is off (``telemetry = false`` and ``KIMI_DISABLE_TELEMETRY``), and
  so is the update check. The model never sees either, so behaviour is unchanged;
  task content still reaches Moonshot in the model requests.
- Background tasks end with the main turn (``print_background_mode = "exit"``)
  so a run cannot outlive its answer.
- The model entry declares what Moonshot's ``/models`` reports for the model:
  always-thinking, efforts low/high/max, 1,048,576-token context. Output is
  capped at 131,072 tokens per call, the API's own default, because Kimi Code
  otherwise asks for the whole context and Moonshot counts the request's
  ``max_completion_tokens`` against the account's rate limit.

Token use is read from the ``usage.record`` events in each agent's
``wire.jsonl``, one per model call, which split input into fresh tokens, cache
reads and cache writes. Moonshot bills 5-minute cache writes at the input rate,
so only cache reads take the cached price.

Imported lazily by cli_agents to avoid circular adapter registration.
"""

from __future__ import annotations

import json
import os
import signal
import subprocess
import tempfile
import threading
from contextlib import suppress
from pathlib import Path
from typing import Any

from harness.agent.cli_agents import (
    CliAgentOutcome,
    HarnessCapabilities,
    HarnessPreflight,
    _subscription_env,
)
from harness.agent.deepseek_harness import sandbox_context, tree_sha256
from harness.agent.muse_code import boundary_profile
from harness.agent.providers import ProviderError
from harness.pricing import cost_usd
from harness.redaction import sanitize

VERSION = "2.4.1"
PACKAGE = "@moonshot-ai/kimi-code"
BASE_URL = "https://api.moonshot.ai/v1"
# VulcanBench model spec -> Moonshot API model id (identical today), with the
# context size Moonshot's /models reports for it.
MODELS = {"kimi-k3.1-pro": "kimi-k3.1-pro"}
CONTEXT_TOKENS = {"kimi-k3.1-pro": 1_048_576}
MAX_OUTPUT_TOKENS = 131_072
# Moonshot's reasoning efforts for K3.1 Pro (thinking cannot be turned off).
EFFORTS = frozenset({"low", "high", "max"})
API_KEY_ENV = "MOONSHOT_API_KEY"
DISABLED_TOOLS = ("WebSearch", "FetchURL")
# Moonshot's error types for failures that are the account or the API, not the model's work.
INFRA_MARKERS = (
    "invalid_authentication_error",
    "exceeded_current_quota_error",
    "insufficient_balance",
    "rate_limit_reached_error",
    "engine_overloaded_error",
    "server_error",
    "ECONNRESET",
    "ETIMEDOUT",
)


def benchmark_config(model: str, effort: str) -> str:
    """The run's whole ``config.toml``: model, effort and the data settings."""
    tools = ", ".join(json.dumps(name) for name in DISABLED_TOOLS)
    return f"""# VulcanBench config for one Kimi Code print-mode run.
default_model = "{model}"
default_permission_mode = "auto"
telemetry = false

[providers.moonshot]
type = "kimi"
base_url = "{BASE_URL}"
api_key_env = "{API_KEY_ENV}"

[models."{model}"]
provider = "moonshot"
model = "{model}"
max_context_size = {CONTEXT_TOKENS[model]}
max_output_size = {MAX_OUTPUT_TOKENS}
capabilities = ["thinking", "always_thinking", "tool_use", "image_in", "video_in"]
support_efforts = ["low", "high", "max"]
default_effort = "max"

[thinking]
effort = "{effort}"

[background]
print_background_mode = "exit"

[tools]
disabled = [{tools}]
"""


def pinned_install() -> tuple[Path, Path, str]:
    """Node executable, the kimi entry point and the verified tree hash of the install."""
    node = Path(os.environ.get("VULCANBENCH_KIMI_CODE_NODE", ""))
    prefix = Path(os.environ.get("VULCANBENCH_KIMI_CODE_PREFIX", ""))
    expected = os.environ.get("VULCANBENCH_KIMI_CODE_SHA256", "")
    if (
        not str(node)
        or not node.is_absolute()
        or not node.is_file()
        or not os.access(node, os.X_OK)
    ):
        raise ProviderError("Set VULCANBENCH_KIMI_CODE_NODE to an absolute, executable node binary")
    if not str(prefix) or not prefix.is_absolute() or not prefix.is_dir():
        raise ProviderError("Set VULCANBENCH_KIMI_CODE_PREFIX to the absolute kimi-code prefix")
    package = prefix / "node_modules" / PACKAGE
    try:
        version = json.loads((package / "package.json").read_text())["version"]
    except (OSError, KeyError, json.JSONDecodeError) as exc:
        raise ProviderError("No kimi-code package under VULCANBENCH_KIMI_CODE_PREFIX") from exc
    if version != VERSION:
        raise ProviderError(f"kimi-code {version} installed, adapter pins {VERSION}")
    actual = tree_sha256(prefix / "node_modules")
    if len(expected) != 64 or actual != expected:
        raise ProviderError("kimi-code install tree SHA256 mismatch")
    return node.resolve(), (package / "dist/main.mjs").resolve(), actual


def _zero() -> dict[str, Any]:
    return dict(input=0, output=0, cacheRead=0, cacheWrite=0, calls=0, metered_usd=0.0)


def collect_usage(paths: list[Path], model: str, priced_spec: str) -> dict[str, Any]:
    """Sum billed usage across every agent's ``wire.jsonl``, subagents included.

    Each ``usage.record`` is one model call and must name ``model``. ``input``
    is the fresh input only; cache reads and writes are counted apart. A record
    seen in two logs (a parent echoing a subagent's call) is counted once.
    """
    totals = _zero()
    seen: set[str] = set()
    for path in sorted(paths):
        with path.open() as source:
            for line in source:
                row = json.loads(line)
                if row.get("type") != "usage.record":
                    continue
                key = json.dumps(row, sort_keys=True)
                if key in seen:
                    continue
                seen.add(key)
                if row.get("model") != model:
                    raise ProviderError(f"kimi-code model mismatch: {row.get('model')!r}")
                usage = row.get("usage")
                if not isinstance(usage, dict):
                    raise ProviderError("kimi-code usage record is not an object")
                receipt = {}
                for wire, name in (
                    ("inputOther", "input"),
                    ("output", "output"),
                    ("inputCacheRead", "cacheRead"),
                    ("inputCacheCreation", "cacheWrite"),
                ):
                    value = usage.get(wire, 0)
                    if type(value) is not int or value < 0:
                        raise ProviderError("Invalid kimi-code token count")
                    receipt[name] = value
                prompt = receipt["input"] + receipt["cacheRead"] + receipt["cacheWrite"]
                spent = cost_usd(
                    priced_spec, prompt, receipt["output"], cached_input_tokens=receipt["cacheRead"]
                )
                if spent is None:
                    raise ProviderError(f"No price for {priced_spec}")
                totals["metered_usd"] += spent
                totals["calls"] += 1
                for name, value in receipt.items():
                    totals[name] += value
    totals["metered_usd"] = round(totals["metered_usd"], 6)
    return totals


class KimiCodeAdapter:
    harness_id = "kimi-code"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            self.harness_id,
            "Kimi Code CLI",
            "kimi",
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
            raise ProviderError(
                "kimi-code runs locally under the outer sandbox, not in a container"
            )
        if network:
            raise ProviderError(
                "kimi-code runs keep web tools off; network tasks are not supported"
            )
        if max_run_cost is not None:
            raise ProviderError("kimi-code runs are bounded by wall clock, not a live cost cap")
        if model not in MODELS:
            raise ProviderError(f"kimi-code model {model!r} is not configured for VulcanBench")
        if effort not in EFFORTS:
            raise ProviderError("kimi-code requires an explicit low, high or max effort")
        if timeout_s is None or timeout_s <= 0:
            raise ProviderError("kimi-code requires a positive wall-clock timeout")
        key = os.environ.get(API_KEY_ENV)
        if not key:
            raise ProviderError(f"kimi-code needs {API_KEY_ENV}")
        node, entry, tree_sha = pinned_install()
        api_model = MODELS[model]
        workspace = Path(workspace).resolve()
        scratch = Path(tempfile.mkdtemp(prefix="vb-kimi-session-")).resolve()
        (scratch / "tmp").mkdir()
        home = scratch / "kimi-home"
        home.mkdir()
        config = benchmark_config(api_model, effort)
        (home / "config.toml").write_text(config)
        env = _subscription_env()
        env.update(
            {
                API_KEY_ENV: key,
                "KIMI_CODE_HOME": str(home),
                "KIMI_DISABLE_TELEMETRY": "1",
                "KIMI_CODE_NO_AUTO_UPDATE": "1",
                "KIMI_CODE_TRUST_WORKSPACE": "1",
                "KIMI_DISABLE_CRON": "1",
                "TMPDIR": str(scratch / "tmp"),
                "TMP": str(scratch / "tmp"),
                "TEMP": str(scratch / "tmp"),
            }
        )
        profile = boundary_profile(workspace, scratch)
        profile_path = scratch / "boundary.sb"
        profile_path.write_text(profile)
        # Print mode takes the task only as an argument; it reaches the process
        # table, which on this single-operator host shows nothing new.
        cmd = [
            "/usr/bin/sandbox-exec",
            "-f",
            str(profile_path),
            str(node),
            str(entry),
            "--output-format",
            "stream-json",
            "-p",
            sandbox_context(scratch) + prompt,
        ]
        collector.record(
            "cli_agent_start",
            {
                "harness": self.harness_id,
                "argv": [*cmd[:-1], "<task prompt>"],
                "harness_version": VERSION,
                "install_tree_sha256": tree_sha,
                "node": str(node),
                "requested_model": model,
                "api_model": api_model,
                "requested_effort": effort,
                "billing": "api",
                "config_toml": config,
                "max_turns_note": "print mode has no step cap; the wall-clock timeout bounds the run",
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
                "macOS outer sandbox; Kimi Code auto permission mode; workspace/session writes; "
                "shared-temp/checkout read deny; explicit TMPDIR context; web tools off; "
                "telemetry and update check off; shell network not isolated; API key in the "
                "agent environment"
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
        answered = False
        try:
            with Path(stream_log_path or scratch / "stdout.jsonl").open("w") as log:
                for line in stdout:
                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    log.write(json.dumps(sanitize(event)) + "\n")
                    log.flush()
                    if event.get("role") == "meta" and event.get("session_id"):
                        outcome.session_id = event["session_id"]
                    elif event.get("role") == "assistant" and event.get("content"):
                        answered = not event.get("tool_calls")
                        collector.record("assistant_text", {"text": event["content"]})
            proc.wait()
        finally:
            timer.cancel()
            if proc.poll() is None:
                kill()
                proc.wait()
            reader.join(timeout=5)
        sessions = home / "sessions"
        logs = sorted(sessions.rglob("wire.jsonl"))
        if stream_log_path:
            archive = Path(stream_log_path).parent / "kimi-code-sessions"
            archive.mkdir(exist_ok=True)
            for path in sorted(p for p in sessions.rglob("*") if p.is_file()):
                target = archive / path.relative_to(sessions)
                target.parent.mkdir(parents=True, exist_ok=True)
                if path.suffix == ".jsonl":
                    with path.open() as source, target.open("w") as destination:
                        for line in source:
                            destination.write(json.dumps(sanitize(json.loads(line))) + "\n")
                else:
                    target.write_text(str(sanitize(path.read_text(errors="replace"))))
            (archive / "stderr.txt").write_text(str(sanitize("".join(errors))))
            (archive / "boundary.sb").write_text(profile)
            (archive / "config.toml").write_text(config)
        usage = collect_usage(logs, api_model, priced_spec)
        outcome.prompt_tokens = usage["input"] + usage["cacheRead"] + usage["cacheWrite"]
        outcome.completion_tokens = usage["output"]
        outcome.cached_input_tokens = usage["cacheRead"]
        outcome.num_turns = usage["calls"]
        outcome.reported_model = model if usage["calls"] else None
        outcome.model_identity_confidence = (
            "harness-recorded" if usage["calls"] else "requested-only"
        )
        outcome.cli_reported_cost_usd = usage["metered_usd"] if usage["calls"] else None
        outcome.finished = bool(answered and proc.returncode == 0 and not outcome.timed_out)
        outcome.subtype = "completed" if outcome.finished else f"exit-{proc.returncode}"
        collector.record("kimi_code_usage", usage)
        if outcome.finished and not usage["calls"]:
            # A run that answered without a usage receipt cannot be audited and must not be scored.
            raise ProviderError("kimi-code run has no auditable usage: no wire.jsonl receipts")
        if not (outcome.timed_out or outcome.finished):
            detail = "".join(errors)[-2000:]
            infra = any(marker in detail for marker in INFRA_MARKERS)
            raise ProviderError(
                f"kimi-code {'API failure' if infra else 'failed'}, "
                f"exit={proc.returncode}: {detail[:500]}"
            )
        collector.record("cli_agent_result", outcome.summary())
        return outcome
