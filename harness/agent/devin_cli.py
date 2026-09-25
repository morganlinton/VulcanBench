"""Devin CLI (Cognition) subscription adapter: SWE-2 and friends through Devin.

``devin:<model>`` runs a task in Devin's local terminal agent (``devin -p``,
print mode) on the host workspace, billed to the Devin account the CLI is
signed in to. Everything downstream (diff, verifier, evaluator, scoring) stays
in VulcanBench, as for every other subscription harness.

Why a separate module: like Muse Code this adapter is imported lazily by
``cli_agents`` so adapter registration cannot cycle.

What is specific to Devin (verified on ``devin`` 3000.10.31, 2026-09-18):

- **Effort is not a flag; it is the last token of the model id.** The account
  catalog (``devin models list --format json``) lists one ``model_uid`` per
  effort variant: ``swe-2-medium`` / ``swe-2-high`` / ``swe-2-max`` for SWE-2,
  ``claude-fable-5-1-xhigh`` and so on for other families. The adapter takes
  the family (``--model swe-2``) plus VulcanBench's ``--effort`` and composes
  the uid, then refuses any uid the catalog does not list rather than letting
  the CLI fall back to a default variant (the CLI logs "did not resolve to an
  available, allowed model; starting on the default" and carries on, which a
  benchmark must never tolerate).
- **Print mode prints only the final text.** The structured record (every
  message, tool call with its arguments, and a per-request token receipt with
  the model that actually generated the response) lives in the CLI's sqlite
  session store, ``$XDG_DATA_HOME/devin/cli/sessions.db``. After the run the
  adapter locates the session by workspace directory, copies its rows into
  ``<run_dir>/devin-session/``, appends normalized message and tool-call
  records to ``cli-agent-stream.jsonl`` for the integrity audit, and sums
  usage into the outcome. Assistant messages are stored twice per request
  (identical receipts), so usage is deduplicated by ``request_id`` and any
  conflicting copy is an error, never a double count.
- **The served model is proven, not assumed.** Every assistant receipt names
  ``generation_model``; a run whose receipts disagree with the requested uid
  fails closed.
- **Web tools are disabled through the per-run config** (``disabled_tools``)
  plus a permission deny, never the user's own config: the run gets its own
  ``XDG_CONFIG_HOME`` so the user's rules, skills and MCP servers stay out of
  the benchmark context, while the login (``credentials.toml`` under the data
  home) is shared. Shell commands run on the host with no kernel sandbox, so
  the workspace perimeter plus the audit is the containment, as for ZCode.
- **No API-equivalent price.** SWE-2 is a Devin-only model with no public
  per-token rate (the catalog lists it as cost tier "Free" through
  2026-10-10), so ``devin:`` specs stay unpriced: tokens and Devin's own
  credit/ACU counters are recorded, ``cost_usd`` is ``None`` and the
  economics receipt says "unavailable" rather than inventing a number.
"""

from __future__ import annotations

import contextlib
import json
import os
import re
import shutil
import sqlite3
import subprocess
import tempfile
import threading
import time
import tomllib
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from harness.agent.cli_agents import (
    CliAgentOutcome,
    HarnessCapabilities,
    HarnessPreflight,
    SubscriptionQuotaError,
    _kill_process_group,
    _require_subscription,
    _subscription_env,
    _version,
)
from harness.agent.providers import ProviderError
from harness.redaction import sanitize

#: First CLI release with ``devin models list``, ``disabled_tools`` and the
#: current print-mode surface. The May 2026 build (2026.5.6) predates SWE-2.
MIN_VERSION = (3000, 10, 21)

#: Devin's web tools as named in its tool list and permission rules, plus the
#: browser preview pair (a headless browser the agent can point at any URL).
DEVIN_WEB_TOOLS = ("webfetch", "web_search", "browser_preview", "close_browser_preview")

#: Effort tokens the catalog uses as the last segment of a model uid.
EFFORT_TOKENS = frozenset({"minimal", "low", "medium", "high", "xhigh", "max"})

#: Speed suffixes a uid may carry after the effort token (``-high-fast``).
_SPEED_SUFFIXES = ("-fast", "-priority")

_LIMIT_PATTERN = re.compile(
    r"usage limit|limit reached|quota|rate limit|out of credits|too many requests|\b429\b",
    re.I,
)
_AUTH_PATTERN = re.compile(
    r"not authorized|unauthori[sz]ed|not logged in|please log in|sign in", re.I
)
_VERSION_RE = re.compile(r"(\d+)\.(\d+)\.(\d+)")


def devin_data_home() -> Path:
    """Devin's data root (``$XDG_DATA_HOME/devin``): login and session store."""
    base = os.environ.get("XDG_DATA_HOME")
    root = Path(base).expanduser() if base else Path.home() / ".local" / "share"
    return root / "devin"


def devin_sessions_db() -> Path:
    return devin_data_home() / "cli" / "sessions.db"


def devin_credentials_path() -> Path:
    return devin_data_home() / "credentials.toml"


def devin_user_config_path() -> Path:
    base = os.environ.get("XDG_CONFIG_HOME")
    root = Path(base).expanduser() if base else Path.home() / ".config"
    return root / "devin" / "config.json"


def parse_version(text: str | None) -> tuple[int, int, int] | None:
    if not text:
        return None
    m = _VERSION_RE.search(text)
    if not m:
        return None
    return int(m.group(1)), int(m.group(2)), int(m.group(3))


def _has_login(path: Path) -> bool:
    """Presence-only check of the CLI's credential file; the value is never read out."""
    try:
        with path.open("rb") as f:
            data = tomllib.load(f)
    except (OSError, tomllib.TOMLDecodeError):
        return False
    return bool(str(data.get("windsurf_api_key") or "").strip())


def _auth_status(devin_bin: str) -> str | None:
    """First line of ``devin auth status`` (no model call), or None if it cannot run."""
    try:
        proc = subprocess.run(
            [devin_bin, "auth", "status"],
            capture_output=True,
            text=True,
            timeout=30,
            env=_subscription_env(),
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    text = (proc.stdout or proc.stderr).strip()
    return text.splitlines()[0].strip() if text else None


def devin_preflight(devin_bin: str = "devin") -> HarnessPreflight:
    """Readiness receipt for the Devin CLI without launching a model run."""
    version = _version(devin_bin)
    if version is None:
        return HarnessPreflight(
            harness="devin",
            available=False,
            version=None,
            authenticated=False,
            auth_mode=None,
            detail=(
                f"{devin_bin!r} not found on PATH; install with "
                "`brew install --cask devin-cli` and run `devin auth login`"
            ),
        )
    parsed = parse_version(version)
    if parsed is None or parsed < MIN_VERSION:
        wanted = ".".join(str(n) for n in MIN_VERSION)
        return HarnessPreflight(
            harness="devin",
            available=True,
            version=version,
            authenticated=False,
            auth_mode=None,
            detail=(
                f"devin {version!r} is older than {wanted}; it lacks `devin models list` and "
                "per-run `disabled_tools`. Update with `brew install --cask devin-cli` "
                "(the 2026.x builds' own `devin update` channel is dead)"
            ),
        )
    creds = devin_credentials_path()
    if not _has_login(creds):
        return HarnessPreflight(
            harness="devin",
            available=True,
            version=version,
            authenticated=False,
            auth_mode=None,
            detail=f"signed out (no login in {creds}); run `{devin_bin} auth login`",
        )
    status = _auth_status(devin_bin)
    if status is not None and not status.lower().startswith("logged in"):
        return HarnessPreflight(
            harness="devin",
            available=True,
            version=version,
            authenticated=False,
            auth_mode=None,
            detail=f"`{devin_bin} auth status`: {status[:120]}; run `{devin_bin} auth login`",
        )
    detail = "Devin account login (credentials.toml)"
    if status:
        detail += f"; auth status: {status[:80]}"
    else:
        detail += "; live auth status unavailable, verified at run start"
    return HarnessPreflight(
        harness="devin",
        available=True,
        version=version,
        authenticated=True,
        auth_mode="subscription",
        plan_name=None,
        detail=detail,
    )


_CATALOG_CACHE: dict[str, dict[str, dict[str, Any]]] = {}


def devin_model_catalog(devin_bin: str = "devin") -> dict[str, dict[str, Any]]:
    """``model_uid`` -> variant record from ``devin models list --format json``.

    Cached per process: the catalog is served by the account and does not
    change within a sweep, and every run consults it before launching.
    """
    cached = _CATALOG_CACHE.get(devin_bin)
    if cached is not None:
        return cached
    try:
        proc = subprocess.run(
            [devin_bin, "models", "list", "--format", "json"],
            capture_output=True,
            text=True,
            timeout=90,
            env=_subscription_env(),
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ProviderError(f"could not read the Devin model catalog: {exc}") from exc
    if proc.returncode != 0:
        raise ProviderError(
            f"`{devin_bin} models list` exited {proc.returncode}: {proc.stderr.strip()[:300]}"
        )
    try:
        data = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise ProviderError("Devin model catalog is not JSON") from exc
    catalog: dict[str, dict[str, Any]] = {}
    for family in data.get("families", []) if isinstance(data, dict) else []:
        if not isinstance(family, dict):
            continue
        for variant in family.get("variants", []):
            if not isinstance(variant, dict) or not variant.get("model_uid"):
                continue
            uid = str(variant["model_uid"])
            catalog[uid] = {
                "family_uid": family.get("family_uid"),
                "family_label": family.get("family_label"),
                "label": variant.get("label"),
                "cost_tier": variant.get("cost_tier"),
                "cost_summary": variant.get("cost_summary"),
                "max_context_tokens": variant.get("max_context_tokens"),
            }
    if not catalog:
        raise ProviderError("Devin model catalog listed no models for this account")
    _CATALOG_CACHE[devin_bin] = catalog
    return catalog


def reset_catalog_cache() -> None:
    _CATALOG_CACHE.clear()


def effort_from_uid(uid: str) -> str | None:
    """The effort token baked into a catalog uid (``swe-2-high-fast`` -> ``high``)."""
    base = uid
    for suffix in _SPEED_SUFFIXES:
        if base.endswith(suffix):
            base = base[: -len(suffix)]
    token = base.rsplit("-", 1)[-1]
    return token if token in EFFORT_TOKENS else None


def resolve_model_uid(model: str, effort: str | None, catalog: dict[str, dict[str, Any]]) -> str:
    """Compose ``<family>-<effort>`` and prove the catalog lists it.

    ``effort`` is the value ``harness.effort`` already mapped (``xhigh`` for
    extra-high). Without an effort the model must itself be an exact catalog
    uid: a bare family alias (``swe``, ``swe-2``) would let the CLI choose a
    default variant, which is not a known effort point.
    """
    model = model.strip()
    if effort:
        uid = f"{model}-{effort}"
        if uid in catalog:
            return uid
        siblings = sorted(
            u
            for u, rec in catalog.items()
            if rec.get("family_uid") == model or u.startswith(f"{model}-")
        )
        raise ProviderError(
            f"Devin's catalog has no {uid!r}: effort {effort!r} is not a variant of "
            f"{model!r} (available: {', '.join(siblings) or 'none'})"
        )
    if model in catalog:
        return model
    raise ProviderError(
        f"{model!r} is not an exact Devin model uid; Devin selects effort through the model "
        "id, so pass --effort (SWE-2: medium|high|max) or name a catalog uid such as "
        "'swe-2-high'"
    )


def build_run_config(
    user_config: dict[str, Any] | None, model_uid: str, network: bool
) -> dict[str, Any]:
    """The per-run user config handed to ``devin --config``.

    Only the org binding is carried over from the user's own config; model,
    web tools and auto-update are pinned per run.
    """
    config: dict[str, Any] = {"version": 1}
    org = user_config.get("devin") if isinstance(user_config, dict) else None
    if isinstance(org, dict) and org.get("org_id"):
        config["devin"] = {"org_id": org["org_id"]}
    config["agent"] = {"model": model_uid}
    config["auto_update"] = False
    # First-run marker: without it print mode prefixes its output with the
    # welcome banner (the run's config home is always fresh).
    config["shell"] = {"setup_complete": True}
    # Print mode refuses an untrusted directory by default; the workspace is a
    # fresh tmp perimeter VulcanBench created, so trust is asserted per run.
    config["respect_workspace_trust"] = False
    if not network:
        config["disabled_tools"] = list(DEVIN_WEB_TOOLS)
        config["permissions"] = {"deny": [*DEVIN_WEB_TOOLS, "Fetch(*)"]}
    return config


def _read_json(path: Path) -> dict[str, Any] | None:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def _workspace_forms(workspace: Path) -> list[str]:
    forms = {str(workspace), str(workspace.resolve()), os.path.realpath(workspace)}
    return sorted(forms)


def find_session(db_path: Path, workspace: Path, started_s: int) -> str | None:
    """Newest session the CLI recorded for this workspace since ``started_s``."""
    if not db_path.is_file():
        return None
    forms = _workspace_forms(workspace)
    placeholders = ",".join("?" for _ in forms)
    try:
        con = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    except sqlite3.Error:
        return None
    try:
        row = con.execute(
            f"select id from sessions where working_directory in ({placeholders}) "
            "and created_at >= ? order by created_at desc, rowid desc limit 1",
            [*forms, started_s],
        ).fetchone()
    except sqlite3.Error:
        return None
    finally:
        con.close()
    return str(row[0]) if row else None


@dataclass
class _Usage:
    prompt: int = 0
    completion: int = 0
    cached: int = 0
    cache_write: int = 0
    requests: int = 0


def _fold_receipt(
    message: dict[str, Any],
    seen: dict[str, dict[str, int]],
    usage: _Usage,
    models: Counter[str],
) -> None:
    metadata = message.get("metadata")
    if not isinstance(metadata, dict):
        return
    metrics = metadata.get("metrics")
    if not isinstance(metrics, dict) or "input_tokens" not in metrics:
        return
    receipt: dict[str, int] = {}
    for name in ("input_tokens", "output_tokens", "cache_read_tokens", "cache_creation_tokens"):
        value = metrics.get(name)
        if value is None:
            value = 0
        if type(value) is not int or value < 0:
            raise ProviderError(f"invalid Devin token count for {name}: {value!r}")
        receipt[name] = value
    key = str(metadata.get("request_id") or message.get("message_id") or "")
    if not key:
        raise ProviderError("Devin assistant receipt has no request or message identity")
    if key in seen:
        if seen[key] != receipt:
            raise ProviderError(f"conflicting Devin token receipts for request {key}")
        return
    seen[key] = receipt
    usage.requests += 1
    usage.prompt += (
        receipt["input_tokens"] + receipt["cache_read_tokens"] + receipt["cache_creation_tokens"]
    )
    usage.completion += receipt["output_tokens"]
    usage.cached += receipt["cache_read_tokens"]
    usage.cache_write += receipt["cache_creation_tokens"]
    served = metadata.get("generation_model")
    if isinstance(served, str) and served:
        models[served] += 1


def _normalized_tool_call(call: dict[str, Any]) -> dict[str, Any]:
    """Audit-friendly tool-call line: call id, name, verbatim args, plus the
    ``path``/``command`` keys the filesystem audit gates on."""
    args = call.get("arguments")
    if isinstance(args, str):
        with contextlib.suppress(json.JSONDecodeError):
            args = json.loads(args)
    record: dict[str, Any] = {
        "toolCallId": call.get("id"),
        "name": call.get("name"),
        "input": args,
    }
    if isinstance(args, dict):
        for key in ("file_path", "path", "target_file", "target_directory"):
            value = args.get(key)
            if isinstance(value, str) and "path" not in record:
                record["path"] = value
        command = args.get("command")
        if isinstance(command, str):
            record["command"] = command
    return record


def harvest_session(  # noqa: PLR0912, PLR0915, linear copy + fold loop
    db_path: Path,
    session_id: str,
    run_dir: Path,
    stream_f: Any,
    outcome: CliAgentOutcome,
) -> dict[str, Any]:
    """Fold the CLI's session store into the run artifacts and the outcome."""
    dest = run_dir / "devin-session"
    dest.mkdir(exist_ok=True)
    usage = _Usage()
    seen: dict[str, dict[str, int]] = {}
    models: Counter[str] = Counter()
    seen_calls: set[str] = set()
    session_meta: dict[str, Any] = {}
    con = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    con.row_factory = sqlite3.Row
    try:
        session = con.execute("select * from sessions where id = ?", (session_id,)).fetchone()
        if session is not None:
            record = {k: session[k] for k in session.keys()}  # noqa: SIM118, sqlite3.Row
            raw_meta = record.get("metadata")
            if isinstance(raw_meta, str):
                with contextlib.suppress(json.JSONDecodeError):
                    parsed = json.loads(raw_meta)
                    if isinstance(parsed, dict):
                        session_meta = parsed
            record.pop("cogs_json", None)  # system prompt scaffolding, large
            (dest / "session.json").write_text(
                json.dumps(sanitize(record), indent=1) + "\n", encoding="utf-8"
            )
        with (dest / "messages.jsonl").open("w", encoding="utf-8") as mf:
            rows = con.execute(
                "select row_id, node_id, parent_node_id, chat_message, created_at, metadata "
                "from message_nodes where session_id = ? order by row_id",
                (session_id,),
            ).fetchall()
            for row in rows:
                try:
                    message = json.loads(row["chat_message"])
                except (TypeError, json.JSONDecodeError):
                    message = {"raw": str(row["chat_message"])[:2000]}
                if not isinstance(message, dict):
                    message = {"raw": message}
                record = {
                    "type": "message",
                    "session_id": session_id,
                    "node_id": row["node_id"],
                    "parent_node_id": row["parent_node_id"],
                    "created_at": row["created_at"],
                    "role": message.get("role"),
                    "data": message,
                }
                json.dump(sanitize(record), mf)
                mf.write("\n")
                role = message.get("role")
                if role == "assistant":
                    _fold_receipt(message, seen, usage, models)
                    for call in message.get("tool_calls") or []:
                        if not isinstance(call, dict):
                            continue
                        call_id = str(call.get("id") or "")
                        if call_id and call_id in seen_calls:
                            continue
                        if call_id:
                            seen_calls.add(call_id)
                        line: dict[str, Any] = {
                            "type": "tool_call",
                            "session_id": session_id,
                            "tool_call": _normalized_tool_call(call),
                        }
                        json.dump(sanitize(line), mf)
                        mf.write("\n")
                        if stream_f is not None:
                            json.dump(sanitize(line), stream_f)
                            stream_f.write("\n")
                elif role == "tool":
                    line = {
                        "type": "tool_result",
                        "session_id": session_id,
                        "tool_call": {
                            "toolCallId": message.get("tool_call_id"),
                            "status": "completed",
                        },
                        "content": message.get("content"),
                    }
                    if stream_f is not None:
                        json.dump(sanitize(line), stream_f)
                        stream_f.write("\n")
                elif role == "user" and stream_f is not None:
                    json.dump(sanitize({"type": "user", "session_id": session_id}), stream_f)
                    stream_f.write("\n")
        with (dest / "tool_calls.jsonl").open("w", encoding="utf-8") as tf:
            for t in con.execute(
                "select tool_call_id, tool_call_json, tool_call_update_json from tool_call_state "
                "where session_id = ?",
                (session_id,),
            ).fetchall():
                entry: dict[str, Any] = {"tool_call_id": t["tool_call_id"]}
                for column in ("tool_call_json", "tool_call_update_json"):
                    raw = t[column]
                    try:
                        entry[column] = json.loads(raw) if raw else None
                    except json.JSONDecodeError:
                        entry[column] = str(raw)[:2000]
                json.dump(sanitize(entry), tf)
                tf.write("\n")
    finally:
        con.close()
    outcome.prompt_tokens = usage.prompt
    outcome.completion_tokens = usage.completion
    outcome.cached_input_tokens = usage.cached
    outcome.num_turns = usage.requests or None
    if models:
        served = models.most_common(1)[0][0]
        outcome.reported_model = served
        outcome.model_identity_confidence = "cli-reported"
        outcome.reported_effort = effort_from_uid(served)
    return {
        "requests": usage.requests,
        "input_tokens": usage.prompt - usage.cached - usage.cache_write,
        "output_tokens": usage.completion,
        "cache_read_tokens": usage.cached,
        "cache_creation_tokens": usage.cache_write,
        "served_models": dict(models),
        "total_credit_cost": session_meta.get("total_credit_cost"),
        "total_acu_cost": session_meta.get("total_acu_cost"),
    }


_LOG_TS_RE = re.compile(r"^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})")


def _log_line_is_recent(line: str, started_s: int) -> bool:
    """True for a CLI log line stamped at or after the run start (UTC)."""
    m = _LOG_TS_RE.match(line)
    if not m:
        return False
    try:
        stamp = time.mktime(time.strptime(f"{m.group(1)}T{m.group(2)}", "%Y-%m-%dT%H:%M:%S"))
    except ValueError:
        return False
    # The log is UTC; mktime read it as local time, so shift by the local offset.
    stamp -= time.timezone if not time.daylight else time.altzone
    return stamp >= started_s - 5


def export_tool_names(export_path: Path) -> list[str] | None:
    """Tool names the agent was given, from the ATIF export's ``tool_definitions``."""
    try:
        data = json.loads(export_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    agent = data.get("agent") if isinstance(data, dict) else None
    if not isinstance(agent, dict):
        return None
    names: list[str] = []
    for entry in agent.get("tool_definitions") or []:
        if not isinstance(entry, dict):
            continue
        function = entry.get("function")
        name = function.get("name") if isinstance(function, dict) else entry.get("name")
        if isinstance(name, str):
            names.append(name)
    return sorted(names)


def _harvest_log(session_id: str, started_s: int, run_dir: Path) -> None:
    """Copy this run's lines from the CLI's process logs (model-resolution
    warnings live there, not on stderr).

    Devin Desktop keeps its own ACP server appending to an older log file, so
    file mtimes are no filter: only lines stamped after the run start count,
    and of those only this session's lines and warnings.
    """
    log_dir = devin_data_home() / "cli" / "logs"
    if not log_dir.is_dir():
        return
    dest = run_dir / "devin-session"
    dest.mkdir(exist_ok=True)
    with (dest / "cli-log.txt").open("w", encoding="utf-8") as out:
        for path in sorted(log_dir.glob("devin_*.log")):
            try:
                if path.stat().st_mtime < started_s - 5:
                    continue
                with path.open(encoding="utf-8", errors="replace") as f:
                    for line in f:
                        if not _log_line_is_recent(line, started_s):
                            continue
                        if session_id in line or "did not resolve" in line or "WARN" in line:
                            out.write(str(sanitize(line.rstrip("\n")))[:2000] + "\n")
            except OSError:
                continue


def run_devin_task(  # noqa: PLR0912, PLR0915, linear process + harvest
    *,
    workspace: Path,
    prompt: str,
    model: str,
    priced_spec: str,
    max_turns: int,
    collector: Any,
    stream_log_path: Path | None = None,
    timeout_s: float | None = None,
    network: bool = False,
    max_run_cost: float | None = None,
    effort: str | None = None,
    devin_bin: str = "devin",
    preflight: HarnessPreflight | None = None,
    agent_container: Any = None,
) -> CliAgentOutcome:
    """Run one task through ``devin -p`` billed to the signed-in Devin account."""
    if agent_container is not None:
        raise ProviderError(
            "agent-in-container mode currently supports only the codex harness, not devin"
        )
    del priced_spec, max_turns  # max_turns: no step cap in print mode; the wall clock bounds a run
    workspace = Path(workspace).resolve()
    if timeout_s is not None and timeout_s <= 0:
        raise ProviderError("run budget exhausted before CLI agent start")
    if max_run_cost is not None:
        raise ProviderError(
            "devin streams no usage during a run (receipts are harvested from its session "
            "store afterwards) and its models carry no API price, so --max-run-cost cannot "
            "be enforced; use a wall-clock --timeout"
        )
    checked = preflight or devin_preflight(devin_bin)
    _require_subscription(checked)
    catalog = devin_model_catalog(devin_bin)
    model_uid = resolve_model_uid(model, effort, catalog)
    variant = catalog[model_uid]

    scratch = Path(tempfile.mkdtemp(prefix="vb-devin-")).resolve()
    config_home = scratch / "config"
    config_path = config_home / "devin" / "config.json"
    config_path.parent.mkdir(parents=True)
    run_config = build_run_config(_read_json(devin_user_config_path()), model_uid, network)
    config_path.write_text(json.dumps(run_config, indent=1) + "\n", encoding="utf-8")
    prompt_path = scratch / "prompt.md"
    prompt_path.write_text(prompt, encoding="utf-8")
    # Devin resolves --export against its own working directory (the task
    # workspace under /tmp), so the path must be absolute or the CLI fails at
    # exit with "failed to write conversation export" and the run is lost.
    export_path = (
        (stream_log_path.parent if stream_log_path else scratch) / "devin-session"
    ).resolve()
    export_path.mkdir(exist_ok=True)
    export_file = export_path / "export.json"

    cmd = [
        devin_bin,
        f"--config={config_path}",
        "--model",
        model_uid,
        "--permission-mode",
        "dangerous",
        "--respect-workspace-trust=false",
        f"--export={export_file}",
        "--prompt-file",
        str(prompt_path),
        "-p",
    ]
    env = _subscription_env()
    # The run's own config home: the user's rules, skills and MCP servers stay
    # out of the benchmark context; the login lives under the data home.
    env["XDG_CONFIG_HOME"] = str(config_home)
    env["NO_COLOR"] = "1"
    env["DEVIN_PERMISSION_MODE"] = "dangerous"
    logged_config: dict[str, Any] = dict(run_config)
    if "devin" in logged_config:
        logged_config["devin"] = "<org binding>"
    collector.record(
        "cli_agent_start",
        {
            "harness": "devin",
            "argv": [cmd[0], "--config=<run config>", *cmd[2:]],
            "harness_version": checked.version,
            "requested_model": model,
            "model_uid": model_uid,
            "catalog_variant": variant,
            "run_config": logged_config,
            "requested_effort": effort,
        },
    )
    outcome = CliAgentOutcome(
        harness="devin",
        execution_boundary=(
            "host-workspace; devin-permission-mode=dangerous; isolated config home; "
            + ("web-allowed" if network else "web-tools-disabled")
        ),
        requested_model=model_uid,
        harness_version=checked.version,
        auth_method=checked.auth_mode,
        plan_name=checked.plan_name,
    )
    db_path = devin_sessions_db()
    started_s = int(time.time()) - 2
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
            start_new_session=True,
        )
    except FileNotFoundError as exc:
        raise ProviderError(
            f"{devin_bin!r} not found on PATH; install with `brew install --cask devin-cli`"
        ) from exc

    stderr_chunks: list[str] = []

    def _drain_stderr() -> None:
        assert proc.stderr is not None
        for chunk in proc.stderr:
            stderr_chunks.append(chunk)

    stderr_thread = threading.Thread(target=_drain_stderr, daemon=True)
    stderr_thread.start()
    killed = {"timeout": False}

    def _kill_on_timeout() -> None:
        killed["timeout"] = True
        _kill_process_group(proc)

    watchdog: threading.Timer | None = None
    if timeout_s is not None:
        watchdog = threading.Timer(timeout_s, _kill_on_timeout)
        watchdog.daemon = True
        watchdog.start()

    stdout_parts: list[str] = []
    stream_f = stream_log_path.open("w", encoding="utf-8") if stream_log_path else None
    usage: dict[str, Any] = {}
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
        session_id = find_session(db_path, workspace, started_s)
        if session_id:
            outcome.session_id = session_id
            if stream_log_path is not None:
                try:
                    usage = harvest_session(
                        db_path, session_id, stream_log_path.parent, stream_f, outcome
                    )
                    _harvest_log(session_id, started_s, stream_log_path.parent)
                except ProviderError:
                    raise
                except Exception as exc:  # harvest must never mask the run itself
                    collector.record("cli_agent_harvest_error", {"error": str(exc)[:500]})
        if stream_f:
            stream_f.close()
        shutil.rmtree(scratch, ignore_errors=True)

    outcome.timed_out = killed["timeout"]
    tool_names = export_tool_names(export_file) if export_file.is_file() else None
    if usage:
        usage["tool_names"] = tool_names
        collector.record("devin_usage", usage)
    if tool_names is not None and not network:
        leaked = sorted(set(tool_names) & set(DEVIN_WEB_TOOLS))
        if leaked:
            raise ProviderError(
                f"devin still exposed web tools {leaked} despite the per-run disabled_tools; "
                "refusing to score a run whose web denial did not hold"
            )
    stderr_text = "".join(stderr_chunks)
    combined = stderr_text + "\n" + "".join(stdout_parts)
    if outcome.reported_model and outcome.reported_model != model_uid:
        raise ProviderError(
            f"devin served {outcome.reported_model!r} for a run that requested {model_uid!r}; "
            "refusing to score a run on the wrong model"
        )
    if proc.returncode != 0 or killed["timeout"]:
        if killed["timeout"]:
            # Partial work still counts; the caller diffs and verifies it.
            return outcome
        tail = stderr_text[-600:].strip() or "".join(stdout_parts)[-600:].strip() or "no output"
        if _LIMIT_PATTERN.search(combined):
            raise SubscriptionQuotaError(
                "devin plan limit hit, rerun after the window resets "
                f"(use --only-missing to resume): {tail[:300]}"
            )
        if _AUTH_PATTERN.search(combined):
            raise ProviderError(
                f"devin login is not valid, run `{devin_bin} auth login`: {tail[:300]}"
            )
        raise ProviderError(f"devin exited with status {proc.returncode}: {tail[:400]}")
    if outcome.session_id is None:
        raise ProviderError(
            "devin exited 0 but no session for this workspace was found in "
            f"{db_path}; cannot attribute usage or tool calls to the run"
        )
    if not usage.get("requests"):
        raise ProviderError("devin completed without auditable usage receipts")
    outcome.subtype = "success"
    outcome.finished = True
    collector.record("cli_agent_result", outcome.summary())
    return outcome


@dataclass(frozen=True)
class DevinAdapter:
    harness_id: str = "devin"

    def capabilities(self) -> HarnessCapabilities:
        return HarnessCapabilities(
            harness=self.harness_id,
            display_name="Devin CLI",
            executable="devin",
            # Print mode prints only the final text; messages, tool calls and
            # per-request receipts are harvested from the sqlite session store.
            structured_events=True,
            reports_tokens=True,
            reports_model=True,
            supports_effort=True,
            supports_live_cost_cap=False,
            sandbox="devin-permission-mode=dangerous; host workspace (web tools disabled)",
        )

    def preflight(self) -> HarnessPreflight:
        return devin_preflight()

    def run_task(self, **kwargs: Any) -> CliAgentOutcome:
        return run_devin_task(**kwargs)
