"""Mirror CLI adapter: Reflection's own coding agent, API-key billed, audited usage.

Mirror (``reflection-mirror`` on PyPI, Apache-2.0) is Reflection AI's coding
harness for Beam. VulcanBench runs its one-shot ``exec`` mode against a pinned
install, inside the same macOS outer sandbox as Muse Code, Pi and DeepSeek
Harness, with a private Mirror home per run so no saved setting, credential,
memory or prior conversation leaks in.

Run settings (owner decisions 2026-10-23, docs/DECISIONS.md):

- Mirror's headless mode refuses to start until its usage notice is
  acknowledged and the folder is trusted. The owner approved writing both into
  each run's private Mirror home (never the user's own ~/.mirror), for that
  run's workspace only.
- Usage analytics are off (``"analytics": false``). Analytics carry no
  content, but the board keeps vendor telemetry off where a switch exists.
- Web tools stay off: Mirror only exposes them when a search provider key
  (You.com, Tavily) is in the environment, and the run environment carries none.
- Tool approval is automatic (``--approval-mode auto``); Mirror's own sandbox
  workspaces need a microsandbox VM and are not used, so the outer sandbox is
  the boundary, as for the other CLI harnesses.
- Updates are off (``MIRROR_SKIP_UPDATE=1`` and ``--no-update``).

Token use is read from Mirror's conversation database (``--database``): each
stored ``AgentMessage`` is decoded with Mirror's own protobuf schema, and every
assistant chat record contributes its ``usage``. Reflection publishes no price
for Beam yet, so runs are unpriced unless the price table gains an entry.

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
from harness.redaction import sanitize

VERSION = "0.1.1"
MODELS = {"Beam-1.0-501B-A23B": "Beam-1.0-501B-A23B"}
EFFORTS = frozenset({"low", "medium", "high", "xhigh", "max"})
API_KEY_ENV = "REFLECTION_API_KEY"


def tree_sha256(root: Path) -> str:
    """Content hash of every file under ``root``, by sorted relative path, skipping bytecode caches."""
    digest = hashlib.sha256()
    for path in sorted(p for p in root.rglob("*") if p.is_file() and not p.is_symlink()):
        rel = path.relative_to(root)
        if "__pycache__" in rel.parts:
            continue
        with path.open("rb") as source:
            digest.update(
                rel.as_posix().encode() + b"\0" + hashlib.file_digest(source, "sha256").digest()
            )
    return digest.hexdigest()


def pinned_install() -> tuple[Path, Path, str]:
    """The ``mirror`` launcher, the installed package directory and its verified tree hash."""
    prefix = Path(os.environ.get("VULCANBENCH_MIRROR_PREFIX", ""))
    expected = os.environ.get("VULCANBENCH_MIRROR_SHA256", "")
    if not str(prefix) or not prefix.is_absolute() or not prefix.is_dir():
        raise ProviderError("Set VULCANBENCH_MIRROR_PREFIX to the absolute Mirror install prefix")
    launcher = prefix / "bin/mirror"
    packages = sorted(prefix.glob("lib/python3*/site-packages"))
    if not launcher.is_file() or len(packages) != 1:
        raise ProviderError("No single Mirror install under VULCANBENCH_MIRROR_PREFIX")
    dist = packages[0] / f"reflection_mirror-{VERSION}.dist-info"
    if not dist.is_dir():
        raise ProviderError(f"Mirror {VERSION} is not the installed version")
    actual = tree_sha256(packages[0] / "mirror")
    if len(expected) != 64 or actual != expected:
        raise ProviderError("Mirror install tree SHA256 mismatch")
    return launcher.resolve(), packages[0], actual


def mirror_home(scratch: Path, workspace: Path) -> Path:
    """A private Mirror home: notices acknowledged for this run's workspace only, analytics off."""
    home = scratch / "mirror-home"
    (home / ".mirror").mkdir(parents=True)
    settings = {
        "usage_warning_acknowledged": True,
        "trusted_directories": [str(workspace.resolve())],
        "analytics": False,
    }
    (home / ".mirror/config.json").write_text(json.dumps(settings))
    return home


def sandbox_context(scratch: Path) -> str:
    return (
        "Execution environment: an outer OS sandbox is active. Write temporary files inside "
        f"the workspace or $TMPDIR ({scratch / 'tmp'}). Global /tmp and /var/tmp are not "
        "available. Other runs' temporary files and the benchmark checkout are inaccessible. "
        'For mktemp, supply an explicit template such as "$TMPDIR/probe.XXXXXX".\n\n'
    )


_DECODER = r"""
import json, sqlite3, sys
from mirror.protocol import agent_pb2
rows = []
db = sqlite3.connect(f"file:{sys.argv[1]}?mode=ro", uri=True)
for seq, payload in db.execute("select sequence, payload from react_entry order by session_id, sequence"):
    m = agent_pb2.AgentMessage()
    m.ParseFromString(payload)
    row = {"sequence": seq, "id": m.id, "time": m.timestamp.seconds + m.timestamp.nanos / 1e9}
    if m.HasField("inference"):
        row["model"] = m.inference.model
        row["effort"] = m.inference.reasoning_effort
    if m.HasField("chat"):
        c = m.chat
        row["role"] = next((r for r in ("system", "user", "assistant", "tool") if c.HasField(r)), None)
        if c.HasField("usage"):
            row["usage"] = {"prompt": c.usage.prompt_tokens, "completion": c.usage.completion_tokens,
                            "cached": c.usage.cached_tokens, "cache_write": c.usage.cache_write_tokens}
        if c.HasField("assistant"):
            row["text"] = c.assistant.content
    if m.HasField("log"):
        row["log"] = {"level": m.log.level, "text": m.log.text}
    rows.append(row)
print(json.dumps(rows))
"""


def decode_database(python: Path, packages: Path, database: Path) -> list[dict[str, Any]]:
    """Decode Mirror's conversation database with Mirror's own pinned protobuf schema."""
    proc = subprocess.run(
        [str(python), "-I", "-c", _DECODER, str(database)],
        capture_output=True,
        text=True,
        timeout=300,
        env={"PYTHONPATH": str(packages), "PATH": os.environ.get("PATH", "")},
        check=False,
    )
    if proc.returncode != 0:
        raise ProviderError(f"Cannot decode Mirror database: {proc.stderr[-500:]}")
    rows: list[dict[str, Any]] = json.loads(proc.stdout)
    return rows


def collect_usage(rows: list[dict[str, Any]], model: str) -> dict[str, Any]:
    """Sum the usage on every assistant record; every recorded inference must name ``model``."""
    totals = dict(prompt=0, completion=0, cached=0, cache_write=0, calls=0, errors=0)
    for row in rows:
        if "model" in row and row["model"] != model:
            raise ProviderError(f"Mirror model mismatch: {row['model']!r}")
        if row.get("log", {}).get("level") == "error":
            totals["errors"] += 1
        usage = row.get("usage")
        if usage is None:
            continue
        for name, value in usage.items():
            if type(value) is not int or value < 0:
                raise ProviderError("Invalid Mirror token count")
            totals[name] += value
        if usage["cached"] > usage["prompt"]:
            raise ProviderError("Inconsistent Mirror token subsets")
        totals["calls"] += 1
    return totals


class MirrorAdapter:
    harness_id = "mirror"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            self.harness_id,
            "Mirror",
            "mirror",
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
        del kwargs, max_turns, priced_spec
        if agent_container:
            raise ProviderError("Mirror runs locally under the outer sandbox, not in a container")
        if network:
            raise ProviderError("Mirror runs keep web tools off; network tasks are not supported")
        if max_run_cost is not None:
            raise ProviderError("Mirror runs are bounded by wall clock, not a live cost cap")
        if model not in MODELS:
            raise ProviderError(f"Mirror model {model!r} is not configured for VulcanBench")
        if effort not in EFFORTS:
            raise ProviderError(
                "Mirror requires an explicit low, medium, high, xhigh or max effort"
            )
        if timeout_s is None or timeout_s <= 0:
            raise ProviderError("Mirror requires a positive wall-clock timeout")
        key = os.environ.get(API_KEY_ENV)
        if not key:
            raise ProviderError(f"Mirror needs {API_KEY_ENV}")
        launcher, packages, tree_sha = pinned_install()
        api_model = MODELS[model]
        workspace = Path(workspace).resolve()
        scratch = Path(tempfile.mkdtemp(prefix="vb-mirror-session-")).resolve()
        (scratch / "tmp").mkdir()
        home = mirror_home(scratch, workspace)
        database = scratch / "state.db"
        env = _subscription_env()
        env.update(
            {
                API_KEY_ENV: key,
                "HOME": str(home),
                "MIRROR_SKIP_UPDATE": "1",
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
            str(launcher),
            "--no-update",
            "--api",
            "reflection",
            "--model",
            api_model,
            "--reasoning-effort",
            effort,
            "--approval-mode",
            "auto",
            "--database",
            str(database),
            "exec",
            "-",
        ]
        collector.record(
            "cli_agent_start",
            {
                "harness": self.harness_id,
                "argv": cmd,
                "harness_version": VERSION,
                "install_tree_sha256": tree_sha,
                "requested_model": model,
                "requested_effort": effort,
                "billing": "api",
                "mirror_settings": json.loads((home / ".mirror/config.json").read_text())
                | {"trusted_directories": ["<run workspace>"]},
                "max_turns_note": "Mirror has no step cap by default; the wall-clock timeout bounds the run",
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
                "macOS outer sandbox; Mirror approvals automatic, its sandbox workspaces unused; "
                "workspace/session writes; shared-temp/checkout read deny; explicit TMPDIR context; "
                "web tools absent; analytics off; private Mirror home per run; shell network not "
                "isolated; API key in the agent environment"
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
        assert proc.stdin is not None and proc.stdout is not None and proc.stderr is not None
        # The task goes in on stdin so it never appears in the process table.
        proc.stdin.write(sandbox_context(scratch) + prompt)
        proc.stdin.close()
        errors: list[str] = []
        stdout_parts: list[str] = []
        out, err = proc.stdout, proc.stderr

        def drain_err() -> None:
            for line in err:
                errors.append(line)

        def drain_out() -> None:
            for line in out:
                stdout_parts.append(line)

        readers = [threading.Thread(target=f, daemon=True) for f in (drain_err, drain_out)]
        for reader in readers:
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
        try:
            proc.wait()
        finally:
            timer.cancel()
            if proc.poll() is None:
                kill()
                proc.wait()
            for reader in readers:
                reader.join(timeout=5)
        stderr_text = "".join(errors)
        session = next(
            (
                line.split("session", 1)[1].strip()
                for line in errors
                if line.startswith("mirror: session")
            ),
            None,
        )
        outcome.session_id = session
        rows = (
            decode_database(launcher.parent / "python", packages, database)
            if database.exists()
            else []
        )
        if stream_log_path:
            log = Path(stream_log_path)
            with log.open("w") as sink:
                for row in rows:
                    sink.write(json.dumps(sanitize(row)) + "\n")
            archive = log.parent / "mirror-session"
            archive.mkdir(exist_ok=True)
            (archive / "stdout.txt").write_text(str(sanitize("".join(stdout_parts))))
            (archive / "stderr.txt").write_text(str(sanitize(stderr_text)))
            (archive / "boundary.sb").write_text(profile)
            for path in (
                sorted((home / ".mirror/logs").rglob("*"))
                if (home / ".mirror/logs").exists()
                else []
            ):
                if path.is_file():
                    target = archive / "logs" / path.relative_to(home / ".mirror/logs")
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text(str(sanitize(path.read_text(errors="replace"))))
        for row in rows:
            if row.get("text"):
                collector.record("assistant_text", {"text": row["text"]})
        usage = collect_usage(rows, api_model)
        outcome.prompt_tokens = usage["prompt"]
        outcome.completion_tokens = usage["completion"]
        outcome.cached_input_tokens = usage["cached"]
        outcome.num_turns = usage["calls"]
        outcome.reported_model = model if usage["calls"] else None
        outcome.model_identity_confidence = (
            "harness-recorded" if usage["calls"] else "requested-only"
        )
        outcome.finished = proc.returncode == 0 and not outcome.timed_out
        outcome.subtype = "completed" if outcome.finished else f"exit-{proc.returncode}"
        collector.record("mirror_usage", usage)
        if outcome.finished and not usage["calls"]:
            raise ProviderError("Mirror run has no auditable usage")
        if not (outcome.timed_out or outcome.finished):
            raise ProviderError(f"Mirror failed, exit={proc.returncode}: {stderr_text[-500:]}")
        collector.record("cli_agent_result", outcome.summary())
        return outcome
