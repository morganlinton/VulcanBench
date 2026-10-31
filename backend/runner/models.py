"""Plain data shapes the Runs page consumes. Dataclasses, serialised with asdict."""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from typing import Any

RunStatus = str  # queued | preparing | running | stopping | finished | stopped |
# infra_failed | failed | external_running | lost | unknown


@dataclass
class ContainerView:
    name: str
    status: str
    state: str
    image: str


@dataclass
class FirstCheck:
    status: str  # ok | warning | pending | fail | unknown
    reasons: list[str] = field(default_factory=list)
    requested_model: str | None = None
    models: dict[str, int] = field(default_factory=dict)
    tool_calls: int = 0
    infra_hits: list[str] = field(default_factory=list)
    rate_limit_status: str | None = None
    evidence: str | None = None
    window_open: bool = False


@dataclass
class TrialView:
    id: str
    name: str
    status: str  # pending | running | finished | error | cancelled | interrupted
    reward: dict[str, Any] | None = None
    started_at: str | None = None
    finished_at: str | None = None
    elapsed_s: float | None = None
    cost_usd: float | None = None
    containers: list[ContainerView] = field(default_factory=list)
    first_check: FirstCheck | None = None
    log_sources: list[str] = field(default_factory=list)
    exception: str | None = None


@dataclass
class RunView:
    id: str
    kind: str  # gate | suite | task
    name: str
    status: RunStatus
    owned: bool = False
    suite_or_task: str | None = None
    model: str | None = None
    harness: str | None = None
    effort: str | None = None
    source_tag: str | None = None
    source_commit: str | None = None
    source_dir: str | None = None
    attempts_done: int = 0
    attempts_total: int | None = None
    solves: int | None = None
    started_at: str | None = None
    finished_at: str | None = None
    elapsed_s: float | None = None
    cost_usd: float | None = None
    pid: int | None = None
    path: str | None = None
    trials: list[TrialView] = field(default_factory=list)
    log_sources: list[str] = field(default_factory=list)
    first_check: FirstCheck | None = None
    infra_retries: int | None = None
    notes: list[str] = field(default_factory=list)
    job: dict[str, Any] | None = None  # the queue's record when the GUI owns this run

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    def summary_dict(self) -> dict[str, Any]:
        """The list-page shape: everything except per-trial detail."""
        d = asdict(self)
        d["trials"] = [
            {k: t[k] for k in ("id", "status", "reward", "elapsed_s")} for t in d["trials"]
        ]
        d.pop("log_sources", None)
        return d
