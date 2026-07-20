"""Run models inside their vendor's own agent CLI (subscription billing).

``claude-code:<model>`` runs a task with Claude Code headless (``claude -p``)
in the prepared workspace instead of the VulcanBench agent loop. The CLI edits
files directly in the workspace; everything downstream (git diff, verifier,
evaluator, scoring) is unchanged.

Why this exists: Claude Code authenticates with a Claude subscription
(Pro/Max), so runs bill the subscription instead of API rates — and it is also
a legitimate benchmark target in its own right, since most people use the
model *through* its vendor harness. Two honesty rules follow:

- Results measure **model + vendor harness**, not the VulcanBench uniform
  loop. A ``claude-code:claude-opus-4-8`` column is not comparable to an
  ``anthropic:claude-opus-4-8`` column; the summary records the harness so
  the leaderboard can't silently mix them.
- ``cost_usd`` is the **hypothetical API cost** computed from the CLI's
  reported token usage at API rates (``harness.pricing`` maps
  ``claude-code:`` specs to ``anthropic:`` prices). The run did not pay it;
  ``cli_agent.billing = "subscription"`` says so, and the CLI's own estimate
  is kept alongside as ``cli_reported_cost_usd``.

Subscription plans have rolling usage limits. A limit hit raises
:class:`~harness.agent.providers.ProviderError` so the suite records an
*error* (resumable with ``--only-missing``) instead of scoring a starved run
as a 0.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import threading
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

from harness.agent.providers import (
    LLMProvider,
    LLMResponse,
    ProviderError,
    TokenUsage,
)
from harness.pricing import cost_usd
from harness.redaction import sanitize

CLI_AGENT_PROVIDERS = frozenset({"claude-code"})

# Claude Code's headless result text when a subscription window is exhausted
# (e.g. "Claude AI usage limit reached|...", "5-hour limit reached ∙ resets 3am").
_LIMIT_PATTERN = re.compile(r"usage limit|rate limit|limit reached|limit will reset", re.I)

# The VulcanBench loop has no web tools, so parity default is web-off; the
# ``--network`` flag opts back in (the CLI runs host-side, so this only gates
# the agent's tools, not the host's connectivity).
_WEB_TOOLS = "WebSearch,WebFetch"

# Single-shot judge/grader calls must not wander the filesystem.
_JUDGE_DISALLOWED_TOOLS = (
    "Bash,Edit,Write,NotebookEdit,Read,Glob,Grep,WebSearch,WebFetch,Task,TodoWrite"
)

_ISSUE_SUFFIX = (
    "\n\nSolve this issue in the current repository. Make the smallest correct "
    "change and run the tests to verify it. Leave your changes uncommitted in "
    "the working tree — do not create git commits."
)


class _Collector(Protocol):
    def record(self, event_type: str, data: dict[str, Any]) -> None: ...


def is_cli_agent_spec(spec: str) -> bool:
    """True when ``spec`` selects a vendor agent CLI (e.g. ``claude-code:...``)."""
    provider = spec.partition(":")[0].strip().lower()
    return provider in CLI_AGENT_PROVIDERS


def build_cli_prompt(issue: str) -> str:
    """The kickoff prompt handed to the agent CLI for a task."""
    return f"# Issue\n\n{issue}{_ISSUE_SUFFIX}"


def _subscription_env() -> dict[str, str]:
    """Subprocess env forcing subscription auth.

    With ``ANTHROPIC_API_KEY`` set, Claude Code bills the API — which defeats
    the point of CLI-agent mode and silently double-spends. Strip it so the
    CLI uses the logged-in subscription (or ``CLAUDE_CODE_OAUTH_TOKEN``).
    """
    env = dict(os.environ)
    env.pop("ANTHROPIC_API_KEY", None)
    return env


def _fold_usage(usage: dict[str, Any]) -> tuple[int, int]:
    """Anthropic usage -> (effective prompt tokens, completion tokens).

    Same fold as ``AnthropicProvider``: ``input_tokens`` is the uncached
    remainder; cache reads bill ~0.1x and cache writes ~1.25x.
    """
    uncached = int(usage.get("input_tokens", 0) or 0)
    cache_read = int(usage.get("cache_read_input_tokens", 0) or 0)
    cache_write = int(usage.get("cache_creation_input_tokens", 0) or 0)
    prompt = round(uncached + cache_read * 0.1 + cache_write * 1.25)
    return prompt, int(usage.get("output_tokens", 0) or 0)


def _fold_usage_totals(usages: Iterable[dict[str, Any]]) -> tuple[int, int]:
    prompt = completion = 0
    for usage in usages:
        p, c = _fold_usage(usage)
        prompt += p
        completion += c
    return prompt, completion


@dataclass
class CliAgentOutcome:
    """What a CLI-agent run produced, in the loop's accounting terms."""

    prompt_tokens: int = 0
    completion_tokens: int = 0
    finished: bool = False
    cost_capped: bool = False
    timed_out: bool = False
    session_id: str | None = None
    subtype: str | None = None
    num_turns: int | None = None
    cli_reported_cost_usd: float | None = None

    def summary(self) -> dict[str, Any]:
        """Provenance block persisted into the run summary."""
        return {
            "harness": "claude-code",
            "billing": "subscription",
            "cost_basis": "hypothetical-api-pricing",
            "session_id": self.session_id,
            "subtype": self.subtype,
            "num_turns": self.num_turns,
            "cli_reported_cost_usd": self.cli_reported_cost_usd,
        }


def run_claude_code_task(  # noqa: PLR0912, PLR0915 — linear stream-parse loop
    *,
    workspace: Path,
    prompt: str,
    model: str,
    priced_spec: str,
    max_turns: int,
    collector: _Collector,
    stream_log_path: Path | None = None,
    timeout_s: float | None = None,
    network: bool = False,
    max_run_cost: float | None = None,
    claude_bin: str = "claude",
) -> CliAgentOutcome:
    """Run one task with Claude Code headless in ``workspace``.

    Streams ``--output-format stream-json`` events into the trace (so
    ``replay.html`` still works), enforces the wall-clock budget with a kill
    timer, and enforces ``max_run_cost`` against the cumulative hypothetical
    API cost of the streamed usage. Partial work survives a timeout or cost
    cap and is diffed/verified by the caller, mirroring the loop's semantics.
    """
    if timeout_s is not None and timeout_s <= 0:
        raise ProviderError("run budget exhausted before CLI agent start")

    cmd = [
        claude_bin,
        "-p",
        prompt,
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        model,
        "--max-turns",
        str(max_turns),
        "--dangerously-skip-permissions",
        # Hermetic runs: don't let the operator's user-level config/memory
        # leak instructions into the benchmark.
        "--setting-sources",
        "project",
    ]
    if not network:
        cmd += ["--disallowedTools", _WEB_TOOLS]

    collector.record(
        "cli_agent_start",
        {"harness": "claude-code", "argv": [cmd[0], "-p", "<prompt omitted>", *cmd[3:]]},
    )

    try:
        proc = subprocess.Popen(
            cmd,
            cwd=workspace,
            env=_subscription_env(),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except FileNotFoundError as e:
        raise ProviderError(
            f"{claude_bin!r} not found on PATH; install Claude Code and sign in "
            "with your subscription (run `claude` once, or set CLAUDE_CODE_OAUTH_TOKEN)"
        ) from e

    stderr_chunks: list[str] = []

    def _drain_stderr() -> None:
        assert proc.stderr is not None
        for chunk in proc.stderr:
            stderr_chunks.append(chunk)

    stderr_thread = threading.Thread(target=_drain_stderr, daemon=True)
    stderr_thread.start()

    outcome = CliAgentOutcome()
    killed = {"timeout": False, "cost": False}

    def _kill_on_timeout() -> None:
        killed["timeout"] = True
        proc.kill()

    watchdog: threading.Timer | None = None
    if timeout_s is not None:
        watchdog = threading.Timer(timeout_s, _kill_on_timeout)
        watchdog.daemon = True
        watchdog.start()

    usage_by_msg: dict[str, dict[str, Any]] = {}
    result_msg: dict[str, Any] | None = None
    stream_f = stream_log_path.open("w", encoding="utf-8") if stream_log_path else None
    try:
        assert proc.stdout is not None
        for raw_line in proc.stdout:
            line = raw_line.strip()
            if not line:
                continue
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if stream_f:
                json.dump(sanitize(event), stream_f)
                stream_f.write("\n")
            etype = event.get("type")
            if etype == "system" and event.get("subtype") == "init":
                outcome.session_id = event.get("session_id")
                collector.record(
                    "cli_agent_init",
                    {"session_id": outcome.session_id, "model": event.get("model")},
                )
            elif etype == "assistant":
                msg = event.get("message") or {}
                usage = msg.get("usage")
                if isinstance(usage, dict):
                    # Keyed by message id: some CLI versions re-emit a message
                    # per content block; overwriting avoids double counting.
                    usage_by_msg[str(msg.get("id") or len(usage_by_msg))] = usage
                collector.record("llm_response", _assistant_trace_data(msg))
                if max_run_cost is not None:
                    p, c = _fold_usage_totals(usage_by_msg.values())
                    run_cost = cost_usd(priced_spec, p, c)
                    if run_cost is not None and run_cost >= max_run_cost:
                        collector.record(
                            "cost_cap_exceeded",
                            {"cost_usd": run_cost, "max_run_cost": max_run_cost},
                        )
                        outcome.cost_capped = True
                        killed["cost"] = True
                        proc.kill()
                        break
            elif etype == "user":
                for block in (event.get("message") or {}).get("content") or []:
                    if isinstance(block, dict) and block.get("type") == "tool_result":
                        collector.record(
                            "tool_observation",
                            {
                                "tool": block.get("tool_use_id", ""),
                                "result": block.get("content"),
                                "error": "tool error" if block.get("is_error") else None,
                            },
                        )
            elif etype == "result":
                result_msg = event
    finally:
        if watchdog is not None:
            watchdog.cancel()
        if stream_f:
            stream_f.close()

    proc.wait()
    stderr_thread.join(timeout=5)
    outcome.timed_out = killed["timeout"]

    if result_msg is None:
        # Killed by the budget/cost watchdog (partial work still counts), or
        # the CLI died without reporting — approximate usage from the stream.
        outcome.prompt_tokens, outcome.completion_tokens = _fold_usage_totals(usage_by_msg.values())
        outcome.num_turns = len(usage_by_msg)
        if not (killed["timeout"] or killed["cost"]):
            tail = "".join(stderr_chunks)[-500:].strip()
            raise ProviderError(
                f"claude code exited without a result (exit {proc.returncode}): {tail or 'no stderr'}"
            )
        return outcome

    usage = result_msg.get("usage") or {}
    outcome.prompt_tokens, outcome.completion_tokens = _fold_usage(usage)
    outcome.subtype = result_msg.get("subtype")
    outcome.num_turns = result_msg.get("num_turns")
    outcome.session_id = result_msg.get("session_id") or outcome.session_id
    reported = result_msg.get("total_cost_usd")
    if isinstance(reported, (int, float)):
        outcome.cli_reported_cost_usd = float(reported)
    result_text = str(result_msg.get("result") or "")

    if result_msg.get("is_error") or outcome.subtype != "success":
        if _LIMIT_PATTERN.search(result_text):
            raise ProviderError(
                "claude code subscription limit hit — rerun after the window "
                f"resets (use --only-missing to resume): {result_text[:300]}"
            )
        if outcome.subtype == "error_max_turns":
            # Ran out of turns: a legitimate outcome (like the loop exhausting
            # max_steps); the partial diff is verified and scored honestly.
            collector.record("cli_agent_result", outcome.summary())
            return outcome
        raise ProviderError(f"claude code run failed ({outcome.subtype}): {result_text[:300]}")

    outcome.finished = True
    collector.record("cli_agent_result", outcome.summary())
    return outcome


def _assistant_trace_data(msg: dict[str, Any]) -> dict[str, Any]:
    """Mirror the loop's ``llm_response`` trace shape so replay.html renders."""
    blocks = [b for b in msg.get("content") or [] if isinstance(b, dict)]
    text = "\n".join(b.get("text", "") for b in blocks if b.get("type") == "text")
    tool_calls = [
        {"id": b.get("id", ""), "name": b.get("name", ""), "arguments": b.get("input") or {}}
        for b in blocks
        if b.get("type") == "tool_use"
    ]
    return {"content": text or None, "tool_calls": tool_calls, "usage": msg.get("usage") or {}}


class ClaudeCodeProvider(LLMProvider):
    """Single-shot completions through Claude Code headless.

    Used for judge/grader calls when the run model is a ``claude-code:`` spec,
    so evaluation also bills the subscription. Tool calling is not supported —
    judges and graders are plain prompt-in/JSON-out completions.
    """

    @property
    def name(self) -> str:
        return "claude-code"

    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        del tools, effort
        system = "\n\n".join(
            str(m.get("content", "")) for m in messages if m.get("role") == "system"
        ).strip()
        prompt = "\n\n".join(
            str(m.get("content", "")) for m in messages if m.get("role") != "system"
        )
        cmd = [
            "claude",
            "-p",
            prompt,
            "--output-format",
            "json",
            "--model",
            self.model,
            "--max-turns",
            "1",
            "--setting-sources",
            "project",
            "--disallowedTools",
            _JUDGE_DISALLOWED_TOOLS,
        ]
        if system:
            cmd += ["--append-system-prompt", system]
        try:
            proc = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=timeout_s if timeout_s and timeout_s > 0 else 600,
                env=_subscription_env(),
                check=False,
            )
        except FileNotFoundError as e:
            raise ProviderError("'claude' not found on PATH for judge/grader call") from e
        except subprocess.TimeoutExpired as e:
            raise ProviderError("claude code judge/grader call timed out") from e
        if proc.returncode != 0:
            raise ProviderError(
                f"claude code judge call failed (exit {proc.returncode}): "
                f"{(proc.stderr or proc.stdout)[-300:]}"
            )
        try:
            body: dict[str, Any] = json.loads(proc.stdout)
        except json.JSONDecodeError as e:
            raise ProviderError(f"claude code returned non-JSON output: {proc.stdout[:200]}") from e
        text = str(body.get("result") or "")
        if body.get("is_error"):
            if _LIMIT_PATTERN.search(text):
                raise ProviderError(f"claude code subscription limit hit: {text[:300]}")
            raise ProviderError(f"claude code judge call errored: {text[:300]}")
        p, c = _fold_usage(body.get("usage") or {})
        return LLMResponse(
            content=text or None,
            usage=TokenUsage(prompt_tokens=p, completion_tokens=c),
            raw=body,
        )
