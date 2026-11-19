"""Antigravity CLI adapter: Google's own agent for Gemini, subscription sign-in.

Antigravity CLI (``agy``) is Google's terminal agent; Google AI Pro and Ultra
moved to it from Gemini CLI on 2026-06-18, and Gemini CLI's own README says not
to use Gemini CLI to benchmark Gemini models. VulcanBench runs agy's one-shot
print mode (``agy -p``) from a pinned, hash-checked binary, inside the same macOS
outer sandbox as Kimi Code and Mirror, with a private ``HOME`` per run.

Run settings (owner decisions 2026-11-19, docs/DECISIONS.md):

- Billing is the owner's Google AI Pro plan through agy's Google sign-in, not an
  API key. Cost is reported as the API-equivalent at Gemini list price, and
  ``useG1Credits`` is off so a run never spends paid AI credits past the plan
  quota (a quota refusal ends the run as infrastructure, not a result).
- Every effort agy offers for the model runs. agy encodes effort in the model
  slug (``gemini-4-neon-xhigh``); Neon has low, medium, high and xhigh.
- Web tools are off, as for every harness on the board. agy has no switch for
  ``search_web`` (permission ``deny`` rules accept unknown actions silently),
  so the run uses a custom main agent whose ``tools`` allowlist is exactly the
  default agent's file and shell tools. A custom agent's body replaces agy's
  identity block, so the body is agy 1.5.12's own identity text, verbatim. The
  sections that go with the removed tools (subagents, skills, slash commands,
  conversation transcripts) are absent; guidelines, communication style,
  artifacts and messaging are unchanged. ``deny`` rules also remove URL reads,
  browser navigation and MCP, and any call to a tool outside the allowlist
  voids the run.
- Print mode runs with ``--dangerously-skip-permissions`` (agy's always-proceed
  mode). The outer sandbox is the boundary; agy's own ``--sandbox`` stays off
  because sandbox-exec cannot nest. agy's updater cannot replace the pinned
  binary during a run: the outer sandbox gives no write access to its
  directory, and agy skips the update when that directory is not writable.
  ``AGY_CLI_DISABLE_AUTO_UPDATE`` alone does not stop the updater (agy 1.5.12
  still checks at every start), so the operator keeps the pin directory
  read-only for agy calls made outside a run.

The private ``HOME`` holds the custom agent and agy's per-run state, so no prior
conversation, summary or setting is visible to the run. macOS finds the login
keychain through ``$HOME/Library/Keychains``, so that one directory is linked in
for the sign-in token; the real ``~/.gemini`` is denied to the run.

Token use comes from the stream's per-step ``usage`` (one per model call):
``input_tokens`` is fresh input, ``cache_read_tokens`` the implicit-cache reads,
``output_tokens`` includes thinking. The step sum must match the result event.

Imported lazily by cli_agents to avoid circular adapter registration.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import signal
import subprocess
import tempfile
import threading
import time
from contextlib import suppress
from pathlib import Path
from typing import Any

from harness.agent.cli_agents import (
    CliAgentOutcome,
    HarnessCapabilities,
    HarnessPreflight,
    _subscription_env,
)
from harness.agent.deepseek_harness import sandbox_context
from harness.agent.muse_code import boundary_profile
from harness.agent.providers import ProviderError
from harness.redaction import sanitize

VERSION = "1.5.12"
# sha256 of the darwin-arm64 ``agy`` from Google's 1.5.12 release tarball
# (manifest sha512 verified when it was pinned, 2026-11-19).
BINARY_SHA256 = "ef265356c3e5d3c6c241509cf71187bae086ac935ef5c5c037dadf62d5876835"
DEFAULT_BINARY = Path.home() / f".local/share/vulcanbench/agy-{VERSION}/agy"
BINARY_ENV = "VULCANBENCH_AGY_BIN"
PLAN_NAME = "Google AI Pro"
# VulcanBench model spec -> agy model slug prefix; agy appends the effort.
MODELS = {"gemini-4-neon": "gemini-4-neon"}
# The efforts `agy models` lists for each model (Neon: no max, no minimal).
EFFORTS = {"gemini-4-neon": frozenset({"low", "medium", "high", "xhigh"})}
AGENT_NAME = "vulcanbench"
# The default agent's file and shell tools, by the names agy resolves.
AGENT_TOOLS = ("read_file", "write_file", "edit_file", "run_command")
# Tools a run may call: the allowlist plus manage_task, which agy always adds.
ALLOWED_TOOLS = frozenset({*AGENT_TOOLS, "manage_task"})
# agy 1.5.12's identity block, generic-system-prompt variant (the one Neon is
# served), non-autonomous branch, exactly as the binary's template renders it.
IDENTITY = (
    "You are Antigravity, a powerful agentic AI assistant designed by the Google Deepmind team.\n"
    "You are working with a USER to solve their task.\n"
    "The USER will send you requests, which you must always prioritize addressing. "
    "User requests are enclosed within <USER_REQUEST> tags.\n"
)
SETTINGS = {
    "useG1Credits": False,
    "permissions": {"deny": ["read_url(*)", "execute_url(*)", "mcp(*)"]},
}
# Failures that are the account, the plan quota or the service, not the model's work.
INFRA_MARKERS = (
    "not logged into",
    "sign in",
    "RESOURCE_EXHAUSTED",
    "quota",
    "usage limit",
    "rate limit",
    "429",
    "UNAVAILABLE",
    "503",
    "INTERNAL",
    "DEADLINE_EXCEEDED",
    "ECONNRESET",
)
_EXIT_CODE = re.compile(r"Command exited with code (-?\d+)")


def agent_file() -> str:
    """The custom main agent: the default's file and shell tools, agy's own identity."""
    tools = ", ".join(AGENT_TOOLS)
    return (
        "---\n"
        f"name: {AGENT_NAME}\n"
        "description: Antigravity's default agent with its file and shell tools only.\n"
        f"tools: [{tools}]\n"
        "mainAgent: true\n"
        "subagent: false\n"
        "model: inherit\n"
        "---\n"
        f"{IDENTITY}"
    )


def model_slug(model: str, effort: str) -> str:
    return f"{MODELS[model]}-{effort}"


def pinned_binary() -> Path:
    """The agy executable, refused unless it is the pinned build."""
    binary = Path(os.environ.get(BINARY_ENV) or DEFAULT_BINARY)
    if not binary.is_absolute() or not binary.is_file() or not os.access(binary, os.X_OK):
        raise ProviderError(f"No executable agy {VERSION} at {binary} (set {BINARY_ENV})")
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if digest != BINARY_SHA256:
        raise ProviderError(f"agy binary SHA256 mismatch: adapter pins {VERSION}")
    return binary.resolve()


def private_home(scratch: Path) -> Path:
    """A per-run HOME: the custom agent, the login keychain link and nothing else."""
    home = scratch / "agy-home"
    agents = home / ".gemini/config/agents"
    agents.mkdir(parents=True)
    (agents / f"{AGENT_NAME}.md").write_text(agent_file())
    (home / "Library").mkdir()
    (home / "Library/Keychains").symlink_to(Path.home() / "Library/Keychains")
    return home


def run_profile(workspace: Path, scratch: Path) -> str:
    """The shared outer sandbox, plus pseudo-terminals and the operator's agy state denied.

    agy runs every ``run_command`` in a PTY; without these rules each command
    fails with "failed to create PTY: operation not permitted".
    """
    real = (Path.home() / ".gemini").resolve()
    return (
        boundary_profile(workspace, scratch)
        + "(allow pseudo-tty)\n"
        + '(allow file-read* file-write* file-ioctl (literal "/dev/ptmx") '
        + '(regex #"^/dev/ttys[0-9]+$"))\n'
        + f"(deny file-read* file-write* (subpath {json.dumps(str(real))}))\n"
    )


def toolchain_env() -> dict[str, str]:
    """Keep rustup, cargo and Go on the operator's toolchains under the private HOME."""
    env = {}
    for name, rel in (("RUSTUP_HOME", ".rustup"), ("CARGO_HOME", ".cargo"), ("GOPATH", "go")):
        path = Path.home() / rel
        if os.environ.get(name):
            env[name] = os.environ[name]
        elif path.is_dir():
            env[name] = str(path)
    return env


def _count(value: Any) -> int:
    if type(value) is not int or value < 0:
        raise ProviderError("Invalid agy token count")
    return value


def step_usage(usage: dict[str, Any]) -> dict[str, int]:
    return {
        "input": _count(usage.get("input_tokens", 0)),
        "output": _count(usage.get("output_tokens", 0)),
        "thinking": _count(usage.get("thinking_tokens", 0)),
        "cacheRead": _count(usage.get("cache_read_tokens", 0)),
    }


class _Stream:
    """What a run's stream-json established: identity, usage, tools, answer."""

    def __init__(self) -> None:
        self.conversation_id: str | None = None
        self.init: dict[str, Any] = {}
        self.steps: dict[Any, dict[str, int]] = {}
        self.result: dict[str, Any] | None = None
        self.tools: list[str] = []
        self.text: list[str] = []

    def feed(self, event: dict[str, Any]) -> None:
        kind = event.get("event")
        if kind == "init":
            self.init = event.get("init") or {}
            self.conversation_id = event.get("conversation_id")
        elif kind == "step_update":
            step = event.get("step_update") or {}
            if step.get("step_type") == "agent_response":
                if isinstance(step.get("text_delta"), str):
                    self.text.append(step["text_delta"])
                if isinstance(step.get("usage"), dict):
                    self.steps[step.get("step_index")] = step_usage(step["usage"])
            elif step.get("step_type") == "tool" and step.get("state") != "ACTIVE":
                self.tools.append(str(step.get("tool_name")))
        elif kind == "result":
            self.result = event.get("result") or {}

    def usage(self) -> dict[str, int]:
        totals = {"input": 0, "output": 0, "thinking": 0, "cacheRead": 0, "calls": 0}
        for usage in self.steps.values():
            for name, value in usage.items():
                totals[name] += value
            totals["calls"] += 1
        return totals


class AntigravityAdapter:
    harness_id = "agy"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            self.harness_id,
            "Antigravity CLI",
            "agy",
            True,
            True,
            True,
            True,
            False,
            "macOS outer sandbox; private HOME; isolated writes; repository read deny",
        )

    def preflight(self) -> HarnessPreflight:
        try:
            pinned_binary()
        except ProviderError as exc:
            return HarnessPreflight(self.harness_id, False, None, False, None, detail=str(exc))
        # The sign-in token's keychain item (service "gemini"); its value is never read.
        found = subprocess.run(
            ["/usr/bin/security", "find-generic-password", "-s", "gemini"],
            capture_output=True,
            check=False,
        )
        signed_in = found.returncode == 0
        return HarnessPreflight(
            self.harness_id,
            True,
            VERSION,
            signed_in,
            "google-oauth" if signed_in else None,
            plan_name=PLAN_NAME if signed_in else None,
            detail="Google sign-in present in the keychain; quota is checked by the run",
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
        del kwargs, max_turns, priced_spec
        if agent_container:
            raise ProviderError("agy runs locally under the outer sandbox, not in a container")
        if network:
            raise ProviderError("agy runs keep web tools off; network tasks are not supported")
        if max_run_cost is not None:
            raise ProviderError("agy runs are bounded by wall clock, not a live cost cap")
        if model not in MODELS:
            raise ProviderError(f"agy model {model!r} is not configured for VulcanBench")
        if effort not in EFFORTS[model]:
            levels = ", ".join(sorted(EFFORTS[model]))
            raise ProviderError(f"agy {model} requires an explicit effort: {levels}")
        if timeout_s is None or timeout_s <= 0:
            raise ProviderError("agy requires a positive wall-clock timeout")
        if not stream_log_path:
            raise ProviderError("agy runs need a stream log path to archive the full trace")
        binary = pinned_binary()
        slug = model_slug(model, effort)
        workspace = Path(workspace).resolve()
        scratch = Path(tempfile.mkdtemp(prefix="vb-agy-session-")).resolve()
        (scratch / "tmp").mkdir()
        home = private_home(scratch)
        settings = json.dumps(SETTINGS, sort_keys=True)
        env = _subscription_env()
        env.update(toolchain_env())
        env.update(
            {
                "HOME": str(home),
                # agy 1.5.12 ignores this; the sandbox is what blocks updates.
                # Kept in case a later agy honors it.
                "AGY_CLI_DISABLE_AUTO_UPDATE": "1",
                "TMPDIR": str(scratch / "tmp"),
                "TMP": str(scratch / "tmp"),
                "TEMP": str(scratch / "tmp"),
            }
        )
        profile = run_profile(workspace, scratch)
        profile_path = scratch / "boundary.sb"
        profile_path.write_text(profile)
        # Print mode takes the task only as an argument; it reaches the process
        # table, which on this single-operator host shows nothing new.
        cmd = [
            "/usr/bin/sandbox-exec",
            "-f",
            str(profile_path),
            str(binary),
            "--output-format",
            "stream-json",
            "--model",
            slug,
            "--agent",
            AGENT_NAME,
            "--dangerously-skip-permissions",
            "--disable-slash-commands",
            "--settings",
            settings,
            "-p",
            sandbox_context(scratch) + prompt,
        ]
        collector.record(
            "cli_agent_start",
            {
                "harness": self.harness_id,
                "argv": [*cmd[:-1], "<task prompt>"],
                "harness_version": VERSION,
                "binary_sha256": BINARY_SHA256,
                "requested_model": model,
                "agy_model": slug,
                "requested_effort": effort,
                "billing": "subscription",
                "plan": PLAN_NAME,
                "agent_md": agent_file(),
                "max_turns_note": "print mode has no step cap; the wall-clock timeout bounds the run",
            },
        )
        outcome = CliAgentOutcome(
            harness=self.harness_id,
            requested_model=model,
            session_id=None,
            harness_version=VERSION,
            auth_method="google-oauth",
            plan_name=PLAN_NAME,
            execution_boundary=(
                "macOS outer sandbox; private HOME (login keychain linked for the sign-in); "
                "custom main agent with the default file and shell tools; agy always-proceed "
                "permissions; URL, browser and MCP denied; web search absent; workspace/session "
                "writes; shared-temp/checkout/~/.gemini read deny; explicit TMPDIR context; "
                "updater blocked (pin directory not writable); shell network not isolated"
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
        stream = _Stream()
        try:
            with Path(stream_log_path).open("w") as log:
                for line in stdout:
                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if not isinstance(event, dict):
                        continue
                    stream.feed(event)
                    # agy's events carry no clock; the receive time dates tool calls.
                    log.write(json.dumps({**sanitize(event), "vb_received_at": time.time()}) + "\n")
                    log.flush()
            proc.wait()
        finally:
            timer.cancel()
            if proc.poll() is None:
                kill()
                proc.wait()
            reader.join(timeout=5)
        outcome.session_id = stream.conversation_id
        self._archive(Path(stream_log_path).parent, home, stream, errors, profile, settings)
        shutil.rmtree(scratch, ignore_errors=True)
        usage = stream.usage()
        outcome.prompt_tokens = usage["input"] + usage["cacheRead"]
        outcome.completion_tokens = usage["output"]
        outcome.cached_input_tokens = usage["cacheRead"]
        outcome.reasoning_output_tokens = usage["thinking"]
        outcome.num_turns = usage["calls"]
        served = stream.init.get("model")
        if served and served != slug:
            raise ProviderError(f"agy served {served!r}, run asked for {slug!r}")
        if stream.init and stream.init.get("agent") != AGENT_NAME:
            raise ProviderError(f"agy ran agent {stream.init.get('agent')!r}, not {AGENT_NAME!r}")
        outcome.reported_model = model if served == slug else None
        outcome.model_identity_confidence = (
            "harness-recorded" if served == slug else "requested-only"
        )
        result = stream.result or {}
        status = result.get("status")
        collector.record("agy_usage", {**usage, "result_status": status, "tools": stream.tools})
        if stream.text:
            collector.record("assistant_text", {"text": "".join(stream.text)})
        outside = sorted(set(stream.tools) - ALLOWED_TOOLS)
        if outside:
            raise ProviderError(
                f"agy called tools outside the allowlist ({', '.join(outside)}); "
                "this run is not a result"
            )
        if stream.result is not None:
            reported = step_usage(result.get("usage") or {})
            summed = {k: usage[k] for k in reported}
            if reported != summed:
                raise ProviderError(f"agy usage mismatch: steps {summed}, result {reported}")
        outcome.finished = bool(
            status == "SUCCESS" and proc.returncode == 0 and not outcome.timed_out
        )
        outcome.subtype = "completed" if outcome.finished else f"exit-{proc.returncode}"
        if outcome.finished and not usage["calls"]:
            # A run that answered without a usage receipt cannot be audited and must not be scored.
            raise ProviderError("agy run has no auditable usage: no step usage in the stream")
        if not (outcome.timed_out or outcome.finished):
            detail = ("".join(errors) + "\n" + json.dumps(result))[-2000:]
            infra = any(marker.lower() in detail.lower() for marker in INFRA_MARKERS)
            raise ProviderError(
                f"agy {'service or quota failure' if infra else 'failed'}, "
                f"exit={proc.returncode}, status={status}: {detail[:500]}"
            )
        collector.record("cli_agent_result", outcome.summary())
        return outcome

    @staticmethod
    def _archive(
        run_dir: Path,
        home: Path,
        stream: _Stream,
        errors: list[str],
        profile: str,
        settings: str,
    ) -> None:
        """Keep the run's conversation, brain artifacts and logs; nothing else from HOME.

        Only named paths are copied: agy can fall back to a file for sign-in tokens
        when a keychain write fails, and that file must never reach a run directory.
        """
        archive = run_dir / "agy-session"
        archive.mkdir(exist_ok=True)
        state = home / ".gemini/antigravity-cli"
        cid = stream.conversation_id
        if cid and re.fullmatch(r"[0-9a-f-]{36}", cid):
            db = state / "conversations" / f"{cid}.db"
            if db.is_file():
                shutil.copy2(db, archive / db.name)
            brain = state / "brain" / cid
            if brain.is_dir():
                for path in sorted(p for p in brain.rglob("*") if p.is_file()):
                    target = archive / "brain" / path.relative_to(brain)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text(str(sanitize(path.read_text(errors="replace"))))
        logs = [state / "cli.log", *sorted((state / "log").glob("*.log"))]
        for path in (p for p in logs if p.is_file()):
            target = archive / "log" / path.name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(str(sanitize(path.read_text(errors="replace"))))
        (archive / "stderr.txt").write_text(str(sanitize("".join(errors))))
        (archive / "boundary.sb").write_text(profile)
        (archive / "settings.json").write_text(settings)
        (archive / f"{AGENT_NAME}.md").write_text(agent_file())
        (archive / "init-tools.json").write_text(json.dumps(stream.init.get("tools") or []))
