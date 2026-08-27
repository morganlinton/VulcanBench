"""Service discovery for a running orderflow deployment.

Two manifests, both JSON, both looked up from the current directory upward:

- ``.vb_services.json``   infrastructure (postgres, redis) published ports,
  written by the environment orchestrator before anything else starts.
- ``.of_topology.json``   application service base URLs, written by
  ``scripts/dev_up.py`` (or the test launcher) once the services are up.

Every process resolves both lazily and caches per-process.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

_INFRA_MANIFEST = ".vb_services.json"
_APP_MANIFEST = ".of_topology.json"

_cache: dict[str, dict] = {}


def _find(name: str) -> Path:
    override = os.environ.get("OF_MANIFEST_DIR")
    bases = [Path(override)] if override else [Path.cwd(), *Path.cwd().parents]
    for base in bases:
        candidate = base / name
        if candidate.exists():
            return candidate
    raise RuntimeError(f"{name} not found from {Path.cwd()} upward; is the stack up?")


def _load(name: str) -> dict:
    if name not in _cache:
        _cache[name] = json.loads(_find(name).read_text(encoding="utf-8"))
    return _cache[name]


def reset_cache() -> None:
    """Forget cached manifests (used by long-lived processes on reconnect)."""
    _cache.clear()


def infra_port(service: str, container_port: str) -> int:
    services = _load(_INFRA_MANIFEST)["services"]
    try:
        return int(services[service][container_port])
    except KeyError as exc:
        raise RuntimeError(f"{service}:{container_port} not in {_INFRA_MANIFEST}") from exc


def service_url(name: str) -> str:
    services = _load(_APP_MANIFEST)["services"]
    try:
        return str(services[name])
    except KeyError as exc:
        raise RuntimeError(f"service {name!r} not in {_APP_MANIFEST}") from exc


def app_manifest_path() -> Path:
    """Where ``dev_up`` should write the app manifest (next to the infra one)."""
    return _find(_INFRA_MANIFEST).parent / _APP_MANIFEST
