"""Economic receipts for API and subscription-backed benchmark runs.

One dollar field cannot honestly represent both metered APIs and flat-rate
subscriptions.  VulcanBench therefore records separate cash, allocation, quota,
and counterfactual API-equivalent measurements.  Unknown values stay ``None``;
the harness never turns "included in my plan" into a fake zero-dollar receipt.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass(frozen=True)
class EconomicsReceipt:
    """Normalized economics for one benchmark attempt."""

    billing_mode: str
    cost_basis: str
    marginal_cash_usd: float | None = None
    overage_cash_usd: float | None = None
    grading_cash_usd: float | None = None
    grading_api_equivalent_usd: float | None = None
    plan_name: str | None = None
    plan_fee_usd: float | None = None
    allocated_plan_cost_usd: float | None = None
    api_equivalent_cost_usd: float | None = None
    cli_reported_cost_usd: float | None = None
    quota: dict[str, Any] | None = None
    measurement_quality: dict[str, str] = field(default_factory=dict)

    def as_summary(self) -> dict[str, Any]:
        """JSON-safe representation persisted in ``summary.json``."""
        return {
            "billing_mode": self.billing_mode,
            "cost_basis": self.cost_basis,
            "marginal_cash_usd": self.marginal_cash_usd,
            "overage_cash_usd": self.overage_cash_usd,
            "grading_cash_usd": self.grading_cash_usd,
            "grading_api_equivalent_usd": self.grading_api_equivalent_usd,
            "plan_name": self.plan_name,
            "plan_fee_usd": self.plan_fee_usd,
            "allocated_plan_cost_usd": self.allocated_plan_cost_usd,
            "api_equivalent_cost_usd": self.api_equivalent_cost_usd,
            "cli_reported_cost_usd": self.cli_reported_cost_usd,
            "quota": self.quota,
            "measurement_quality": dict(self.measurement_quality),
        }


def api_receipt(total_cost_usd: float | None, grading_cost_usd: float | None) -> EconomicsReceipt:
    """Receipt for a normal metered API run."""
    quality = "provider-priced" if total_cost_usd is not None else "unavailable"
    return EconomicsReceipt(
        billing_mode="api-metered",
        cost_basis="metered-api-pricing",
        marginal_cash_usd=total_cost_usd,
        grading_cash_usd=grading_cost_usd,
        grading_api_equivalent_usd=grading_cost_usd,
        api_equivalent_cost_usd=total_cost_usd,
        measurement_quality={
            "marginal_cash_usd": quality,
            "api_equivalent_cost_usd": quality,
        },
    )


def router_receipt(
    *,
    reported_cost_usd: float,
    judges_cost_usd: float | None,
    api_equivalent_cost_usd: float | None,
) -> EconomicsReceipt:
    """Receipt for a run billed by a model router's own per-call ledger.

    The router's reported total is the cash paid (``marginal_cash_usd``);
    the price-table figure for the same tokens, when the router is priced
    at all, is kept as the list-price counterfactual so a router column can
    be read against a direct-API column.
    """
    total = round(reported_cost_usd + judges_cost_usd, 6) if judges_cost_usd is not None else None
    return EconomicsReceipt(
        billing_mode="api-metered",
        cost_basis="router-reported",
        marginal_cash_usd=total,
        grading_cash_usd=judges_cost_usd,
        grading_api_equivalent_usd=judges_cost_usd,
        api_equivalent_cost_usd=api_equivalent_cost_usd,
        measurement_quality={
            "marginal_cash_usd": "router-reported",
            "api_equivalent_cost_usd": (
                "estimated-from-reported-tokens"
                if api_equivalent_cost_usd is not None
                else "unavailable"
            ),
        },
    )


def subscription_receipt(
    *,
    api_equivalent_cost_usd: float | None,
    grading_cash_usd: float | None,
    grading_api_equivalent_usd: float | None,
    plan_name: str | None,
    cli_reported_cost_usd: float | None,
    api_equivalent_quality: str | None = None,
    quota: dict[str, Any] | None = None,
) -> EconomicsReceipt:
    """Receipt for an included-usage subscription run.

    Marginal cash and plan allocation remain unknown until a product exposes an
    overage receipt or the operator supplies a billing-period allocation.  The
    API-equivalent value is a counterfactual, never presented as cash paid.
    ``quota`` is the usage-window snapshot around the task (see
    ``harness/agent/quota_probe.py``) when the CLI exposes one.
    """
    api_quality = api_equivalent_quality or (
        "estimated-from-reported-tokens" if api_equivalent_cost_usd is not None else "unavailable"
    )
    quota_quality = "unavailable"
    if quota:
        quota_quality = "probe-windows" if quota.get("source") == "probe" else "in-stream-windows"
    return EconomicsReceipt(
        billing_mode="subscription-included",
        cost_basis="subscription-plus-api-equivalent",
        plan_name=plan_name,
        api_equivalent_cost_usd=api_equivalent_cost_usd,
        grading_cash_usd=grading_cash_usd,
        grading_api_equivalent_usd=grading_api_equivalent_usd,
        cli_reported_cost_usd=cli_reported_cost_usd,
        quota=quota,
        measurement_quality={
            "marginal_cash_usd": "unavailable",
            "overage_cash_usd": "unavailable",
            "allocated_plan_cost_usd": "unavailable",
            "api_equivalent_cost_usd": api_quality,
            "grading_cash_usd": (
                "provider-priced" if grading_cash_usd is not None else "unavailable"
            ),
            "grading_api_equivalent_usd": (
                "estimated-from-reported-tokens"
                if grading_api_equivalent_usd is not None
                else "unavailable"
            ),
            "quota": quota_quality,
        },
    )
