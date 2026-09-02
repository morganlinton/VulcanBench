"""Per-model token pricing.

Computes the USD cost of a run from its prompt/completion token counts. Prices
are a built-in table (USD per 1M tokens) that can be overridden via the
``VULCANBENCH_PRICING`` env var (path to a JSON file merged over the defaults).

Honesty: unknown models return ``None`` (cost unknown) rather than a guessed
number; ``mock`` models are free. Built-in prices are a point-in-time snapshot,
override them for anything that must be exact.
"""

from __future__ import annotations

import json
import os
from functools import lru_cache
from typing import Any

# USD per 1,000,000 tokens, as of 2026-06. Keys are exact "provider:model" specs;
# lookup also falls back to a "provider:" prefix default. Override with a JSON
# file at $VULCANBENCH_PRICING ({"openai:gpt-4o": {"input": .., "output": ..}}).
# These are a point-in-time snapshot, verify against the provider's pricing page
# before publishing numbers, and use the override file for anything that must be
# exact.
PRICES: dict[str, dict[str, float]] = {
    # GPT-5.6 list prices. Cached input is a cache read; cache writes and the
    # >272K long-context tier are not exposed by every harness receipt and are
    # therefore not inferred here.
    "openai:gpt-5.6-sol": {"input": 5.00, "cached_input": 0.50, "output": 30.00},
    "openai:gpt-5.6-terra": {"input": 2.50, "cached_input": 0.25, "output": 15.00},
    "openai:gpt-5.6-luna": {"input": 1.00, "cached_input": 0.10, "output": 6.00},
    "openai:gpt-5.5": {"input": 5.00, "output": 30.00},
    "openai:gpt-5.5-pro": {"input": 30.00, "output": 180.00},
    "openai:gpt-5.4": {"input": 2.50, "output": 15.00},
    "openai:gpt-5.4-mini": {"input": 0.75, "output": 4.50},
    "openai:gpt-5.4-nano": {"input": 0.20, "output": 1.25},
    "openai:gpt-5": {"input": 1.25, "output": 10.00},
    "openai:gpt-5-mini": {"input": 0.25, "output": 2.00},
    "openai:gpt-5-nano": {"input": 0.05, "output": 0.40},
    "openai:gpt-4o": {"input": 2.50, "output": 10.00},
    "openai:gpt-4o-mini": {"input": 0.15, "output": 0.60},
    "openai:gpt-4.1": {"input": 2.00, "output": 8.00},
    "openai:gpt-4.1-mini": {"input": 0.40, "output": 1.60},
    "openai:o3": {"input": 2.00, "output": 8.00},
    "openai:o4-mini": {"input": 1.10, "output": 4.40},
    "anthropic:claude-fable-5": {"input": 10.00, "output": 50.00},
    # Fable 5.1 matches Fable 5 on input/output; its cache reads are $0.25/M
    # (75% below Fable 5), which this table does not model, so api-equivalent
    # costs for cache-heavy agent runs are an upper bound.
    "anthropic:claude-fable-5-1": {"input": 10.00, "output": 50.00},
    "anthropic:claude-opus-5": {"input": 5.00, "output": 25.00},
    "anthropic:claude-opus-4-8": {"input": 5.00, "output": 25.00},
    "anthropic:claude-opus-4-7": {"input": 5.00, "output": 25.00},
    "anthropic:claude-opus-4-6": {"input": 5.00, "output": 25.00},
    # Sonnet 5 standard pricing; intro promo ($2/$10) runs through 2026-08-31.
    "anthropic:claude-sonnet-5": {"input": 3.00, "output": 15.00},
    "anthropic:claude-sonnet-4-6": {"input": 3.00, "output": 15.00},
    "anthropic:claude-haiku-4-5": {"input": 1.00, "output": 5.00},
    # Grok list prices are the <200K-input tier; xAI doubles input/cached/output
    # for requests with >=200K input tokens, which per-run receipts do not
    # expose, so long-context turns are underestimated here.
    "xai:grok-4.6": {"input": 2.00, "cached_input": 0.50, "output": 6.00},
    "xai:grok-4.5": {"input": 2.00, "cached_input": 0.30, "output": 6.00},
    "xai:grok-4.3": {"input": 1.25, "cached_input": 0.20, "output": 2.50},
    "zai:glm-5.3": {"input": 1.40, "cached_input": 0.26, "output": 4.40},
    "zai:glm-5.2": {"input": 1.40, "output": 4.40},
    "zai:glm-5.1": {"input": 1.40, "output": 4.40},
    "zai:glm-5": {"input": 1.00, "output": 3.20},
    "zai:glm-5-turbo": {"input": 1.20, "output": 4.00},
    # Cache-hit input is $0.30/M; we bill all input at the cache-miss rate, so
    # kimi costs are a slight overestimate on long multi-turn runs.
    "kimi:kimi-k3": {"input": 3.00, "output": 15.00},
    # OpenRouter, pinned to AkashML bf16 (see harness.agent.providers pins).
    "openrouter:qwen/qwen3.8-27b": {"input": 0.45, "cached_input": 0.05, "output": 3.20},
    # DashScope international list prices (≤256K / ≤32K tier as applicable).
    # Long-context tiers and promo discounts are not modeled, override with
    # VULCANBENCH_PRICING for exact numbers.
    # qwen3.8-27b (open-weights, first-party DashScope). Implicit cache read is
    # $0.10/M (the auto path; the harness sets no explicit cache breakpoints for
    # Qwen). Explicit cache read ($0.05) is not used here.
    "qwen:qwen3.8-27b": {"input": 0.50, "cached_input": 0.10, "output": 3.00},
    "qwen:qwen3.8-max": {"input": 2.00, "output": 6.00},
    "qwen:qwen3.7-plus": {"input": 0.40, "output": 1.60},
    "qwen:qwen3.7-max": {"input": 2.50, "output": 7.50},
    "qwen:qwen3.6-flash": {"input": 0.25, "output": 1.50},
    "qwen:qwen3-max": {"input": 1.20, "output": 6.00},
    "qwen:qwen3.5-plus": {"input": 0.40, "output": 2.40},
    "qwen:qwen-plus": {"input": 0.40, "output": 1.20},
    # DeepSeek V4 public-beta list prices. A peak/off-peak policy (2x during
    # Beijing peak hours) has been announced but is not yet in effect and is
    # not modeled, override with VULCANBENCH_PRICING if/when it lands.
    "deepseek:deepseek-v4-flash": {"input": 0.14, "output": 0.28},
    "deepseek:deepseek-v4-pro": {"input": 0.435, "output": 0.87},
    # Meta Model API standard tier. Contributor requests permit Meta to use
    # prompts/completions for training in exchange for the lower rate.
    "meta:muse-spark-1.2": {"input": 1.25, "output": 4.25},
    "meta:muse-spark-1.2-contributor": {"input": 0.10, "output": 0.20},
    # Free / offline.
    "mock:": {"input": 0.0, "output": 0.0},
    # Local inference: no marginal per-token cost. $0 is the marginal cash truth;
    # hardware and electricity are not modeled.
    "ollama:": {"input": 0.0, "output": 0.0},
}

_PER_MILLION = 1_000_000.0

# Vendor agent-CLI specs (subscription billing) price at the underlying API
# rates, so their ``cost_usd`` is the *hypothetical* API cost of the same
# tokens. The run summary marks these with ``cli_agent.billing`` so the
# number is never mistaken for actual spend.
_SPEC_ALIASES = {
    "claude-code:": "anthropic:",
    "codex:": "openai:",
    "grok-build:": "xai:",
    "zcode:": "zai:",
}


def _canonical_spec(model: str) -> str:
    for prefix, replacement in _SPEC_ALIASES.items():
        if model.startswith(prefix):
            return replacement + model[len(prefix) :]
    return model


@lru_cache(maxsize=1)
def _prices() -> dict[str, dict[str, float]]:
    prices = dict(PRICES)
    override = os.environ.get("VULCANBENCH_PRICING")
    if override and os.path.exists(override):
        try:
            with open(override, encoding="utf-8") as f:
                custom = json.load(f)
            if isinstance(custom, dict):
                for spec, override_rate in custom.items():
                    if isinstance(override_rate, dict) and isinstance(prices.get(spec), dict):
                        prices[spec] = {**prices[spec], **override_rate}
                    else:
                        prices[spec] = override_rate
        except (OSError, json.JSONDecodeError):
            pass
    return prices


def _rate(model: str) -> dict[str, float] | None:
    model = _canonical_spec(model)
    prices = _prices()
    if model in prices:
        return prices[model]
    provider = model.split(":", 1)[0] + ":" if ":" in model else ""
    return prices.get(provider)


def cost_usd(
    model: str,
    prompt_tokens: int,
    completion_tokens: int,
    *,
    cached_input_tokens: int = 0,
) -> float | None:
    """USD cost for a model call, or ``None`` if the model is not priced.

    ``prompt_tokens`` is the total input count. When a provider reports cache
    reads separately, that subset is billed at ``cached_input`` when the price
    table provides it. Otherwise cached input conservatively uses the normal
    input rate.
    """
    rate = _rate(model)
    if rate is None:
        return None
    cached = min(max(0, cached_input_tokens), max(0, prompt_tokens))
    uncached = max(0, prompt_tokens) - cached
    cached_rate = rate.get("cached_input", rate["input"])
    cost = (
        uncached * rate["input"] + cached * cached_rate + max(0, completion_tokens) * rate["output"]
    ) / _PER_MILLION
    return round(cost, 6)


def has_cached_input_price(model: str) -> bool:
    """Whether the effective model price distinguishes cache reads."""
    rate = _rate(model)
    return rate is not None and "cached_input" in rate


def cached_input_factor(model: str, default: float = 0.1) -> float:
    """Cache-read price as a fraction of full input price, from the table.

    OpenAI-compatible providers fold cache reads into the effective prompt
    count at this factor (``effective = uncached + cached * factor``), so a
    direct-API run's cost reflects the provider's own cache-read rate rather
    than a generic guess. Falls back to ``default`` when a model has no
    ``cached_input`` entry, preserving prior behavior for those models.
    """
    rate = _rate(model)
    if rate and rate.get("input") and "cached_input" in rate:
        return rate["cached_input"] / rate["input"]
    return default


def is_priced(model: str) -> bool:
    """True if ``model`` has a known price (so a real cost can be computed)."""
    return _rate(model) is not None


def reset_cache() -> None:
    """Clear the memoized price table (used by tests after setting the env var)."""
    _prices.cache_clear()


def merged_prices() -> dict[str, Any]:
    """Return the effective price table (defaults + override). For inspection."""
    return dict(_prices())
