"""Runner configuration: ``[runner]`` in vulcanbench.toml plus env overrides."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

from harness.settings import SETTINGS_PATH, runner_settings

REPO_ROOT = SETTINGS_PATH.parent
ENV_PREFIX = "VULCANBENCH_RUNNER_"


@dataclass(frozen=True)
class RunnerConfig:
    enabled: bool
    home: Path
    gate_out: Path
    run_worktrees: Path
    extra_run_roots: tuple[Path, ...]
    first_check_after_s: int

    @property
    def gate_jobs_dir(self) -> Path:
        return self.gate_out / "jobs"

    @property
    def harness_runs_dir(self) -> Path:
        return self.home / "runs"

    @property
    def run_roots(self) -> tuple[Path, ...]:
        """Harness output roots scanned by the Runs page, in index order."""
        return (self.harness_runs_dir, *self.extra_run_roots)

    def to_dict(self) -> dict[str, object]:
        return {
            "enabled": self.enabled,
            "home": str(self.home),
            "gate_out": str(self.gate_out),
            "run_worktrees": str(self.run_worktrees),
            "run_roots": [str(p) for p in self.run_roots],
            "first_check_after_s": self.first_check_after_s,
        }


def _path(value: str) -> Path:
    p = Path(value).expanduser()
    if not p.is_absolute():
        p = REPO_ROOT / p
    return Path(os.path.normpath(p.absolute()))


def _env(key: str) -> str | None:
    value = os.environ.get(ENV_PREFIX + key.upper())
    return value if value else None


def runner_enabled() -> bool:
    return os.environ.get("VULCANBENCH_RUNNER", "") in {"1", "true", "yes", "on"}


def load_config(settings_path: Path = SETTINGS_PATH) -> RunnerConfig:
    """Read the toml table and apply ``VULCANBENCH_RUNNER_*`` overrides.

    Cheap enough to call per request, which keeps tests (and operators) free
    to change the environment without restarting the backend.
    """
    table = runner_settings(settings_path)
    extra_env = _env("extra_run_roots")
    extra = (
        [p for p in extra_env.split(os.pathsep) if p]
        if extra_env is not None
        else list(table["extra_run_roots"])
    )
    first_env = _env("first_check_after_s")
    first = int(first_env) if first_env is not None else int(table["first_check_after_s"])
    return RunnerConfig(
        enabled=runner_enabled(),
        home=_path(_env("home") or str(table["home"])),
        gate_out=_path(_env("gate_out") or str(table["gate_out"])),
        run_worktrees=_path(_env("run_worktrees") or str(table["run_worktrees"])),
        extra_run_roots=tuple(_path(p) for p in extra),
        first_check_after_s=first,
    )
