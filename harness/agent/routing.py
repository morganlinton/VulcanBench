"""Per-call routing ledger: which model actually served each step.

A model router (``router:<name>/<model>``, ``openrouter:openrouter/auto``) or
a server-side fallback may answer one request with a different model than the
one requested.  Providers surface the served model, the response id and, where
the gateway returns one, its own per-call cost; this ledger folds those into
run-level facts that ``summary.json`` carries next to the token counts.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass, field
from typing import Any


@dataclass
class RoutingLedger:
    served: Counter[str] = field(default_factory=Counter)
    switches: int = 0
    steps: int = 0
    steps_with_cost: int = 0
    reported_cost_usd: float = 0.0
    _last: str | None = None

    def record(self, served_model: str | None, reported_cost_usd: float | None) -> None:
        self.steps += 1
        if served_model:
            if self._last is not None and served_model != self._last:
                self.switches += 1
            self._last = served_model
            self.served[served_model] += 1
        if reported_cost_usd is not None:
            self.steps_with_cost += 1
            self.reported_cost_usd += float(reported_cost_usd)

    @property
    def cost_complete(self) -> bool:
        """True when every step carried a gateway-reported cost."""
        return self.steps > 0 and self.steps_with_cost == self.steps

    @property
    def router_reported_cost_usd(self) -> float | None:
        """The gateway's own bill for the run, or ``None`` if any step lacked one.

        A partial sum would read as a cheaper run than it was, so it is withheld
        and the per-step count is left in the summary instead.
        """
        return round(self.reported_cost_usd, 6) if self.cost_complete else None

    def as_summary(self) -> dict[str, Any]:
        return {
            "served_models": dict(self.served),
            "served_model_switches": self.switches,
            "router_reported_cost_usd": self.router_reported_cost_usd,
            "router_cost_steps": self.steps_with_cost,
        }


def empty_routing_summary() -> dict[str, Any]:
    return RoutingLedger().as_summary()
