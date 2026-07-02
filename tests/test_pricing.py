"""Tests for per-model pricing."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from harness import pricing


@pytest.fixture(autouse=True)
def _clear_cache() -> None:
    pricing.reset_cache()


def test_known_model_cost() -> None:
    # gpt-4o: input 2.50/1M, output 10.00/1M -> 1000*2.5/1e6 + 500*10/1e6
    assert pricing.cost_usd("openai:gpt-4o", 1000, 500) == 0.0075


def test_unknown_model_is_none() -> None:
    assert pricing.cost_usd("openai:does-not-exist-9000", 1000, 500) is None
    assert pricing.is_priced("foo:bar") is False


def test_mock_is_free() -> None:
    assert pricing.cost_usd("mock:synthetic", 1_000_000, 1_000_000) == 0.0
    assert pricing.is_priced("mock:anything") is True


def test_env_override(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    override = tmp_path / "prices.json"
    override.write_text(json.dumps({"custom:model": {"input": 1.0, "output": 2.0}}))
    monkeypatch.setenv("VULCANBENCH_PRICING", str(override))
    pricing.reset_cache()
    # 1M input @ $1 + 1M output @ $2 = $3
    assert pricing.cost_usd("custom:model", 1_000_000, 1_000_000) == 3.0


def test_provider_prefix_fallback() -> None:
    # mock: prefix matches any mock model even without an exact entry.
    assert pricing.cost_usd("mock:whatever", 100, 100) == 0.0


def test_zai_glm_priced() -> None:
    assert pricing.is_priced("zai:glm-5.2")
    assert pricing.cost_usd("zai:glm-5.2", 1_000_000, 1_000_000) == 5.80


def test_anthropic_frontier_models_priced() -> None:
    # Sonnet 5 standard pricing: input 3.00/1M, output 15.00/1M -> 1M+1M = 18.00.
    assert pricing.is_priced("anthropic:claude-sonnet-5")
    assert pricing.cost_usd("anthropic:claude-sonnet-5", 1_000_000, 1_000_000) == 18.0
    # Opus 4.8: input 5.00/1M, output 25.00/1M -> 30.00. Both are needed for the
    # effort-sweep cost/Pareto axis; an unpriced model silently yields cost=None.
    assert pricing.cost_usd("anthropic:claude-opus-4-8", 1_000_000, 1_000_000) == 30.0
    # Fable 5: input 10.00/1M, output 50.00/1M -> 60.00.
    assert pricing.is_priced("anthropic:claude-fable-5")
    assert pricing.cost_usd("anthropic:claude-fable-5", 1_000_000, 1_000_000) == 60.0
