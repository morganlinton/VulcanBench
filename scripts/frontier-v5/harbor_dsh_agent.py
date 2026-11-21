"""DeepSeek Harness (dsh) as a Harbor agent, for Frontier v5 runs through run_gate.sh.

Harbor 0.24.0 ships no dsh agent, so run_gate.sh loads this one with
``-a harbor_dsh_agent:DeepSeekHarness`` (``scripts/frontier-v5`` on harbor's
PYTHONPATH). It mirrors the vulcanbench harness adapter
(``harness/agent/deepseek_harness.py``, docs/DECISIONS.md 2026-10-19) with the
task container as the boundary instead of the macOS outer sandbox:

- dsh is pinned to 0.2.1-rc.2, DeepSeek V4.1 Flash's Frontier v4 pin, and the
  install is refused if npm resolved any other version. The installed
  package's file hash is written to ``dsh-install.json`` for the record.
- One ``headless`` run per trial, the task on stdin, with a fresh
  ``DSH_HOME`` (no saved setting or credential) and a settings overlay that fixes model and effort, turns web
  tools off, turns session-log and telemetry upload off and keeps session
  logs uncompressed. dsh's own sandbox and approvals are off
  (``danger-full-access``): the container is the boundary.
- Reasoning effort is one of dsh's DeepSeek levels: low, high or max (the
  API has no medium or xhigh); anything else is refused before a model call.
- The API key comes from ``DEEPSEEK_API_KEY`` in harbor's environment
  (run_gate.sh passes the operator's env file with ``--env-file``) and is
  handed to the agent process only.

Records in the trial's ``agent/`` directory: ``dsh.txt`` (dsh's ``--json``
stdout), ``dsh.stderr``, ``dsh-session-logs/`` (the run's ``DSH_HOME``, so the session
logs are written straight to the host and survive a timeout kill;
``harness/agent/tool_calls.py`` reads their ``tool/call`` and ``tool/result``
events), ``vulcanbench.patch.yml`` and ``dsh-install.json``. Token use is
summed from the session logs' ``assistant/message`` receipts and retried
``assistant/attempt`` streams, as the harness adapter does; cost is left to
the importer, which prices every column the same way.

Infrastructure: an ``error`` event whose code is an account or API failure
(AUTH, QUOTA, RATE_LIMIT, SERVER, TRANSPORT, MISSING_CREDENTIAL), a plugin
that did not activate, or a finished run with no usage receipt is not the
model's work. The agent writes ``vb-infra.txt`` with a
``VULCANBENCH_INFRA:`` line for each, which run_gate.sh's infrastructure
guard matches, so the run stops and the trial is set aside.

This module imports only harbor and the standard library: it runs in
harbor's own environment, where the vulcanbench package is not installed.
"""

from __future__ import annotations

import json
import shlex
from pathlib import Path
from typing import Any

from harbor.agents.installed.base import BaseInstalledAgent, with_prompt_template
from harbor.agents.installed.node_install import nvm_node_install_snippet
from harbor.agents.options import InstalledAgentOptions
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext
from pydantic import AliasChoices, Field

VERSION = "0.2.1-rc.2"
PACKAGE = "@deepseek-ai/dsh"
PROVIDER = "deepseek-official"
#: VulcanBench model spec -> DeepSeek API model id (as in the harness adapter).
MODELS = {"deepseek-flash": "deepseek-flash", "deepseek-v4-pro": "deepseek-v4-pro"}
EFFORTS = ("low", "high", "max")
API_KEY_ENV = "DEEPSEEK_API_KEY"
#: Host the agent phase must reach; run_gate.sh adds it with --allow-agent-host.
API_HOST = "api.deepseek.com"
INFRA_CODES = ("AUTH", "QUOTA", "RATE_LIMIT", "SERVER", "TRANSPORT", "MISSING_CREDENTIAL")
INFRA_MARK = "VULCANBENCH_INFRA:"

REMOTE_DIR = "/tmp/vb-dsh"
REMOTE_PATCH = f"{REMOTE_DIR}/vulcanbench.patch.yml"
REMOTE_PROMPT = f"{REMOTE_DIR}/prompt.txt"


def benchmark_patch(model: str, effort: str) -> str:
    """The dsh settings overlay: model, effort, web tools and data uploads off."""
    return f"""# VulcanBench overlay for one dsh headless run (Frontier v5, Harbor).
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


class DshOptions(InstalledAgentOptions):
    reasoning_effort: str | None = Field(
        default=None,
        validation_alias=AliasChoices("reasoning_effort", "effort"),
        description="dsh DeepSeek reasoning level: low, high or max.",
    )
    disable_web_search: bool = Field(
        default=True, description="Web tools stay off; false is refused."
    )


def _usage(rows: list[dict[str, Any]]) -> dict[str, int]:
    """Billed tokens from dsh session events (Messages semantics: input excludes cache reads)."""
    out = dict(input=0, output=0, cache_read=0, cache_write=0, calls=0, retried=0)
    for row in rows:
        kind = row.get("type")
        if kind not in ("assistant/message", "assistant/attempt"):
            continue
        data = row.get("data") or {}
        usage = data.get("usage") if kind == "assistant/message" else None
        if usage is None:
            for rec in reversed(data.get("stream") or []):
                chunk = rec.get("chunk") if isinstance(rec, dict) else None
                if (
                    rec.get("type") == "chunk"
                    and isinstance(chunk, dict)
                    and chunk.get("type") == "usage"
                ):
                    usage = chunk.get("usage")
                    break
        if not isinstance(usage, dict):
            continue
        out["input"] += int(usage.get("inputTokens") or 0)
        out["output"] += int(usage.get("outputTokens") or 0)
        out["cache_read"] += int(usage.get("cacheReadTokens") or 0)
        out["cache_write"] += int(usage.get("cacheWriteTokens") or 0)
        out["calls" if kind == "assistant/message" else "retried"] += 1
    return out


def _jsonl(path: Path) -> list[dict[str, Any]]:
    rows = []
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
        return rows
    for line in lines:
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(row, dict):
            rows.append(row)
    return rows


def infra_findings(
    stdout_rows: list[dict[str, Any]], stderr: str, usage: dict[str, int], finished: bool
) -> list[str]:
    """Reasons a trial is an infrastructure failure, not the model's work."""
    found = []
    for ev in stdout_rows:
        if ev.get("type") != "error":
            continue
        text = json.dumps(ev)
        for code in INFRA_CODES:
            if f"{code}:" in text or f'"{code}"' in text:
                found.append(f"dsh error event {code}")
                break
    if "did not activate" in stderr:
        found.append("a dsh plugin did not activate (receipts may be missing)")
    if finished and not usage["calls"]:
        found.append("dsh finished with no usage receipt")
    return found


class DeepSeekHarness(BaseInstalledAgent):
    options_model = DshOptions
    options: DshOptions

    _OUTPUT = "dsh.txt"
    _STDERR = "dsh.stderr"
    _SESSIONS = "dsh-session-logs"

    @staticmethod
    def name() -> str:
        return "dsh"

    def get_version_command(self) -> str | None:
        return (
            f". ~/.nvm/nvm.sh; node -p \"require('$(npm root -g)/{PACKAGE}/package.json').version\""
        )

    def _effort(self) -> str:
        effort = self.options.reasoning_effort
        if effort not in EFFORTS:
            raise ValueError(f"dsh needs an explicit effort in {EFFORTS}, got {effort!r}")
        return effort

    def _api_model(self) -> str:
        name = (self.model_name or "").split("/")[-1]
        if name not in MODELS:
            raise ValueError(
                f"dsh model {self.model_name!r} is not configured (one of {sorted(MODELS)})"
            )
        return MODELS[name]

    def _check_options(self) -> None:
        if (self.options.version or VERSION) != VERSION:
            raise ValueError(f"dsh is pinned to {VERSION}, got version={self.options.version}")
        if not self.options.disable_web_search:
            raise ValueError("dsh runs keep web tools off (disable_web_search=true)")
        self._effort()
        self._api_model()

    async def install(self, environment: BaseEnvironment) -> None:
        self._check_options()
        await self.ensure_system_dependencies(environment, ("curl",))
        root = f"$(npm root -g)/{PACKAGE}"
        await self.exec_as_agent(
            environment,
            command=(
                "set -euo pipefail; "
                f"{nvm_node_install_snippet()} && "
                f"npm install -g {PACKAGE}@{VERSION} && "
                f'test "$(node -p "require(\'{root}/package.json\').version")" = {VERSION} && '
                f"mkdir -p {self.environment_logs_dir} && "
                f"(cd {root} && find . -type f ! -path './node_modules/*' -print0 | sort -z "
                "| xargs -0 sha256sum | sha256sum | cut -d' ' -f1) > /tmp/vb-dsh-tree && "
                f'printf \'{{"package": "{PACKAGE}", "version": "{VERSION}", '
                '"node": "%s", "package_tree_sha256": "%s"}\\n\' '
                f'"$(node -v)" "$(cat /tmp/vb-dsh-tree)" > {self.environment_logs_dir}/dsh-install.json'
            ),
        )

    @with_prompt_template
    async def run(
        self, instruction: str, environment: BaseEnvironment, context: AgentContext
    ) -> None:
        self._check_options()
        key = self._get_env(API_KEY_ENV)
        if not key:
            raise ValueError(
                f"dsh needs {API_KEY_ENV} in harbor's environment (run_gate.sh --env-file)"
            )
        patch = benchmark_patch(self._api_model(), self._effort())
        logs = str(self.environment_logs_dir)
        # DSH_HOME lives in the agent log directory, so the session logs are on
        # the host as they are written and survive a timeout kill.
        home = f"{logs}/{self._SESSIONS}"
        await self.exec_as_agent(
            environment,
            command=f"mkdir -p {REMOTE_DIR} {shlex.quote(home)} && chmod 700 {REMOTE_DIR}",
        )
        await self._upload_config_text(
            environment, content=patch, remote_path=REMOTE_PATCH, filename="patch.yml"
        )
        await self._upload_config_text(
            environment, content=instruction, remote_path=REMOTE_PROMPT, filename="prompt.txt"
        )
        q = shlex.quote
        await self.exec_as_agent(
            environment,
            command=(
                f". ~/.nvm/nvm.sh; mkdir -p {q(logs)} && cp {REMOTE_PATCH} {q(logs)}/vulcanbench.patch.yml; "
                f'node "$(npm root -g)/{PACKAGE}/lib/bin.js" --profile headless --patch {REMOTE_PATCH} --json - '
                f"< {REMOTE_PROMPT} 2> {q(logs)}/{self._STDERR} | tee {q(logs)}/{self._OUTPUT}"
            ),
            env={
                API_KEY_ENV: key,
                "DSH_HOME": home,
                "DSH_PERMISSION_MODE": "danger-full-access",
                "DSH_TELEMETRY_MODE": "DISABLED",
            },
        )

    def populate_context_post_run(self, context: AgentContext) -> None:
        stdout_rows = _jsonl(self.logs_dir / self._OUTPUT)
        rows: list[dict[str, Any]] = []
        for p in sorted((self.logs_dir / self._SESSIONS).rglob("*.jsonl")):
            rows += _jsonl(p)
        usage = _usage(rows)
        context.n_input_tokens = usage["input"] + usage["cache_read"] + usage["cache_write"]
        context.n_cache_tokens = usage["cache_read"]
        context.n_output_tokens = usage["output"]
        context.metadata = {
            "dsh_version": VERSION,
            "calls": usage["calls"],
            "retried_attempts": usage["retried"],
            "cache_write_tokens": usage["cache_write"],
        }
        try:
            stderr = (self.logs_dir / self._STDERR).read_text(errors="replace")
        except OSError:
            stderr = ""
        finished = any(ev.get("type") == "final" for ev in stdout_rows)
        findings = infra_findings(stdout_rows, stderr, usage, finished)
        if findings:
            (self.logs_dir / "vb-infra.txt").write_text(
                "".join(f"{INFRA_MARK} {f}\n" for f in findings)
            )
