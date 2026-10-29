"""Read-only view of Docker containers via ``docker ps``.

Uses the CLI rather than the Docker SDK so the backend keeps no extra
dependency, and caches for a few seconds because every poll of the Runs page
would otherwise shell out again.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import time
from dataclasses import dataclass, field

CACHE_TTL_S = 5.0
_TIMEOUT_S = 8


@dataclass
class Container:
    id: str
    name: str
    status: str
    state: str
    image: str
    labels: dict[str, str] = field(default_factory=dict)
    mounts: str = ""

    @property
    def compose_project(self) -> str | None:
        return self.labels.get("com.docker.compose.project")


class _Cache:
    stamp: float = 0.0
    items: list[Container] | None = None


_cache = _Cache()


def _parse_labels(raw: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for part in raw.split(","):
        if "=" in part:
            k, v = part.split("=", 1)
            out[k.strip()] = v.strip()
    return out


def docker_available() -> bool:
    return shutil.which("docker") is not None


def list_containers(*, now: float | None = None) -> list[Container]:
    """All containers (running or not), cached for CACHE_TTL_S seconds."""
    t = time.monotonic() if now is None else now
    cached = _cache.items
    if cached is not None and t - _cache.stamp < CACHE_TTL_S:
        return cached
    docker = shutil.which("docker")
    if docker is None:
        return []
    try:
        proc = subprocess.run(
            [docker, "ps", "-a", "--no-trunc", "--format", "{{json .}}"],
            capture_output=True,
            text=True,
            timeout=_TIMEOUT_S,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return cached or []
    if proc.returncode != 0:
        return cached or []
    out: list[Container] = []
    for raw in proc.stdout.splitlines():
        line = raw.strip()
        if not line:
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(row, dict):
            continue
        out.append(
            Container(
                id=str(row.get("ID", "")),
                name=str(row.get("Names", "")),
                status=str(row.get("Status", "")),
                state=str(row.get("State", "")),
                image=str(row.get("Image", "")),
                labels=_parse_labels(str(row.get("Labels", ""))),
                mounts=str(row.get("Mounts", "")),
            )
        )
    _cache.stamp, _cache.items = t, out
    return out


def reset_cache() -> None:
    _cache.stamp, _cache.items = 0.0, None
