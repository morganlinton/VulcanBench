"""Run models inside their own agent CLI (subscription or API-metered).

``claude-code:<model>``, ``codex:<model>``, ``cursor:<model>``, ``grok-build:<model>``,
``zcode:<model>``, and ``pi:<inner-spec>`` run a task
in the product's headless CLI instead of the VulcanBench agent loop.  The
external harness owns its prompts, context management, and tools; everything
downstream (git diff, verifier, evaluator, scoring) remains under VulcanBench.

Why this exists: Claude Code authenticates with a Claude subscription
(Pro/Max), so runs bill the subscription instead of API rates, and it is also
a legitimate benchmark target in its own right, since most people use the
model *through* its vendor harness. Two honesty rules follow:

- Results measure **model + vendor harness**, not the VulcanBench uniform
  loop. A ``claude-code:claude-opus-4-8`` column is not comparable to an
  ``anthropic:claude-opus-4-8`` column; the summary records the harness so
  the leaderboard can't silently mix them.
- ``cost_usd`` remains a backward-compatible API-equivalent value.  The
  ``economics`` receipt is authoritative and separates marginal cash, plan
  allocation, quota consumption, and API-equivalent value.

Subscription plans have rolling usage limits. A limit hit raises
:class:`~harness.agent.providers.ProviderError` so the suite records an
*error* (resumable with ``--only-missing``) instead of scoring a starved run
as a 0.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import sqlite3
import subprocess
import threading
import time
import uuid
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

from harness.agent.providers import (
    META_DEFAULT_BASE_URL,
    LLMProvider,
    LLMResponse,
    NonRetryableProviderError,
    ProviderError,
    TokenUsage,
    _resolve_meta_route,
    parse_model_spec,
)
from harness.pricing import cost_usd
from harness.redaction import sanitize

SUBSCRIPTION_HARNESSES = frozenset({"claude-code", "codex", "cursor", "grok-build", "zcode"})
API_HARNESSES = frozenset({"pi"})
CLI_AGENT_PROVIDERS = frozenset({*SUBSCRIPTION_HARNESSES, *API_HARNESSES})

_PI_PASSTHROUGH_ENV = frozenset(
    {
        "META_MUSE_SPARK_API",
        "MODEL_API_KEY",
        "OPENROUTER_API_KEY",
        "META_BASE_URL",
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_BASE_URL",
        "ZAI_API_KEY",
        "ZAI_BASE_URL",
        "MOONSHOT_API_KEY",
        "MOONSHOT_BASE_URL",
        "DASHSCOPE_API_KEY",
        "DASHSCOPE_BASE_URL",
        "DEEPSEEK_API_KEY",
        "DEEPSEEK_BASE_URL",
        "XAI_API_KEY",
        "XAI_BASE_URL",
    }
)

# Claude Code's headless result text when a subscription window is exhausted
# (e.g. "Claude AI usage limit reached|...", "5-hour limit reached ∙ resets 3am").
_LIMIT_PATTERN = re.compile(r"usage limit|rate limit|limit reached|limit will reset", re.I)

_SAFE_ENV_KEYS = frozenset(
    {
        "PATH",
        "HOME",
        "SHELL",
        "TMPDIR",
        "LANG",
        "TERM",
        "USER",
        "LOGNAME",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "CODEX_HOME",
        "CLAUDE_CONFIG_DIR",
        "GROK_HOME",
        # ZCode: alternate node binary for the npm launcher, and the storage /
        # credential roots a user may have relocated. No ZCODE_API_KEY /
        # ZCODE_BASE_URL: those select API-key billing and are deliberately absent.
        "ZCODE_NODE",
        "ZCODE_HOME",
        "ZCODE_DATA_BASE_DIR",
        "ZCODE_STORAGE_DIR",
        "NVM_DIR",
    }
)

# The VulcanBench loop has no web tools, so parity default is web-off; the
# ``--network`` flag opts back in (the CLI runs host-side, so this only gates
# the agent's tools, not the host's connectivity).
_WEB_TOOLS = "WebSearch,WebFetch"

# Cursor permission sets. ``allow`` covers the tools a benchmark run needs;
# ``deny`` blocks the web tools that would let an agent fetch its task's own
# upstream fix. The schema requires both keys.
_CURSOR_WEB_DENIED_PERMISSIONS = {
    "allow": [
        "Shell(*)",
        "Read(*)",
        "Write(*)",
        "Edit(*)",
        "Glob(*)",
        "Grep(*)",
        "Delete(*)",
        "Ls(*)",
    ],
    "deny": ["WebFetch(*)", "WebSearch", "WebSearch(*)"],
}

# Single-shot judge/grader calls must not wander the filesystem.
_JUDGE_DISALLOWED_TOOLS = (
    "Bash,Edit,Write,NotebookEdit,Read,Glob,Grep,WebSearch,WebFetch,Task,TodoWrite"
)

_ISSUE_SUFFIX = (
    "\n\nSolve this issue in the current repository. Make the smallest correct "
    "change and run the tests to verify it. Leave your changes uncommitted in "
    "the working tree, do not create git commits."
)


class _Collector(Protocol):
    def record(self, event_type: str, data: dict[str, Any]) -> None: ...


class SubscriptionQuotaError(NonRetryableProviderError):
    """A rolling subscription limit that should pause, not hot-loop retries."""


@dataclass(frozen=True)
class HarnessCapabilities:
    """Features an external harness can prove to VulcanBench."""

    harness: str
    display_name: str
    executable: str
    structured_events: bool
    reports_tokens: bool
    reports_model: bool
    supports_effort: bool
    supports_live_cost_cap: bool
    sandbox: str

    def as_summary(self) -> dict[str, Any]:
        return {
            "harness": self.harness,
            "display_name": self.display_name,
            "executable": self.executable,
            "structured_events": self.structured_events,
            "reports_tokens": self.reports_tokens,
            "reports_model": self.reports_model,
            "supports_effort": self.supports_effort,
            "supports_live_cost_cap": self.supports_live_cost_cap,
            "sandbox": self.sandbox,
        }


@dataclass(frozen=True)
class HarnessPreflight:
    """Non-secret readiness receipt returned by ``harness doctor``."""

    harness: str
    available: bool
    version: str | None
    authenticated: bool
    auth_mode: str | None
    plan_name: str | None = None
    detail: str | None = None

    @property
    def ready(self) -> bool:
        if not (self.available and self.authenticated):
            return False
        if self.auth_mode == "subscription":
            return True
        return self.harness in API_HARNESSES and self.auth_mode == "api"

    def as_summary(self) -> dict[str, Any]:
        return {
            "harness": self.harness,
            "available": self.available,
            "version": self.version,
            "authenticated": self.authenticated,
            "auth_mode": self.auth_mode,
            "plan_name": self.plan_name,
            "ready": self.ready,
            "detail": self.detail,
        }


class CliAgentAdapter(Protocol):
    """Contract implemented by subscription-backed execution harnesses."""

    @property
    def harness_id(self) -> str: ...

    def capabilities(self) -> HarnessCapabilities: ...

    def preflight(self) -> HarnessPreflight: ...

    def run_task(self, **kwargs: Any) -> CliAgentOutcome: ...


def is_cli_agent_spec(spec: str) -> bool:
    """True when ``spec`` selects a vendor agent CLI (e.g. ``claude-code:...``)."""
    provider = spec.partition(":")[0].strip().lower()
    return provider in CLI_AGENT_PROVIDERS


def build_cli_prompt(issue: str) -> str:
    """The kickoff prompt handed to the agent CLI for a task."""
    return f"# Issue\n\n{issue}{_ISSUE_SUFFIX}"


def _subscription_env(extra: dict[str, str] | None = None) -> dict[str, str]:
    """Minimal environment for a subscription CLI process.

    Provider API keys and unrelated shell secrets are deliberately absent. The
    CLI can still find its executable and cached browser/keychain login through
    ``PATH`` and ``HOME``.  Test adapters may add explicit non-secret values via
    ``extra``.
    """
    env = {
        key: value
        for key, value in os.environ.items()
        if key in _SAFE_ENV_KEYS or key.startswith("LC_")
    }
    # PATH leaks this checkout's location (.venv/bin sits under the repo), and
    # a live grok run was observed extracting the prefix and running
    # `find <repo> -name cargo` over it. Scrub repo-rooted entries: the CLIs
    # find their own binaries through the remaining entries.
    repo_root = str(Path(__file__).resolve().parents[2])
    if "PATH" in env:
        env["PATH"] = os.pathsep.join(
            p for p in env["PATH"].split(os.pathsep) if p and not p.startswith(repo_root)
        )
    env["DISABLE_AUTOUPDATER"] = "1"
    # Host-run agents must not touch the system Python. A live ZCode run on
    # oss-aiohttp-upgrade-deferred (Aug 2026) ran `pip install -e .` in its
    # workspace, hit Homebrew's python3.14, and left a dangling editable
    # install that broke the host pytest once the workspace was cleaned.
    # PIP_REQUIRE_VIRTUALENV makes pip refuse any install outside a venv the
    # agent creates itself; PYTHONNOUSERSITE keeps user site-packages out of
    # reach as the fallback target.
    env["PIP_REQUIRE_VIRTUALENV"] = "1"
    env["PYTHONNOUSERSITE"] = "1"
    if extra:
        env.update(extra)
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


def _version(executable: str) -> str | None:
    if shutil.which(executable) is None:
        return None
    try:
        proc = subprocess.run(
            [executable, "--version"],
            capture_output=True,
            text=True,
            timeout=10,
            env=_subscription_env(),
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    text = (proc.stdout or proc.stderr).strip()
    return text.splitlines()[0] if text else None


def _claude_preflight(claude_bin: str = "claude") -> HarnessPreflight:
    version = _version(claude_bin)
    if version is None:
        return HarnessPreflight(
            harness="claude-code",
            available=False,
            version=None,
            authenticated=False,
            auth_mode=None,
            detail=f"{claude_bin!r} not found on PATH",
        )
    try:
        proc = subprocess.run(
            [claude_bin, "auth", "status", "--json"],
            capture_output=True,
            text=True,
            timeout=15,
            env=_subscription_env(),
            check=False,
        )
        body = json.loads(proc.stdout) if proc.stdout.strip() else {}
    except (OSError, subprocess.TimeoutExpired, json.JSONDecodeError) as exc:
        return HarnessPreflight(
            harness="claude-code",
            available=True,
            version=version,
            authenticated=False,
            auth_mode=None,
            detail=f"could not read Claude authentication status: {exc}",
        )
    logged_in = bool(body.get("loggedIn"))
    auth_method = str(body.get("authMethod") or "").lower()
    subscription = logged_in and auth_method == "claude.ai"
    return HarnessPreflight(
        harness="claude-code",
        available=True,
        version=version,
        authenticated=logged_in,
        auth_mode="subscription" if subscription else (auth_method or None),
        plan_name=str(body.get("subscriptionType") or "") or None,
        detail=None if subscription else "Claude Code is not using a Claude subscription login",
    )


def _codex_preflight(codex_bin: str = "codex") -> HarnessPreflight:
    version = _version(codex_bin)
    if version is None:
        return HarnessPreflight(
            harness="codex",
            available=False,
            version=None,
            authenticated=False,
            auth_mode=None,
            detail=f"{codex_bin!r} not found on PATH",
        )
    try:
        proc = subprocess.run(
            [codex_bin, "login", "status"],
            capture_output=True,
            text=True,
            timeout=15,
            env=_subscription_env(),
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return HarnessPreflight(
            harness="codex",
            available=True,
            version=version,
            authenticated=False,
            auth_mode=None,
            detail=f"could not read Codex authentication status: {exc}",
        )
    status = f"{proc.stdout}\n{proc.stderr}".strip().lower()
    authenticated = proc.returncode == 0 and "not logged in" not in status
    subscription = authenticated and "chatgpt" in status
    api_key = authenticated and ("api key" in status or "api-key" in status)
    return HarnessPreflight(
        harness="codex",
        available=True,
        version=version,
        authenticated=authenticated,
        auth_mode="subscription" if subscription else ("api" if api_key else None),
        detail=None if subscription else "Codex is not using a ChatGPT subscription login",
    )


def _cursor_preflight(cursor_bin: str = "cursor-agent") -> HarnessPreflight:
    version = _version(cursor_bin)
    if version is None:
        return HarnessPreflight(
            harness="cursor",
            available=False,
            version=None,
            authenticated=False,
            auth_mode=None,
            detail=f"{cursor_bin!r} not found on PATH",
        )
    if os.environ.get("CURSOR_API_KEY"):
        # API-key auth bills xAI/OpenAI-style metered usage, not the Cursor
        # plan; fail closed exactly like a signed-out Claude Code or Codex.
        return HarnessPreflight(
            harness="cursor",
            available=True,
            version=version,
            authenticated=True,
            auth_mode="api-key",
            detail="CURSOR_API_KEY is set; unset it to bill the Cursor subscription",
        )
    try:
        proc = subprocess.run(
            [cursor_bin, "status"],
            capture_output=True,
            text=True,
            timeout=15,
            env=_subscription_env(),
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return HarnessPreflight(
            harness="cursor",
            available=True,
            version=version,
            authenticated=False,
            auth_mode=None,
            detail=f"`{cursor_bin} status` failed: {exc}",
        )
    status_text = (proc.stdout or "") + (proc.stderr or "")
    if re.search(r"not logged in", status_text, re.I):
        return HarnessPreflight(
            harness="cursor",
            available=True,
            version=version,
            authenticated=False,
            auth_mode=None,
            detail=f"signed out; run `{cursor_bin} login`",
        )
    plan_match = re.search(r"(?:plan|membership)\s*[:=]?\s*(\S[^\n]*)", status_text, re.I)
    return HarnessPreflight(
        harness="cursor",
        available=True,
        version=version,
        authenticated=True,
        auth_mode="subscription",
        plan_name=plan_match.group(1).strip() if plan_match else None,
    )


def run_cursor_task(  # noqa: PLR0912, PLR0915, linear stream-parse loop
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
    effort: str | None = None,
    cursor_bin: str = "cursor-agent",
    env_overrides: dict[str, str] | None = None,
    preflight: HarnessPreflight | None = None,
) -> CliAgentOutcome:
    """Run one task through ``cursor-agent -p`` billed to the Cursor account.

    Cursor's stream-json reports no token usage or cost, so the outcome carries
    zero token counts and the economics receipt honestly records the
    API-equivalent value as unavailable, Cursor's own dashboard is the only
    ledger for what a run consumed. ``max_turns`` cannot be forwarded (no such
    flag) and ``max_run_cost`` cannot be enforced live (no streamed usage).
    """
    del priced_spec, max_turns
    workspace = workspace.resolve()
    if timeout_s is not None and timeout_s <= 0:
        raise ProviderError("run budget exhausted before CLI agent start")
    if max_run_cost is not None:
        raise ProviderError(
            "cursor-agent reports no usage stream, so --max-run-cost cannot be "
            "enforced; use a wall-clock --timeout for subscription runs"
        )

    checked = preflight or _cursor_preflight(cursor_bin)
    _require_subscription(checked)
    if not network:
        # Web parity with the loop (which has no web tools) and with
        # claude-code's --disallowedTools. Without this, v3's post-cutoff
        # decontamination is defeated at runtime: tasks derive from public
        # merged PRs, and an unrestricted agent does fetch the exact upstream
        # fix (observed in Harness Study No. 01).
        #
        # Verified against cursor-agent 2026.08, and the mechanism is fussy:
        #   * --force approves every permission query, INCLUDING denied ones,
        #     so a deny list under --force is silently useless.
        #   * --trust honours denies, but with no allow list it also rejects
        #     shell calls, which the benchmark needs for running tests.
        # So: --trust plus an explicit allow list for the work tools, and a
        # deny list for web. Both keys are required by the config schema.
        cursor_dir = workspace / ".cursor"
        cursor_dir.mkdir(exist_ok=True)
        (cursor_dir / "cli.json").write_text(
            json.dumps({"permissions": _CURSOR_WEB_DENIED_PERMISSIONS}, indent=1),
            encoding="utf-8",
        )
    # Cursor's per-model bracket syntax carries effort when the loop resolved a
    # supported level (e.g. "grok-4.6[effort=high]").
    model_arg = f"{model}[effort={effort}]" if effort else model
    cmd = [
        cursor_bin,
        "-p",
        prompt,
        "--output-format",
        "stream-json",
        "--model",
        model_arg,
        "--sandbox",
        "enabled",
        # --force would override the web deny; --trust honours it and the
        # allow list above restores the tools a run needs.
        "--force" if network else "--trust",
    ]

    collector.record(
        "cli_agent_start",
        {
            "harness": "cursor",
            "argv": [cmd[0], "-p", "<prompt omitted>", *cmd[3:]],
            "harness_version": checked.version,
        },
    )
    try:
        proc = subprocess.Popen(
            cmd,
            cwd=workspace,
            env=_subscription_env(env_overrides),
            text=True,
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except FileNotFoundError as exc:
        raise ProviderError(
            f"{cursor_bin!r} not found on PATH; install the Cursor CLI and run `{cursor_bin} login`"
        ) from exc

    stderr_chunks: list[str] = []

    def _drain_stderr() -> None:
        assert proc.stderr is not None
        for chunk in proc.stderr:
            stderr_chunks.append(chunk)

    stderr_thread = threading.Thread(target=_drain_stderr, daemon=True)
    stderr_thread.start()

    outcome = CliAgentOutcome(
        harness="cursor",
        execution_boundary=(
            "host-workspace; cursor-sandbox=enabled; "
            + ("force-allow; web-allowed" if network else "trust+allowlist; web-denied")
        ),
        requested_model=model,
        harness_version=checked.version,
        auth_method=checked.auth_mode,
        plan_name=checked.plan_name,
    )
    killed = {"timeout": False}

    def _kill_on_timeout() -> None:
        killed["timeout"] = True
        proc.kill()

    watchdog: threading.Timer | None = None
    if timeout_s is not None:
        watchdog = threading.Timer(timeout_s, _kill_on_timeout)
        watchdog.daemon = True
        watchdog.start()

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
                reported_model = event.get("model")
                if reported_model:
                    outcome.reported_model = str(reported_model)
                    outcome.model_identity_confidence = "cli-reported"
                collector.record(
                    "cli_agent_init",
                    {
                        "session_id": outcome.session_id,
                        "model": reported_model,
                        "harness_version": outcome.harness_version,
                    },
                )
            elif etype == "assistant":
                msg = event.get("message") or {}
                collector.record("llm_response", _assistant_trace_data(msg))
            elif etype == "tool_call":
                collector.record(
                    "tool_observation" if event.get("subtype") == "completed" else "tool_call",
                    {
                        "tool": event.get("call_id", ""),
                        "result": event.get("result"),
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
        if killed["timeout"]:
            # Partial work still counts; the caller diffs and verifies it.
            return outcome
        tail = "".join(stderr_chunks)[-500:].strip()
        detail = tail or "no stderr"
        if _LIMIT_PATTERN.search(detail):
            raise SubscriptionQuotaError(
                "cursor usage limit hit, rerun after topping up credits "
                f"(use --only-missing to resume): {detail[:300]}"
            )
        raise ProviderError(
            f"cursor-agent exited without a result (exit {proc.returncode}): {detail}"
        )

    outcome.subtype = result_msg.get("subtype")
    outcome.session_id = result_msg.get("session_id") or outcome.session_id
    result_text = str(result_msg.get("result") or "")
    if result_msg.get("is_error") or outcome.subtype != "success":
        if _LIMIT_PATTERN.search(result_text):
            raise SubscriptionQuotaError(
                "cursor usage limit hit, rerun after topping up credits "
                f"(use --only-missing to resume): {result_text[:300]}"
            )
        raise ProviderError(f"cursor-agent run failed ({outcome.subtype}): {result_text[:300]}")

    outcome.finished = True
    collector.record("cli_agent_result", outcome.summary())
    return outcome


def _grok_build_preflight(grok_bin: str = "grok") -> HarnessPreflight:
    version = _version(grok_bin)
    if version is None:
        return HarnessPreflight(
            harness="grok-build",
            available=False,
            version=None,
            authenticated=False,
            auth_mode=None,
            detail=f"{grok_bin!r} not found on PATH",
        )
    if os.environ.get("XAI_API_KEY"):
        # API-key auth bills metered console.x.ai usage, not the Grok plan;
        # fail closed exactly like CURSOR_API_KEY on the Cursor adapter.
        return HarnessPreflight(
            harness="grok-build",
            available=True,
            version=version,
            authenticated=True,
            auth_mode="api-key",
            detail="XAI_API_KEY is set; unset it to bill the Grok subscription",
        )
    try:
        proc = subprocess.run(
            [grok_bin, "models"],
            capture_output=True,
            text=True,
            timeout=30,
            env=_subscription_env(),
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return HarnessPreflight(
            harness="grok-build",
            available=True,
            version=version,
            authenticated=False,
            auth_mode=None,
            detail=f"`{grok_bin} models` failed: {exc}",
        )
    status_text = (proc.stdout or "") + (proc.stderr or "")
    if re.search(r"not authenticated|not logged in", status_text, re.I):
        return HarnessPreflight(
            harness="grok-build",
            available=True,
            version=version,
            authenticated=False,
            auth_mode=None,
            detail=f"signed out; run `{grok_bin} login`",
        )
    login_match = re.search(r"logged in with\s+(\S+)", status_text, re.I)
    return HarnessPreflight(
        harness="grok-build",
        available=True,
        version=version,
        authenticated=True,
        auth_mode="subscription",
        plan_name=login_match.group(1).strip().rstrip(".") if login_match else None,
    )


# Tools Grok Build must not have on a decontaminated suite. Removal via
# --disallowed-tools is airtight (the model cannot call a tool that does not
# exist), at the cost of the "refused attempts" telemetry the Cursor study
# had, a deliberate trade after Harness Study No. 01 showed how fussy
# permission-layer denies are. --deny WebFetch rides along as a second layer.
_GROK_WEB_TOOLS = "web_search,web_fetch"


def _grok_session_dir(session_id: str) -> Path | None:
    """Locate ``~/.grok/sessions/**/<session_id>`` for the trace artifacts."""
    home = Path(os.environ.get("GROK_HOME") or Path.home() / ".grok")
    root = home / "sessions"
    if not root.is_dir():
        return None
    for candidate in root.rglob(session_id):
        if candidate.is_dir() and (candidate / "summary.json").exists():
            return candidate
    return None


def _harvest_grok_trace(  # noqa: PLR0912, linear artifact-copy + fold loop
    session_dir: Path,
    run_dir: Path,
    stream_f: Any,
    outcome: CliAgentOutcome,
) -> None:
    """Copy session artifacts into the run dir and fold them into the stream.

    Grok's live streaming-json is text/thought/end only; every tool call
    (with ``toolCallId`` and ``rawInput``: the audit substrate) lives in the
    session's ``updates.jsonl``. Appending those records to the stream log is
    what lets ``run_audit`` see grok runs at all.
    """
    dest = run_dir / "grok-session"
    dest.mkdir(exist_ok=True)
    max_tokens = 0
    for name in ("summary.json", "updates.jsonl", "events.jsonl"):
        src = session_dir / name
        if not src.exists():
            continue
        shutil.copy2(src, dest / name)
    updates = session_dir / "updates.jsonl"
    if updates.exists() and stream_f is not None:
        with updates.open(encoding="utf-8", errors="replace") as f:
            for raw_line in f:
                stripped = raw_line.strip()
                if not stripped:
                    continue
                try:
                    record = json.loads(stripped)
                except json.JSONDecodeError:
                    continue
                meta = (record.get("params") or {}).get("_meta") or {}
                total = meta.get("totalTokens")
                if isinstance(total, int):
                    max_tokens = max(max_tokens, total)
                json.dump(sanitize(record), stream_f)
                stream_f.write("\n")
    if max_tokens:
        outcome.cli_total_tokens = max_tokens
    summary_path = session_dir / "summary.json"
    if summary_path.exists():
        try:
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return
        reported = summary.get("current_model_id")
        if reported:
            outcome.reported_model = str(reported)
            outcome.model_identity_confidence = "cli-reported"
        if summary.get("reasoning_effort"):
            outcome.reported_effort = str(summary["reasoning_effort"])
        if summary.get("sandbox_profile"):
            outcome.sandbox_profile = str(summary["sandbox_profile"])


def run_grok_build_task(  # noqa: PLR0912, PLR0915, linear stream-parse loop
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
    effort: str | None = None,
    grok_bin: str = "grok",
    env_overrides: dict[str, str] | None = None,
    preflight: HarnessPreflight | None = None,
) -> CliAgentOutcome:
    """Run one task through ``grok -p`` billed to the Grok subscription.

    Verified against grok 0.2.69 and 1.0.5 (alpha). On 0.2.69 the flag
    surface had a trap: ``--effort`` parsed and was silently IGNORED for
    reasoning (the session kept the default "high"); 1.0.5 makes it an alias
    of ``--reasoning-effort``, which is the flag this adapter always sends
    (accepted: none/minimal/low/medium/high/xhigh, confirmed by reading
    back ``reasoning_effort`` from the session summary on both versions).
    On 0.2.69 the live stream carried no tool calls or usage; 1.0.5 streams
    ``tool_call``/``tool_call_update``/``usage`` events and a final ``end``
    event with the full split (input/output/cache-read/reasoning tokens,
    ``num_turns``, and the CLI's own ``total_cost_usd``), so token receipts
    and a live API-equivalent cost cap both work. The session trace is still
    harvested post-run as the audit substrate, the session id is chosen up
    front with ``-s`` so a timeout can still find its trace.

    ``--sandbox strict`` is Seatbelt/Landlock-enforced: reads outside the
    workspace are kernel-denied, which contains the filesystem channel harder
    than Cursor's sandbox did. On macOS the sandbox does NOT block child
    network (curl in a shell still works); web tools are removed instead and
    the audit remains the check on the rest.
    """
    workspace = workspace.resolve()
    if timeout_s is not None and timeout_s <= 0:
        raise ProviderError("run budget exhausted before CLI agent start")

    checked = preflight or _grok_build_preflight(grok_bin)
    _require_subscription(checked)

    session_id = str(uuid.uuid4())
    # Custom kernel sandbox: toolchain parity with other harnesses (read
    # everywhere, write CWD, `strict` also denied ~/.cargo and homebrew,
    # crippling non-Python tasks), plus a kernel deny on this checkout so the
    # answer keys are unreadable even if the agent learns the repo path.
    # Grok fails closed if the profile cannot be applied.
    repo_root = str(Path(__file__).resolve().parents[2])
    grok_dir = workspace / ".grok"
    grok_dir.mkdir(exist_ok=True)
    (grok_dir / "sandbox.toml").write_text(
        f'[profiles.vulcanbench]\nextends = "workspace"\ndeny = ["{repo_root}"]\n',
        encoding="utf-8",
    )
    cmd = [
        grok_bin,
        "-p",
        prompt,
        "--cwd",
        str(workspace),
        "--output-format",
        "streaming-json",
        "--always-approve",
        "--no-auto-update",
        "--sandbox",
        "vulcanbench",
        "--session-id",
        session_id,
        "-m",
        model,
        "--max-turns",
        str(max_turns),
    ]
    if effort:
        # NOT --effort; see docstring.
        cmd += ["--reasoning-effort", effort]
    if not network:
        cmd += ["--disallowed-tools", _GROK_WEB_TOOLS, "--deny", "WebFetch"]

    collector.record(
        "cli_agent_start",
        {
            "harness": "grok-build",
            "argv": [cmd[0], "-p", "<prompt omitted>", *cmd[3:]],
            "harness_version": checked.version,
        },
    )
    env = _subscription_env(env_overrides)
    env["GROK_DISABLE_AUTOUPDATER"] = "1"
    # Cross-session memory would let run N+1 remember run N's task,
    # repeat-to-repeat contamination inside one sweep. Grok 1.0 dropped the
    # --no-memory flag; GROK_MEMORY=0 force-disables regardless of the
    # user's config.toml, which a flag never did.
    env["GROK_MEMORY"] = "0"
    try:
        proc = subprocess.Popen(
            cmd,
            cwd=workspace,
            env=env,
            text=True,
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except FileNotFoundError as exc:
        raise ProviderError(
            f"{grok_bin!r} not found on PATH; install Grok Build and run `{grok_bin} login`"
        ) from exc

    stderr_chunks: list[str] = []

    def _drain_stderr() -> None:
        assert proc.stderr is not None
        for chunk in proc.stderr:
            stderr_chunks.append(chunk)

    stderr_thread = threading.Thread(target=_drain_stderr, daemon=True)
    stderr_thread.start()

    outcome = CliAgentOutcome(
        harness="grok-build",
        execution_boundary=(
            "host-workspace; grok-sandbox=vulcanbench "
            "(workspace-writes + kernel-denied repo reads); "
            + ("web-allowed" if network else "web-tools-removed")
        ),
        requested_model=model,
        harness_version=checked.version,
        auth_method=checked.auth_mode,
        plan_name=checked.plan_name,
        session_id=session_id,
    )
    killed = {"timeout": False, "cost": False}

    def _kill_on_timeout() -> None:
        killed["timeout"] = True
        proc.kill()

    watchdog: threading.Timer | None = None
    if timeout_s is not None:
        watchdog = threading.Timer(timeout_s, _kill_on_timeout)
        watchdog.daemon = True
        watchdog.start()

    end_msg: dict[str, Any] | None = None
    error_msg: str | None = None
    text_parts: list[str] = []
    thought_chars = 0
    # Running totals from per-call ``usage`` events (grok >= 1.0). Grok's
    # output_tokens already includes reasoning (verified: per-call outputs sum
    # to the end event's output_tokens, and end total = input + cache reads +
    # output), no completion_excludes_reasoning fold here, unlike raw xAI.
    acc = {"prompt": 0, "completion": 0, "cached": 0, "reasoning": 0}
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
            etype = event.get("type")
            # text/thought chunks are word-level; logging each would bloat the
            # stream log ~100x for no audit value (tool calls arrive later
            # from the trace). Aggregate them and log everything else.
            if etype == "text":
                text_parts.append(str(event.get("data") or ""))
                continue
            if etype == "thought":
                thought_chars += len(str(event.get("data") or ""))
                continue
            if stream_f:
                json.dump(sanitize(event), stream_f)
                stream_f.write("\n")
            if etype == "end":
                end_msg = event
            elif etype == "error":
                error_msg = str(event.get("message") or "")
            elif etype == "max_turns_reached":
                outcome.subtype = "max_turns"
            elif etype == "usage":
                usage = event.get("usage") or {}
                acc["prompt"] += (
                    int(usage.get("input_tokens", 0) or 0)
                    + int(usage.get("cache_read_input_tokens", 0) or 0)
                    + int(usage.get("cache_creation_input_tokens", 0) or 0)
                )
                acc["completion"] += int(usage.get("output_tokens", 0) or 0)
                acc["cached"] += int(usage.get("cache_read_input_tokens", 0) or 0)
                acc["reasoning"] += int(usage.get("reasoning_tokens", 0) or 0)
                if max_run_cost is not None:
                    run_cost = cost_usd(
                        priced_spec,
                        acc["prompt"],
                        acc["completion"],
                        cached_input_tokens=acc["cached"],
                    )
                    if run_cost is not None and run_cost >= max_run_cost:
                        collector.record(
                            "cost_cap_exceeded",
                            {"cost_usd": run_cost, "max_run_cost": max_run_cost},
                        )
                        outcome.cost_capped = True
                        killed["cost"] = True
                        proc.kill()
    finally:
        if watchdog is not None:
            watchdog.cancel()
        proc.wait()
        stderr_thread.join(timeout=5)
        if text_parts or thought_chars:
            collector.record(
                "llm_response",
                {
                    "text": "".join(text_parts)[:4000],
                    "thought_chars": thought_chars,
                },
            )
        session_dir = _grok_session_dir(session_id)
        if session_dir is not None and stream_log_path is not None:
            run_dir = stream_log_path.parent
            _harvest_grok_trace(session_dir, run_dir, stream_f, outcome)
        if stream_f:
            stream_f.close()

    outcome.timed_out = killed["timeout"]
    outcome.prompt_tokens = acc["prompt"]
    outcome.completion_tokens = acc["completion"]
    outcome.cached_input_tokens = acc["cached"]
    outcome.reasoning_output_tokens = acc["reasoning"]

    if end_msg is None:
        if killed["timeout"] or killed["cost"]:
            # Partial work still counts; the caller diffs and verifies it.
            return outcome
        tail = (error_msg or "").strip() or "".join(stderr_chunks)[-500:].strip() or "no stderr"
        if _LIMIT_PATTERN.search(tail):
            raise SubscriptionQuotaError(
                "grok usage limit hit, rerun after the window resets "
                f"(use --only-missing to resume): {tail[:300]}"
            )
        raise ProviderError(f"grok exited without an end event (exit {proc.returncode}): {tail}")

    outcome.subtype = outcome.subtype or str(end_msg.get("stopReason") or "")
    outcome.session_id = end_msg.get("sessionId") or outcome.session_id
    end_usage = end_msg.get("usage") or {}
    if end_usage:
        # The end event's totals are authoritative over per-call accumulation.
        cache_read = int(end_usage.get("cache_read_input_tokens", 0) or 0)
        outcome.prompt_tokens = (
            int(end_usage.get("input_tokens", 0) or 0)
            + cache_read
            + int(end_usage.get("cache_creation_input_tokens", 0) or 0)
        )
        outcome.completion_tokens = int(end_usage.get("output_tokens", 0) or 0)
        outcome.cached_input_tokens = cache_read
        outcome.reasoning_output_tokens = int(end_usage.get("reasoning_tokens", 0) or 0)
        total = end_usage.get("total_tokens")
        if isinstance(total, int):
            outcome.cli_total_tokens = total
    if end_msg.get("num_turns") is not None:
        outcome.num_turns = int(end_msg["num_turns"])
    if end_msg.get("total_cost_usd") is not None:
        outcome.cli_reported_cost_usd = float(end_msg["total_cost_usd"])
    outcome.finished = True
    collector.record("cli_agent_result", outcome.summary())
    return outcome


# ---------------------------------------------------------------------------
# ZCode (Z.ai's coding harness for GLM)
# ---------------------------------------------------------------------------

#: The ZCode tool names that reach the public web. ``WebFetch``/``WebSearch``
#: are the built-in tool ids; ``web_search`` is the catalog alias the runtime
#: also registers. Removed via ``--disallowed-tools`` (the runtime's headless
#: denylist) and mirrored into the per-run project config so subagents inherit
#: the deny. The Browser Use plugin (a headless Chromium the npm launcher
#: enables by default for ``--prompt`` sessions) is a second web channel and is
#: switched off through the same config.
_ZCODE_WEB_TOOLS = ("WebFetch", "WebSearch", "web_search")
_ZCODE_BROWSER_PLUGIN_ID = "browser-use@zcode-plugins-official"

#: ZCode ``model_usage.query_source`` values that are NOT agent turns: title
#: generation, memory synthesis, internal tool model calls, and web processing.
#: These run on the lite model, so they must not vote for the run's reported
#: model/effort (agent turns are ``main_turn``). A denylist keeps a future
#: turn-source name counted rather than silently dropped.
_ZCODE_AUX_QUERY_SOURCES = frozenset(
    {
        "session_title_generation",
        "memory_synthesize",
        "tool_internal_model_call",
        "web_fetch_processing",
        "web_search",
        "workspace_generate_text",
        "git_commit_message",
    }
)

#: Z.ai / BigModel endpoints that bill a GLM Coding Plan rather than metered
#: API usage. A key configured against any other base URL is a pay-as-you-go
#: key, and the preflight fails closed on it exactly as it does for
#: ``XAI_API_KEY`` on Grok Build.
_ZCODE_PLAN_ENDPOINT_MARKERS = (
    "api.z.ai/api/anthropic",
    "api.z.ai/api/coding/",
    "open.bigmodel.cn/api/anthropic",
    "open.bigmodel.cn/api/coding/",
)

# Coding Plan exhaustion surfaces as provider errors from Z.ai. The process
# often exits 1 with only a generic stack on stderr, while the real cause
# ("[1308][Usage limit reached for 5 hour...]" or "[1302][Rate limit...]")
# lands in the harvested session's model_usage.error_message, so both the
# stderr tail and the session rows are scanned. Codes: 1302 rate limit,
# 1308 5-hour/weekly window limit.
_ZCODE_LIMIT_PATTERN = re.compile(
    r"usage limit|rate limit|limit reached|limit will reset|quota|"
    r"insufficient balance|too many requests|\b429\b|\[130[28]\]",
    re.I,
)


def _zcode_home() -> Path:
    """Root of ZCode's CLI state (``~/.zcode`` unless relocated)."""
    override = os.environ.get("ZCODE_STORAGE_DIR") or os.environ.get("ZCODE_HOME")
    return Path(override).expanduser() if override else Path.home() / ".zcode"


def _zcode_user_config_path() -> Path:
    return _zcode_home() / "cli" / "config.json"


def _zcode_credentials_path() -> Path:
    """Shared Z.ai login store (``~/.zcode/v2/credentials.json``)."""
    base = os.environ.get("ZCODE_DATA_BASE_DIR")
    root = Path(base).expanduser() if base else Path.home()
    return root / ".zcode" / "v2" / "credentials.json"


def _zcode_session_db_path(user_config: dict[str, Any] | None = None) -> Path:
    storage = user_config.get("storage") if isinstance(user_config, dict) else None
    configured = storage.get("sessionDbPath") if isinstance(storage, dict) else None
    if isinstance(configured, str) and configured.strip():
        return Path(configured).expanduser()
    return _zcode_home() / "cli" / "db" / "db.sqlite"


def _read_json_file(path: Path) -> dict[str, Any] | None:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def _zcode_version(zcode_bin: str) -> str | None:
    """Both lines of ``zcode --version`` (launcher and runtime), joined."""
    if shutil.which(zcode_bin) is None:
        return None
    try:
        proc = subprocess.run(
            [zcode_bin, "--version"],
            capture_output=True,
            text=True,
            timeout=20,
            env=_subscription_env(),
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    lines = [ln.strip() for ln in (proc.stdout or proc.stderr).splitlines() if ln.strip()]
    return "; ".join(lines[:2]) if lines else None


def _zcode_preflight(zcode_bin: str = "zcode") -> HarnessPreflight:
    """Readiness receipt for ZCode without launching a model run.

    Authentication is read from ZCode's own stores, presence only, never the
    values: the shared OAuth credential file (``zcode login``, bills the GLM
    Coding Plan) or a Coding Plan API key configured in the user config against
    a plan endpoint. A key against any other endpoint is metered API billing
    and fails closed, matching the XAI_API_KEY / CURSOR_API_KEY rule.
    """
    version = _zcode_version(zcode_bin)
    if version is None:
        return HarnessPreflight(
            harness="zcode",
            available=False,
            version=None,
            authenticated=False,
            auth_mode=None,
            detail=(
                f"{zcode_bin!r} not found on PATH; install with "
                "`npm install -g zcode-app-cli@latest` (Node >= 22.19) and run `zcode login`"
            ),
        )
    creds = _read_json_file(_zcode_credentials_path()) or {}
    if creds.get("oauth:zai:access_token") or creds.get("oauth:zai:refresh_token"):
        return HarnessPreflight(
            harness="zcode",
            available=True,
            version=version,
            authenticated=True,
            auth_mode="subscription",
            plan_name=_zcode_plan_name(creds),
            detail="Z.ai OAuth login (GLM Coding Plan)",
        )
    config = _read_json_file(_zcode_user_config_path()) or {}
    raw_providers = config.get("provider")
    providers: dict[str, Any] = raw_providers if isinstance(raw_providers, dict) else {}
    model_block = config.get("model")
    main_model = str(model_block.get("main") or "") if isinstance(model_block, dict) else ""
    provider_id = main_model.partition("/")[0] if "/" in main_model else ""
    candidates = [providers.get(provider_id)] if provider_id else list(providers.values())
    for entry in candidates:
        if not isinstance(entry, dict):
            continue
        raw_options = entry.get("options")
        options: dict[str, Any] = raw_options if isinstance(raw_options, dict) else {}
        if not str(options.get("apiKey") or "").strip():
            continue
        base_url = str(options.get("baseURL") or "")
        if any(marker in base_url for marker in _ZCODE_PLAN_ENDPOINT_MARKERS):
            return HarnessPreflight(
                harness="zcode",
                available=True,
                version=version,
                authenticated=True,
                auth_mode="subscription",
                detail=f"Coding Plan API key in {_zcode_user_config_path()} ({base_url})",
            )
        return HarnessPreflight(
            harness="zcode",
            available=True,
            version=version,
            authenticated=True,
            auth_mode="api-key",
            detail=(
                f"API key configured against {base_url or 'a custom endpoint'}: metered "
                "billing, not the GLM Coding Plan; run `zcode login` for a subscription run"
            ),
        )
    return HarnessPreflight(
        harness="zcode",
        available=True,
        version=version,
        authenticated=False,
        auth_mode=None,
        detail=f"signed out; run `{zcode_bin} login` (Z.ai OAuth) to bill the GLM Coding Plan",
    )


def _zcode_plan_name(creds: dict[str, Any]) -> str | None:
    """Non-secret plan label from the OAuth user-info blob, if it carries one."""
    info = creds.get("oauth:zai:user_info")
    if isinstance(info, str):
        try:
            info = json.loads(info)
        except json.JSONDecodeError:
            return None
    if not isinstance(info, dict):
        return None
    for key in ("plan", "planName", "plan_name", "package", "packageName", "subscription"):
        value = info.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
        if isinstance(value, dict):
            label = value.get("name") or value.get("title")
            if isinstance(label, str) and label.strip():
                return label.strip()
    return None


def _zcode_model_ref(model: str, user_config: dict[str, Any] | None) -> tuple[str, str]:
    """Resolve ``(provider_id, "<provider>/<model>")`` for a bare or qualified model.

    The provider id is taken from the user config's own default model
    (``model.main``) when the caller passed a bare model, so we pin exactly the
    provider ZCode is already authenticated against (e.g. ``zai``) rather than
    guessing. A slash-qualified model is honored as-is.
    """
    if "/" in model:
        provider_id = model.split("/", 1)[0]
        return provider_id, model
    default_main = ""
    if isinstance(user_config, dict):
        model_block = user_config.get("model")
        if isinstance(model_block, dict):
            default_main = str(model_block.get("main") or "")
    provider_id = default_main.split("/", 1)[0] if "/" in default_main else "zai"
    return provider_id, f"{provider_id}/{model}"


def _zcode_project_config(
    model: str,
    *,
    network: bool,
    effort: str | None,
    user_config: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Per-run ``<workspace>/.zcode/config.json`` (merged over the user config).

    Only the keys a clean benchmark must pin: the model, the thought level
    (effort), permission mode, no cross-session memory (repeat N+1 must not
    remember repeat N's task), and the web denies when web is off. Everything
    else (skills, MCP, subagents, the product's default lite model) stays at
    ZCode's defaults, because the column measures model + product harness.

    The referenced provider block is copied verbatim from the user config into
    the project scope. This is required, not cosmetic: setting ``model.main`` in
    a project config makes ZCode resolve the provider in project scope, and
    without the provider's ``baseURL`` (and credential) there it fails with
    "Model provider <id> is missing baseURL" or an auth error. The copied block
    carries the credential, so :func:`_redact_project_config` must be used
    before logging this config anywhere.
    """
    if user_config is None:
        user_config = _read_json_file(_zcode_user_config_path())
    provider_id, model_ref = _zcode_model_ref(model, user_config)
    config: dict[str, Any] = {
        "model": {"main": model_ref},
        "permission": {"mode": "yolo"},
        "features": {"memory": False},
        "memory": {"use": False, "write": False, "autoConsolidate": False},
    }
    providers = user_config.get("provider") if isinstance(user_config, dict) else None
    if isinstance(providers, dict) and isinstance(providers.get(provider_id), dict):
        config["provider"] = {provider_id: providers[provider_id]}
    if effort:
        config["modelCatalog"] = {"overrides": {model_ref: {"reasoning": {"defaultLevel": effort}}}}
    if not network:
        config["permission"]["disallowedTools"] = list(_ZCODE_WEB_TOOLS)
        config["plugins"] = {"enabledPlugins": {_ZCODE_BROWSER_PLUGIN_ID: False}}
    return config


def _redact_project_config(config: dict[str, Any]) -> dict[str, Any]:
    """Deep copy with any provider ``options.apiKey`` masked, for safe logging."""
    redacted: dict[str, Any] = json.loads(json.dumps(config))
    providers = redacted.get("provider")
    if isinstance(providers, dict):
        for entry in providers.values():
            options = entry.get("options") if isinstance(entry, dict) else None
            if isinstance(options, dict) and options.get("apiKey"):
                options["apiKey"] = "***"
    return redacted


def _zcode_find_session(
    db_path: Path, workspace: Path, started_ms: int
) -> tuple[str | None, list[str]]:
    """Locate the session ZCode created for this workspace (plus subagent children)."""
    if not db_path.exists():
        return None, []
    candidates = {str(workspace), os.path.realpath(workspace)}
    try:
        con = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
        try:
            placeholders = ",".join("?" for _ in candidates)
            rows = con.execute(
                f"select id from session where directory in ({placeholders}) "
                "and parent_id is null and time_created >= ? "
                "order by time_created desc limit 1",
                (*candidates, started_ms),
            ).fetchall()
            if not rows:
                return None, []
            root = str(rows[0][0])
            children = [
                str(r[0])
                for r in con.execute(
                    "select id from session where parent_id = ? order by time_created",
                    (root,),
                ).fetchall()
            ]
            return root, children
        finally:
            con.close()
    except Exception:  # sqlite availability is environmental; treat as no session
        return None, []


def _harvest_zcode_session(  # noqa: PLR0912, PLR0915, linear copy + fold loop
    db_path: Path,
    session_id: str,
    child_ids: list[str],
    run_dir: Path,
    stream_f: Any,
    outcome: CliAgentOutcome,
) -> dict[str, int]:
    """Fold ZCode's sqlite session store into the run artifacts.

    ZCode's headless mode prints only the assistant's final text; every
    message, tool call (``part`` rows with ``callID``/``tool``/``state.input``,
    the audit substrate) and per-request token receipt (``model_usage``) lives
    in ``~/.zcode/cli/db/db.sqlite``. This copies the session's rows into
    ``<run_dir>/zcode-session/`` as JSONL, appends message/tool records to the
    stream log so ``run_audit`` can see them, and sums usage into the outcome.
    """
    totals = {"prompt": 0, "completion": 0, "cached": 0, "reasoning": 0, "requests": 0}
    dest = run_dir / "zcode-session"
    dest.mkdir(exist_ok=True)
    ids = [session_id, *child_ids]
    placeholders = ",".join("?" for _ in ids)
    con = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    con.row_factory = sqlite3.Row
    try:
        with (dest / "messages.jsonl").open("w", encoding="utf-8") as mf:
            rows = con.execute(
                f"select m.id, m.session_id, m.sequence, m.time_created, m.data "
                f"from message m where m.session_id in ({placeholders}) "
                "order by m.session_id, m.sequence, m.time_created",
                ids,
            ).fetchall()
            for row in rows:
                try:
                    data = json.loads(row["data"])
                except (TypeError, json.JSONDecodeError):
                    data = {"raw": str(row["data"])[:2000]}
                record = {
                    "type": "message",
                    "session_id": row["session_id"],
                    "message_id": row["id"],
                    "sequence": row["sequence"],
                    "time_created": row["time_created"],
                    "data": data,
                }
                json.dump(sanitize(record), mf)
                mf.write("\n")
                if stream_f is not None:
                    json.dump(sanitize(record), stream_f)
                    stream_f.write("\n")
                parts = con.execute(
                    "select id, sequence, time_created, data from part "
                    "where message_id = ? order by sequence, time_created",
                    (row["id"],),
                ).fetchall()
                for part in parts:
                    try:
                        pdata = json.loads(part["data"])
                    except (TypeError, json.JSONDecodeError):
                        pdata = {"raw": str(part["data"])[:2000]}
                    precord: dict[str, Any] = {
                        "type": "part",
                        "session_id": row["session_id"],
                        "message_id": row["id"],
                        "part_id": part["id"],
                        "data": pdata,
                    }
                    if isinstance(pdata, dict) and pdata.get("type") == "tool":
                        raw_state = pdata.get("state")
                        state: dict[str, Any] = raw_state if isinstance(raw_state, dict) else {}
                        # Normalized tool-call line: the audit keys on
                        # toolCallId and reads path/command args verbatim.
                        precord["tool_call"] = {
                            "toolCallId": pdata.get("callID") or pdata.get("callId"),
                            "name": pdata.get("tool"),
                            "status": state.get("status"),
                            "input": state.get("input"),
                        }
                    json.dump(sanitize(precord), mf)
                    mf.write("\n")
                    if stream_f is not None:
                        json.dump(sanitize(precord), stream_f)
                        stream_f.write("\n")
        models: dict[str, int] = {}
        variants: dict[str, int] = {}
        with (dest / "model_usage.jsonl").open("w", encoding="utf-8") as uf:
            usage_rows = con.execute(
                f"select * from model_usage where session_id in ({placeholders}) "
                "order by started_at",
                ids,
            ).fetchall()
            for u in usage_rows:
                urec = {k: u[k] for k in u.keys()}  # noqa: SIM118, sqlite3.Row
                json.dump(sanitize(urec), uf)
                uf.write("\n")
                if u["status"] == "cancelled":
                    continue
                totals["requests"] += 1
                cache_read = int(u["cache_read_input_tokens"] or 0)
                cache_write = int(u["cache_creation_input_tokens"] or 0)
                totals["prompt"] += int(u["input_tokens"] or 0) + cache_read + cache_write
                totals["completion"] += int(u["output_tokens"] or 0)
                totals["cached"] += cache_read
                totals["reasoning"] += int(u["reasoning_tokens"] or 0)
                # Attribute the run's model/effort from the root session's
                # agent turns. ZCode tags those ``query_source == "main_turn"``;
                # auxiliary calls (title/memory/tool-internal/web) run on the
                # lite model and must not vote. Denylist, not allowlist, so a
                # future turn source is still counted.
                if (
                    u["session_id"] == session_id
                    and (u["query_source"] or "") not in _ZCODE_AUX_QUERY_SOURCES
                ):
                    ref = f"{u['provider_id']}/{u['model_id']}"
                    models[ref] = models.get(ref, 0) + 1
                    if u["variant"]:
                        variants[str(u["variant"])] = variants.get(str(u["variant"]), 0) + 1
        for name in ("turn_usage", "tool_usage"):
            with (dest / f"{name}.jsonl").open("w", encoding="utf-8") as tf:
                for t in con.execute(
                    f"select * from {name} where session_id in ({placeholders})", ids
                ).fetchall():
                    json.dump(sanitize({k: t[k] for k in t.keys()}), tf)  # noqa: SIM118
                    tf.write("\n")
        if models:
            main_ref = max(models.items(), key=lambda kv: kv[1])[0]
            outcome.reported_model = main_ref
            outcome.model_identity_confidence = "cli-reported"
        if variants:
            outcome.reported_effort = max(variants.items(), key=lambda kv: kv[1])[0]
    finally:
        con.close()
    outcome.prompt_tokens = totals["prompt"]
    outcome.completion_tokens = totals["completion"]
    outcome.cached_input_tokens = totals["cached"]
    outcome.reasoning_output_tokens = totals["reasoning"]
    outcome.num_turns = totals["requests"] or None
    return totals


def _harvest_zcode_log(session_id: str, run_dir: Path) -> None:
    """Copy this session's lines from ZCode's daily JSONL log into the run dir."""
    log_dir = _zcode_home() / "cli" / "log"
    if not log_dir.is_dir():
        return
    dest = run_dir / "zcode-session"
    dest.mkdir(exist_ok=True)
    out_path = dest / "log.jsonl"
    with out_path.open("w", encoding="utf-8") as out:
        for path in sorted(log_dir.glob("zcode-*.jsonl")):
            try:
                with path.open(encoding="utf-8", errors="replace") as f:
                    for line in f:
                        if session_id in line:
                            try:
                                out.write(json.dumps(sanitize(json.loads(line))) + "\n")
                            except json.JSONDecodeError:
                                continue
            except OSError:
                continue


def run_zcode_task(  # noqa: PLR0912, PLR0915, linear process + harvest
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
    effort: str | None = None,
    zcode_bin: str = "zcode",
    env_overrides: dict[str, str] | None = None,
    preflight: HarnessPreflight | None = None,
) -> CliAgentOutcome:
    """Run one task through ``zcode --prompt`` billed to a GLM Coding Plan.

    ZCode is Z.ai's desktop harness for GLM; the ``zcode`` command is the
    ``zcode-app-cli`` npm launcher around the same agent runtime
    (``zcode-runtime`` 0.16.x at the time of writing). Verified headless
    surface on that runtime: ``--prompt``/``-p``, ``--cwd``, ``--mode``,
    ``--disallowed-tools``, ``--verbose``, ``--no-color``. The launcher's help
    also advertises ``--max-turns``, ``--settings`` and ``--allowed-tools``,
    but the runtime's strict parser rejects them, so ``max_turns`` cannot be
    forwarded (the run's wall clock bounds it) and per-run settings travel
    through the project-level ``<workspace>/.zcode/config.json`` instead,
    which the runtime merges over the user config.

    Headless mode prints only the assistant's final text; tool calls,
    messages and per-request usage are harvested from ZCode's sqlite session
    store after the run (see :func:`_harvest_zcode_session`), so token
    receipts exist but a live ``--max-run-cost`` cap cannot be enforced.
    """
    del priced_spec, max_turns
    workspace = workspace.resolve()
    if timeout_s is not None and timeout_s <= 0:
        raise ProviderError("run budget exhausted before CLI agent start")
    if max_run_cost is not None:
        raise ProviderError(
            "zcode streams no usage during a run (tokens are harvested from its "
            "session store afterwards), so --max-run-cost cannot be enforced; "
            "use a wall-clock --timeout for subscription runs"
        )

    checked = preflight or _zcode_preflight(zcode_bin)
    _require_subscription(checked)

    zcode_dir = workspace / ".zcode"
    zcode_dir.mkdir(exist_ok=True)
    project_config = _zcode_project_config(model, network=network, effort=effort)
    (zcode_dir / "config.json").write_text(
        json.dumps(project_config, indent=1) + "\n", encoding="utf-8"
    )

    cmd = [
        zcode_bin,
        "--prompt",
        prompt,
        "--cwd",
        str(workspace),
        "--mode",
        "yolo",
        "--no-color",
        "--verbose",
    ]
    if not network:
        # Variadic flag: keep it last so it cannot swallow a following value.
        cmd += ["--disallowed-tools", *_ZCODE_WEB_TOOLS]

    collector.record(
        "cli_agent_start",
        {
            "harness": "zcode",
            "argv": [cmd[0], "--prompt", "<prompt omitted>", *cmd[3:]],
            "harness_version": checked.version,
            "project_config": _redact_project_config(project_config),
        },
    )
    env = _subscription_env(env_overrides)
    env["ZCODE_DISABLE_UPDATE_CHECK"] = "1"
    env["NO_UPDATE_NOTIFIER"] = "1"
    env["NO_COLOR"] = "1"
    user_config = _read_json_file(_zcode_user_config_path())
    db_path = _zcode_session_db_path(user_config)
    started_ms = int(time.time() * 1000) - 1000
    try:
        proc = subprocess.Popen(
            cmd,
            cwd=workspace,
            env=env,
            text=True,
            encoding="utf-8",
            errors="replace",
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except FileNotFoundError as exc:
        raise ProviderError(
            f"{zcode_bin!r} not found on PATH; install with "
            "`npm install -g zcode-app-cli@latest` and run `zcode login`"
        ) from exc

    stderr_chunks: list[str] = []

    def _drain_stderr() -> None:
        assert proc.stderr is not None
        for chunk in proc.stderr:
            stderr_chunks.append(chunk)

    stderr_thread = threading.Thread(target=_drain_stderr, daemon=True)
    stderr_thread.start()

    outcome = CliAgentOutcome(
        harness="zcode",
        execution_boundary=(
            "host-workspace; zcode-mode=yolo; memory-off; "
            + ("web-allowed" if network else "web-tools-removed+browser-plugin-off")
        ),
        requested_model=model,
        harness_version=checked.version,
        auth_method=checked.auth_mode,
        plan_name=checked.plan_name,
    )
    killed = {"timeout": False}

    def _kill_on_timeout() -> None:
        killed["timeout"] = True
        proc.kill()

    watchdog: threading.Timer | None = None
    if timeout_s is not None:
        watchdog = threading.Timer(timeout_s, _kill_on_timeout)
        watchdog.daemon = True
        watchdog.start()

    stdout_parts: list[str] = []
    stream_f = stream_log_path.open("w", encoding="utf-8") if stream_log_path else None
    try:
        assert proc.stdout is not None
        for raw_line in proc.stdout:
            stdout_parts.append(raw_line)
            if stream_f:
                json.dump(sanitize({"type": "stdout", "text": raw_line.rstrip("\n")}), stream_f)
                stream_f.write("\n")
    finally:
        if watchdog is not None:
            watchdog.cancel()
        proc.wait()
        stderr_thread.join(timeout=5)
        text = "".join(stdout_parts)
        if text.strip():
            collector.record("llm_response", {"text": text[:4000]})
        session_id, child_ids = _zcode_find_session(db_path, workspace, started_ms)
        if session_id:
            outcome.session_id = session_id
        if session_id and stream_log_path is not None:
            try:
                _harvest_zcode_session(
                    db_path, session_id, child_ids, stream_log_path.parent, stream_f, outcome
                )
                _harvest_zcode_log(session_id, stream_log_path.parent)
            except Exception as exc:  # harvest must never mask the run itself
                collector.record("cli_agent_harvest_error", {"error": str(exc)[:500]})
        if stream_f:
            stream_f.close()

    outcome.timed_out = killed["timeout"]
    stderr_text = "".join(stderr_chunks)
    if proc.returncode != 0 or killed["timeout"]:
        if killed["timeout"]:
            # Partial work still counts; the caller diffs and verifies it.
            return outcome
        tail = stderr_text[-600:].strip() or "no stderr"
        # The plan-limit cause is usually buried in the session, not on stderr.
        limit_detail = (
            tail
            if _ZCODE_LIMIT_PATTERN.search(tail)
            else _zcode_session_limit_error(db_path, outcome.session_id)
        )
        if limit_detail:
            raise SubscriptionQuotaError(
                "zcode Coding Plan limit hit, rerun after the window resets "
                f"(use --only-missing to resume): {limit_detail[:300]}"
            )
        raise ProviderError(f"zcode exited with status {proc.returncode}: {tail[:400]}")
    if outcome.session_id is None:
        raise ProviderError(
            "zcode exited 0 but no session for this workspace was found in "
            f"{db_path}; cannot attribute usage or tool calls to the run"
        )
    outcome.subtype = "success"
    outcome.finished = True
    collector.record("cli_agent_result", outcome.summary())
    return outcome


def _zcode_session_limit_error(db_path: Path, session_id: str | None) -> str | None:
    """Return a Coding Plan limit message from the session's usage rows, if any.

    When ZCode hits a 5-hour/weekly window (code 1308) or a rate limit (1302),
    the process exits 1 with only a generic stack on stderr; the real cause is
    recorded in ``model_usage.error_message``. Scanning it lets the suite pause
    on a quota exhaustion (``SubscriptionQuotaError``) instead of hot-retrying.
    """
    if not session_id or not db_path.exists():
        return None
    try:
        con = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
        try:
            rows = con.execute(
                "select error_message from model_usage "
                "where session_id = ? and error_message is not null "
                "order by started_at desc limit 20",
                (session_id,),
            ).fetchall()
        finally:
            con.close()
    except sqlite3.Error:
        return None
    for (msg,) in rows:
        if msg and _ZCODE_LIMIT_PATTERN.search(str(msg)):
            return str(msg)
    return None


def _require_subscription(preflight: HarnessPreflight) -> None:
    if preflight.ready:
        return
    detail = preflight.detail or "subscription authentication is not ready"
    raise ProviderError(f"{preflight.harness} preflight failed: {detail}")


def _require_ready(preflight: HarnessPreflight) -> None:
    if preflight.ready:
        return
    detail = preflight.detail or "authentication is not ready"
    raise ProviderError(f"{preflight.harness} preflight failed: {detail}")


@dataclass
class CliAgentOutcome:
    """What a CLI-agent run produced, in the loop's accounting terms."""

    harness: str = "unknown"
    billing: str = "subscription"
    cost_basis: str = "api-equivalent"
    execution_boundary: str | None = None
    requested_model: str | None = None
    reported_model: str | None = None
    model_identity_confidence: str = "requested-only"
    harness_version: str | None = None
    auth_method: str | None = None
    plan_name: str | None = None
    prompt_tokens: int = 0
    completion_tokens: int = 0
    cached_input_tokens: int = 0
    reasoning_output_tokens: int = 0
    finished: bool = False
    cost_capped: bool = False
    timed_out: bool = False
    session_id: str | None = None
    subtype: str | None = None
    num_turns: int | None = None
    cli_reported_cost_usd: float | None = None
    # Cumulative context+output total when a CLI reports only that (Grok
    # Build's trace `_meta.totalTokens`); no prompt/completion split exists,
    # so it never feeds pricing, it is provenance, not a bill.
    cli_total_tokens: int | None = None
    reported_effort: str | None = None
    sandbox_profile: str | None = None

    def summary(self) -> dict[str, Any]:
        """Provenance block persisted into the run summary."""
        return {
            "harness": self.harness,
            "harness_version": self.harness_version,
            "billing": self.billing,
            "cost_basis": self.cost_basis,
            "execution_boundary": self.execution_boundary,
            "auth_method": self.auth_method,
            "plan_name": self.plan_name,
            "requested_model": self.requested_model,
            "reported_model": self.reported_model,
            "model_identity_confidence": self.model_identity_confidence,
            "session_id": self.session_id,
            "subtype": self.subtype,
            "num_turns": self.num_turns,
            "cached_input_tokens": self.cached_input_tokens,
            "reasoning_output_tokens": self.reasoning_output_tokens,
            "cli_reported_cost_usd": self.cli_reported_cost_usd,
            "cli_total_tokens": self.cli_total_tokens,
            "reported_effort": self.reported_effort,
            "sandbox_profile": self.sandbox_profile,
        }


def run_claude_code_task(  # noqa: PLR0912, PLR0915, linear stream-parse loop
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
    effort: str | None = None,
    claude_bin: str = "claude",
    env_overrides: dict[str, str] | None = None,
    preflight: HarnessPreflight | None = None,
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

    checked = preflight or _claude_preflight(claude_bin)
    _require_subscription(checked)

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
        "--permission-mode",
        "auto",
        "--safe-mode",
        "--no-session-persistence",
        # Hermetic runs: don't let the operator's user-level config/memory
        # leak instructions into the benchmark.
        "--setting-sources",
        "project",
    ]
    if effort:
        cmd += ["--effort", effort]
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
            env=_subscription_env(env_overrides),
            text=True,
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except FileNotFoundError as e:
        raise ProviderError(
            f"{claude_bin!r} not found on PATH; install Claude Code and sign in "
            "with your subscription by running `claude` once"
        ) from e

    stderr_chunks: list[str] = []

    def _drain_stderr() -> None:
        assert proc.stderr is not None
        for chunk in proc.stderr:
            stderr_chunks.append(chunk)

    stderr_thread = threading.Thread(target=_drain_stderr, daemon=True)
    stderr_thread.start()

    outcome = CliAgentOutcome(
        harness="claude-code",
        execution_boundary="host-workspace; permission-mode=auto; safe-mode",
        requested_model=model,
        harness_version=checked.version,
        auth_method=checked.auth_mode,
        plan_name=checked.plan_name,
    )
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
                reported_model = event.get("model")
                if reported_model:
                    outcome.reported_model = str(reported_model)
                    outcome.model_identity_confidence = "cli-reported"
                collector.record(
                    "cli_agent_init",
                    {
                        "session_id": outcome.session_id,
                        "model": reported_model,
                        "harness_version": outcome.harness_version,
                    },
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
        # the CLI died without reporting, approximate usage from the stream.
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
            raise SubscriptionQuotaError(
                "claude code subscription limit hit, rerun after the window "
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


def _codex_item_trace_data(item: dict[str, Any]) -> tuple[str, dict[str, Any]] | None:
    """Translate a Codex JSONL item to a replay-compatible event."""
    item_type = item.get("type")
    if item_type == "agent_message":
        return "llm_response", {
            "content": item.get("text"),
            "tool_calls": [],
            "usage": {},
        }
    if item_type == "command_execution":
        return "tool_observation", {
            "tool": "command_execution",
            "command": item.get("command"),
            "result": item.get("aggregated_output") or item.get("output"),
            "exit_code": item.get("exit_code"),
            "status": item.get("status"),
        }
    if item_type in {"file_change", "mcp_tool_call", "web_search"}:
        return "tool_observation", {
            "tool": item_type,
            "result": item,
            "status": item.get("status"),
        }
    return None


def run_codex_task(  # noqa: PLR0912, PLR0915, linear process/stream adapter
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
    effort: str | None = None,
    codex_bin: str = "codex",
    env_overrides: dict[str, str] | None = None,
    preflight: HarnessPreflight | None = None,
) -> CliAgentOutcome:
    """Run one task through ``codex exec --json`` using ChatGPT auth."""
    del priced_spec, max_turns
    # The subprocess also uses this directory as cwd. Passing a relative path
    # to ``--cd`` would make Codex resolve it a second time from inside itself.
    workspace = workspace.resolve()
    if timeout_s is not None and timeout_s <= 0:
        raise ProviderError("run budget exhausted before CLI agent start")
    if max_run_cost is not None:
        raise ProviderError(
            "codex reports usage at turn completion, so --max-run-cost cannot be "
            "enforced live; use a wall-clock --timeout for subscription runs"
        )

    checked = preflight or _codex_preflight(codex_bin)
    _require_subscription(checked)
    cmd = [
        codex_bin,
        "exec",
        "--json",
        "--ephemeral",
        "--ignore-user-config",
        "--sandbox",
        "workspace-write",
        "--cd",
        str(workspace),
        "--model",
        model,
    ]
    if effort:
        cmd += ["--config", f'model_reasoning_effort="{effort}"']
    if network:
        cmd += ["--config", "sandbox_workspace_write.network_access=true"]
    cmd.append("-")

    collector.record(
        "cli_agent_start",
        {
            "harness": "codex",
            "argv": [*cmd[:-1], "<prompt via stdin>"],
            "harness_version": checked.version,
        },
    )
    try:
        proc = subprocess.Popen(
            cmd,
            cwd=workspace,
            env=_subscription_env(env_overrides),
            text=True,
            encoding="utf-8",
            errors="replace",
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except FileNotFoundError as exc:
        raise ProviderError(
            f"{codex_bin!r} not found on PATH; install Codex and run `codex login` "
            "with a ChatGPT subscription"
        ) from exc

    assert proc.stdin is not None
    proc.stdin.write(prompt)
    proc.stdin.close()
    stderr_chunks: list[str] = []

    def _drain_stderr() -> None:
        assert proc.stderr is not None
        for chunk in proc.stderr:
            stderr_chunks.append(chunk)

    stderr_thread = threading.Thread(target=_drain_stderr, daemon=True)
    stderr_thread.start()
    outcome = CliAgentOutcome(
        harness="codex",
        execution_boundary="host-workspace; sandbox=workspace-write",
        requested_model=model,
        harness_version=checked.version,
        auth_method=checked.auth_mode,
        plan_name=checked.plan_name,
    )
    killed = {"timeout": False}

    def _kill_on_timeout() -> None:
        killed["timeout"] = True
        proc.kill()

    watchdog: threading.Timer | None = None
    if timeout_s is not None:
        watchdog = threading.Timer(timeout_s, _kill_on_timeout)
        watchdog.daemon = True
        watchdog.start()

    terminal_error: str | None = None
    saw_turn_completed = False
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
            event_type = event.get("type")
            if event_type == "thread.started":
                outcome.session_id = event.get("thread_id")
                collector.record(
                    "cli_agent_init",
                    {
                        "session_id": outcome.session_id,
                        "model": model,
                        "harness_version": outcome.harness_version,
                    },
                )
            elif event_type == "item.completed":
                translated = _codex_item_trace_data(event.get("item") or {})
                if translated:
                    collector.record(*translated)
            elif event_type == "turn.completed":
                saw_turn_completed = True
                usage = event.get("usage") or {}
                outcome.prompt_tokens = int(usage.get("input_tokens", 0) or 0)
                outcome.cached_input_tokens = int(usage.get("cached_input_tokens", 0) or 0)
                outcome.completion_tokens = int(usage.get("output_tokens", 0) or 0)
                outcome.reasoning_output_tokens = int(usage.get("reasoning_output_tokens", 0) or 0)
                outcome.subtype = "success"
                outcome.finished = True
            elif event_type in {"turn.failed", "error"}:
                payload = event.get("error") or event.get("message") or event
                terminal_error = str(payload)
    finally:
        if watchdog is not None:
            watchdog.cancel()
        if stream_f:
            stream_f.close()

    proc.wait()
    stderr_thread.join(timeout=5)
    outcome.timed_out = killed["timeout"]
    if outcome.timed_out:
        return outcome
    if not saw_turn_completed or proc.returncode != 0:
        detail = terminal_error or "".join(stderr_chunks)[-500:].strip() or "no error detail"
        if _LIMIT_PATTERN.search(detail):
            raise SubscriptionQuotaError(
                "codex subscription limit hit, rerun after the window resets "
                f"(use --only-missing to resume): {detail[:300]}"
            )
        raise ProviderError(f"codex exec failed (exit {proc.returncode}): {detail[:500]}")
    collector.record("cli_agent_result", outcome.summary())
    return outcome


def _pi_inner_spec(cli_model: str) -> str:
    """Inner ``provider:model`` that Pi should call.

    ``--harness pi --model meta:muse-spark-1.2`` becomes ``pi:meta:muse-spark-1.2``,
    so the CLI adapter sees ``meta:muse-spark-1.2``. A bare ``muse-spark-*`` id
    defaults to the Meta provider used in Report No. 19.
    """
    model = cli_model.strip()
    if ":" in model:
        return model
    if model.lower().startswith("muse-spark"):
        return f"meta:{model}"
    raise ProviderError(
        "pi harness needs an inner provider:model spec "
        f"(e.g. meta:muse-spark-1.2), got {cli_model!r}"
    )


def _pi_key_env_names() -> tuple[str, ...]:
    return (
        "META_MUSE_SPARK_API",
        "MODEL_API_KEY",
        "OPENROUTER_API_KEY",
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
    )


def _pi_env(home: Path, extra: dict[str, str] | None = None) -> dict[str, str]:
    """Host env for Pi: subscription scrub plus the API keys Pi must send."""
    overrides = {"HOME": str(home)}
    for key in _PI_PASSTHROUGH_ENV:
        value = os.environ.get(key)
        if value:
            overrides[key] = value
    if extra:
        overrides.update(extra)
    return _subscription_env(overrides)


def _pi_preflight(pi_bin: str = "pi") -> HarnessPreflight:
    version = _version(pi_bin)
    if version is None:
        return HarnessPreflight(
            harness="pi",
            available=False,
            version=None,
            authenticated=False,
            auth_mode=None,
            detail=(
                f"{pi_bin!r} not found on PATH; install with "
                "`npm install -g @earendil-works/pi-coding-agent`"
            ),
        )
    present = [name for name in _pi_key_env_names() if os.environ.get(name)]
    if not present:
        return HarnessPreflight(
            harness="pi",
            available=True,
            version=version,
            authenticated=False,
            auth_mode=None,
            detail=(
                "Pi is API-metered; set META_MUSE_SPARK_API (or MODEL_API_KEY / "
                "OPENROUTER_API_KEY) for Muse Spark, or OPENAI_API_KEY / "
                "ANTHROPIC_API_KEY for those providers"
            ),
        )
    return HarnessPreflight(
        harness="pi",
        available=True,
        version=version,
        authenticated=True,
        auth_mode="api",
        plan_name=None,
        detail=None,
    )


def _write_pi_meta_models_json(home: Path, inner_model: str) -> tuple[str, str]:
    """Write a per-run Pi models.json that targets Meta's OpenAI-compatible API.

    Returns ``(pi_model_flag, key_env)``. Pi resolves ``apiKey`` as an
    environment-variable name (see Pi models.md), so we write the name, not
    the secret and not a ``$NAME`` shell interpolant (that would be sent as
    a literal and Meta would 401).

    ``contextWindow`` must be declared: Pi assumes 128 K for custom models,
    auto-compacts past it, and its compaction resume has crashed runs
    ("Cannot continue from message role: assistant"). Meta accepted a 137.8 K
    request on a live pennylane run, so 128 K is wrong for Muse Spark;
    override with ``META_CONTEXT_WINDOW`` if Meta documents a different size.
    """
    route = _resolve_meta_route(inner_model)
    key_env = next((name for name in route.key_envs if os.environ.get(name)), None)
    if key_env is None:
        needed = " or ".join(route.key_envs)
        raise ProviderError(f"{needed} is not set")
    models_dir = home / ".pi" / "agent"
    models_dir.mkdir(parents=True, exist_ok=True)
    payload = {
        "providers": {
            "vulcan-meta": {
                "baseUrl": route.base or META_DEFAULT_BASE_URL,
                "api": "openai-responses",
                "apiKey": key_env,
                "authHeader": True,
                "models": [
                    {
                        "id": route.wire_model,
                        "name": inner_model,
                        "reasoning": True,
                        "input": ["text"],
                        "contextWindow": int(os.environ.get("META_CONTEXT_WINDOW", "262144")),
                    }
                ],
            }
        }
    }
    (models_dir / "models.json").write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    return f"vulcan-meta/{route.wire_model}", key_env


def _pi_usage_tokens(usage: dict[str, Any]) -> tuple[int, int, int]:
    """Parse one Pi usage record into (prompt, completion, cache_read).

    Pi's ``usage.input`` counts only uncached fresh input; cache reads and
    writes are separate fields. VulcanBench's convention (see the grok
    adapter) is ``prompt_tokens`` = total input including cache, with
    ``cached_input_tokens`` the cache-read subset, so pricing can discount it.
    """
    cache = int(
        usage.get("cacheRead") or usage.get("cache_read") or usage.get("cached_input_tokens") or 0
    )
    cache_write = int(usage.get("cacheWrite") or usage.get("cache_write") or 0)
    inp = int(usage.get("input") or usage.get("inputTokens") or usage.get("input_tokens") or 0)
    out = int(usage.get("output") or usage.get("outputTokens") or usage.get("output_tokens") or 0)
    return inp + cache + cache_write, out, cache


def _pi_usage_cost(usage: dict[str, Any]) -> float | None:
    """Pi's own USD figure for one usage record, if present."""
    raw: Any = usage.get("cost")
    if isinstance(raw, dict):
        raw = raw.get("total")
    if raw is None:
        return None
    try:
        return float(raw)
    except (TypeError, ValueError):
        return None


def run_pi_task(  # noqa: PLR0912, PLR0915  linear stream-parse loop
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
    effort: str | None = None,
    pi_bin: str = "pi",
    env_overrides: dict[str, str] | None = None,
    preflight: HarnessPreflight | None = None,
) -> CliAgentOutcome:
    """Run one task through Pi (``@earendil-works/pi-coding-agent``) on API keys.

    Pi is a minimal open-source coding harness (read/write/edit/bash, no web
    tools by default). Results measure **model + Pi**, not the VulcanBench
    loop: the same ``meta:muse-spark-1.2`` column through ``--harness vulcan``
    vs ``--harness pi`` is the harness delta. Billing stays on the API track
    (``cli_agent.billing=api``); tokens price at the inner spec
    (``pi:meta:muse-spark-1.2`` → ``meta:muse-spark-1.2``).

    Isolation: ``HOME`` is a sibling of the workspace (operator ``~/.pi``
    skills/config cannot leak in) and ``--no-session`` disables Pi's session
    log. Meta models get a per-run ``models.json`` pointing at the same
    Responses endpoint Vulcan's ``MetaProvider`` uses.
    """
    del priced_spec, max_turns, network  # Pi has no max-turns flag; no web tools to deny.
    workspace = workspace.resolve()
    if timeout_s is not None and timeout_s <= 0:
        raise ProviderError("run budget exhausted before CLI agent start")
    if max_run_cost is not None:
        raise ProviderError(
            "Pi reports usage at message boundaries rather than a live cost "
            "stream, so --max-run-cost cannot be enforced; use --timeout"
        )

    checked = preflight or _pi_preflight(pi_bin)
    _require_ready(checked)

    inner = _pi_inner_spec(model)
    provider_name, inner_model = parse_model_spec(inner)
    home = workspace.parent / "pi-home"
    home.mkdir(parents=True, exist_ok=True)
    if provider_name == "meta":
        pi_model, _key_env = _write_pi_meta_models_json(home, inner_model)
    elif provider_name in {"openai", "anthropic"}:
        pi_model = f"{provider_name}/{inner_model}"
    else:
        raise ProviderError(
            f"pi harness does not yet route provider {provider_name!r}; "
            "use meta:muse-spark-1.2 (or openai:/anthropic: inner specs)"
        )

    cmd = [
        pi_bin,
        "--mode",
        "json",
        "-p",
        "--no-session",
        "--no-skills",
        "--no-extensions",
        "--no-context-files",
        "--model",
        pi_model,
    ]
    if effort:
        cmd += ["--thinking", effort]
    cmd.append(prompt)

    logged_argv = [
        cmd[0],
        "--mode",
        "json",
        "-p",
        "--no-session",
        "--no-skills",
        "--no-extensions",
        "--no-context-files",
        "--model",
        pi_model,
    ]
    if effort:
        logged_argv += ["--thinking", effort]
    logged_argv.append("<prompt omitted>")

    collector.record(
        "cli_agent_start",
        {
            "harness": "pi",
            "argv": logged_argv,
            "harness_version": checked.version,
            "inner_spec": inner,
        },
    )

    try:
        proc = subprocess.Popen(
            cmd,
            cwd=workspace,
            env=_pi_env(home, env_overrides),
            text=True,
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except FileNotFoundError as e:
        raise ProviderError(
            f"{pi_bin!r} not found on PATH; install with "
            "`npm install -g @earendil-works/pi-coding-agent`"
        ) from e

    stderr_chunks: list[str] = []

    def _drain_stderr() -> None:
        assert proc.stderr is not None
        for chunk in proc.stderr:
            stderr_chunks.append(chunk)

    stderr_thread = threading.Thread(target=_drain_stderr, daemon=True)
    stderr_thread.start()

    outcome = CliAgentOutcome(
        harness="pi",
        billing="api",
        cost_basis="metered-api-pricing",
        execution_boundary="host-workspace; pi tools=read,write,edit,bash; no-session",
        requested_model=inner,
        reported_model=pi_model,
        model_identity_confidence="requested-plus-wire",
        harness_version=checked.version,
        auth_method="api",
        reported_effort=effort,
        sandbox_profile="none (host; docker verifier allowed)",
    )
    killed = {"timeout": False}

    def _kill_on_timeout() -> None:
        killed["timeout"] = True
        proc.kill()

    watchdog: threading.Timer | None = None
    if timeout_s is not None:
        watchdog = threading.Timer(timeout_s, _kill_on_timeout)
        watchdog.daemon = True
        watchdog.start()

    # Pi reports usage per assistant message (finalized on ``message_end``);
    # Pi's own session stats sum those messages. Summing here, not keeping the
    # last record, is what makes the run's tokens and cost the whole run's.
    sum_prompt = sum_completion = sum_cached = 0
    sum_cost = 0.0
    usage_records = 0
    cost_records = 0
    last_usage: dict[str, Any] | None = None
    last_error: str | None = None
    session_id: str | None = None
    turns = 0
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
            etype = str(event.get("type") or "")
            if etype == "session":
                session_id = str(event.get("id") or "") or session_id
            elif etype == "turn_end":
                turns += 1
            usage = event.get("usage")
            if isinstance(usage, dict):
                last_usage = usage
            message = event.get("message")
            if isinstance(message, dict):
                msg_usage = message.get("usage")
                if isinstance(msg_usage, dict):
                    last_usage = msg_usage
                    # Only finalized assistant messages count toward the sum;
                    # message_start/message_update carry partial figures for
                    # the same message and would double-count.
                    if etype == "message_end" and message.get("role") == "assistant":
                        prompt, completion, cache = _pi_usage_tokens(msg_usage)
                        sum_prompt += prompt
                        sum_completion += completion
                        sum_cached += cache
                        usage_records += 1
                        msg_cost = _pi_usage_cost(msg_usage)
                        if msg_cost is not None:
                            sum_cost += msg_cost
                            cost_records += 1
                err = message.get("errorMessage")
                if message.get("stopReason") == "error" or err:
                    last_error = str(err or "pi turn failed")
            collector.record("cli_agent_event", {"type": etype})
    finally:
        if watchdog is not None:
            watchdog.cancel()
        if stream_f:
            stream_f.close()

    proc.wait()
    stderr_thread.join(timeout=2)
    outcome.timed_out = killed["timeout"]
    outcome.session_id = session_id
    outcome.num_turns = turns or None
    if usage_records:
        outcome.prompt_tokens = sum_prompt
        outcome.completion_tokens = sum_completion
        outcome.cached_input_tokens = sum_cached
        if cost_records:
            outcome.cli_reported_cost_usd = round(sum_cost, 6)
    elif last_usage:
        # No message_end usage seen (older Pi, or a stream cut short): the
        # last record is a per-message figure and understates the run.
        prompt_tokens, completion_tokens, cached = _pi_usage_tokens(last_usage)
        outcome.prompt_tokens = prompt_tokens
        outcome.completion_tokens = completion_tokens
        outcome.cached_input_tokens = cached
        outcome.cli_reported_cost_usd = _pi_usage_cost(last_usage)
    if outcome.timed_out:
        outcome.finished = False
        collector.record("cli_agent_result", outcome.summary())
        return outcome
    if last_error:
        raise ProviderError(f"pi provider error: {last_error[:500]}")
    if proc.returncode != 0:
        detail = "".join(stderr_chunks)[-500:].strip() or "no error detail"
        raise ProviderError(f"pi failed (exit {proc.returncode}): {detail[:500]}")
    outcome.finished = True
    collector.record("cli_agent_result", outcome.summary())
    return outcome


@dataclass(frozen=True)
class ClaudeCodeAdapter:
    harness_id: str = "claude-code"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            harness=self.harness_id,
            display_name="Claude Code",
            executable="claude",
            structured_events=True,
            reports_tokens=True,
            reports_model=True,
            supports_effort=True,
            supports_live_cost_cap=True,
            sandbox="native-permission-auto; host workspace",
        )

    def preflight(self) -> HarnessPreflight:
        return _claude_preflight()

    def run_task(self, **kwargs: Any) -> CliAgentOutcome:
        return run_claude_code_task(**kwargs)


@dataclass(frozen=True)
class CodexAdapter:
    harness_id: str = "codex"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            harness=self.harness_id,
            display_name="Codex CLI",
            executable="codex",
            structured_events=True,
            reports_tokens=True,
            reports_model=False,
            supports_effort=True,
            supports_live_cost_cap=False,
            sandbox="workspace-write",
        )

    def preflight(self) -> HarnessPreflight:
        return _codex_preflight()

    def run_task(self, **kwargs: Any) -> CliAgentOutcome:
        return run_codex_task(**kwargs)


@dataclass(frozen=True)
class CursorAdapter:
    harness_id: str = "cursor"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            harness=self.harness_id,
            display_name="Cursor CLI",
            executable="cursor-agent",
            structured_events=True,
            # stream-json carries no usage or cost fields: token counts and
            # API-equivalent value are honestly unavailable for cursor runs.
            reports_tokens=False,
            reports_model=True,
            supports_effort=True,
            supports_live_cost_cap=False,
            sandbox="cursor-sandbox=enabled; force-allow",
        )

    def preflight(self) -> HarnessPreflight:
        return _cursor_preflight()

    def run_task(self, **kwargs: Any) -> CliAgentOutcome:
        return run_cursor_task(**kwargs)


@dataclass(frozen=True)
class GrokBuildAdapter:
    harness_id: str = "grok-build"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            harness=self.harness_id,
            display_name="Grok Build",
            executable="grok",
            structured_events=True,
            # grok >= 1.0 streams per-call usage and an end event with the
            # full token split and the CLI's own total_cost_usd.
            reports_tokens=True,
            reports_model=True,
            supports_effort=True,
            supports_live_cost_cap=True,
            sandbox="grok-sandbox=vulcanbench (workspace writes, kernel-denied repo)",
        )

    def preflight(self) -> HarnessPreflight:
        return _grok_build_preflight()

    def run_task(self, **kwargs: Any) -> CliAgentOutcome:
        return run_grok_build_task(**kwargs)


@dataclass(frozen=True)
class ZCodeAdapter:
    harness_id: str = "zcode"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            harness=self.harness_id,
            display_name="ZCode",
            executable="zcode",
            # Headless stdout is plain text; the structured record (messages,
            # tool calls with inputs, per-request usage) is harvested from
            # ZCode's sqlite session store after the run.
            structured_events=True,
            reports_tokens=True,
            reports_model=True,
            supports_effort=True,
            supports_live_cost_cap=False,
            sandbox="zcode-mode=yolo; host workspace (web tools removed)",
        )

    def preflight(self) -> HarnessPreflight:
        return _zcode_preflight()

    def run_task(self, **kwargs: Any) -> CliAgentOutcome:
        return run_zcode_task(**kwargs)


@dataclass(frozen=True)
class PiAdapter:
    harness_id: str = "pi"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            harness=self.harness_id,
            display_name="Pi",
            executable="pi",
            structured_events=True,
            reports_tokens=True,
            reports_model=True,
            supports_effort=True,
            supports_live_cost_cap=False,
            sandbox="host workspace (read/write/edit/bash; no web tools)",
        )

    def preflight(self) -> HarnessPreflight:
        return _pi_preflight()

    def run_task(self, **kwargs: Any) -> CliAgentOutcome:
        return run_pi_task(**kwargs)


_CLI_AGENT_ADAPTERS: dict[str, CliAgentAdapter] = {
    "claude-code": ClaudeCodeAdapter(),
    "codex": CodexAdapter(),
    "cursor": CursorAdapter(),
    "grok-build": GrokBuildAdapter(),
    "pi": PiAdapter(),
    "zcode": ZCodeAdapter(),
}


def get_cli_agent_adapter(spec_or_name: str) -> CliAgentAdapter:
    """Resolve a harness name or ``harness:model`` spec to its adapter."""
    name = spec_or_name.partition(":")[0].strip().lower()
    try:
        return _CLI_AGENT_ADAPTERS[name]
    except KeyError as exc:
        known = ", ".join(sorted(_CLI_AGENT_ADAPTERS))
        raise ValueError(f"unknown execution harness {name!r}; known: {known}") from exc


def list_cli_agent_adapters() -> list[CliAgentAdapter]:
    """All external harness adapters in stable display order."""
    return [_CLI_AGENT_ADAPTERS[name] for name in sorted(_CLI_AGENT_ADAPTERS)]


def _fold_codex_judge_usage(usage: dict[str, Any]) -> tuple[int, int]:
    """Codex usage fields to effective tokens for a single-shot judge call.

    OpenAI bills cached input at ~0.1x, and Codex's ``input_tokens`` INCLUDES
    the cached portion (unlike Anthropic's split fields), so fold it down.
    """
    total_in = int(usage.get("input_tokens", 0) or 0)
    cached = min(int(usage.get("cached_input_tokens", 0) or 0), total_in)
    prompt = round((total_in - cached) + cached * 0.1)
    return prompt, int(usage.get("output_tokens", 0) or 0)


class CodexProvider(LLMProvider):
    """Single-shot completions through Codex headless.

    Used for judge/grader calls when the run model is a ``codex:`` spec, so
    evaluation also bills the subscription. Runs in the read-only sandbox;
    tool calling is not supported, judges and graders are plain
    prompt-in/text-out completions.
    """

    @property
    def name(self) -> str:
        return "codex"

    def complete(  # noqa: PLR0912, linear stream-parse over event kinds
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        del tools, effort
        prompt = "\n\n".join(str(m.get("content", "")) for m in messages)
        cmd = [
            "codex",
            "exec",
            "--json",
            "--ephemeral",
            "--ignore-user-config",
            "--model",
            self.model,
            "--sandbox",
            "read-only",
            "-",
        ]
        try:
            proc = subprocess.run(
                cmd,
                input=prompt,
                capture_output=True,
                text=True,
                timeout=timeout_s if timeout_s and timeout_s > 0 else 600,
                env=_subscription_env(),
                check=False,
            )
        except FileNotFoundError as e:
            raise ProviderError("'codex' not found on PATH for judge/grader call") from e
        except subprocess.TimeoutExpired as e:
            raise ProviderError("codex judge/grader call timed out") from e

        text = ""
        prompt_tokens = completion_tokens = 0
        error_text: str | None = None
        for raw_line in proc.stdout.splitlines():
            line = raw_line.strip()
            if not line:
                continue
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            etype = str(event.get("type") or "")
            if etype == "item.completed":
                item = event.get("item") or {}
                itype = str(item.get("item_type") or item.get("type") or "")
                if itype in ("assistant_message", "agent_message"):
                    text = str(item.get("text") or "")
                elif itype == "error":
                    error_text = str(item.get("message") or "")
            elif etype == "turn.completed":
                p, c = _fold_codex_judge_usage(event.get("usage") or {})
                prompt_tokens += p
                completion_tokens += c
            elif etype in ("turn.failed", "error"):
                error_text = str(
                    (event.get("error") or {}).get("message") or event.get("message") or "error"
                )
        if error_text and _LIMIT_PATTERN.search(error_text):
            raise ProviderError(f"codex subscription limit hit: {error_text[:300]}")
        if proc.returncode != 0:
            raise ProviderError(
                f"codex judge call failed (exit {proc.returncode}): "
                f"{(error_text or proc.stderr or proc.stdout)[-300:]}"
            )
        if error_text:
            raise ProviderError(f"codex judge call errored: {error_text[:300]}")
        return LLMResponse(
            content=text or None,
            usage=TokenUsage(prompt_tokens=prompt_tokens, completion_tokens=completion_tokens),
            raw={"stdout_tail": proc.stdout[-1000:]},
        )


class ClaudeCodeProvider(LLMProvider):
    """Single-shot completions through Claude Code headless.

    Used for judge/grader calls when the run model is a ``claude-code:`` spec,
    so evaluation also bills the subscription. Tool calling is not supported,
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
        _require_subscription(_claude_preflight())
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
            "--permission-mode",
            "auto",
            "--safe-mode",
            "--no-session-persistence",
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
                raise SubscriptionQuotaError(f"claude code subscription limit hit: {text[:300]}")
            raise ProviderError(f"claude code judge call errored: {text[:300]}")
        p, c = _fold_usage(body.get("usage") or {})
        return LLMResponse(
            content=text or None,
            usage=TokenUsage(prompt_tokens=p, completion_tokens=c),
            raw=body,
        )
