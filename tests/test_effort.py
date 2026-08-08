"""Tests for normalized reasoning-effort helpers."""

from __future__ import annotations

import pytest

from harness.effort import effort_config, parse_efforts


def test_parse_efforts_defaults_and_dedupes() -> None:
    assert parse_efforts(None) == ["low", "medium", "high"]
    assert parse_efforts("low, medium, low") == ["low", "medium"]


def test_openai_extra_high_maps_to_xhigh() -> None:
    cfg = effort_config("openai", "extra-high")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "extra-high",
        "provider": "openai",
        "provider_value": "xhigh",
        "supported": True,
    }


def test_openai_max_maps_to_distinct_max_value() -> None:
    cfg = effort_config("openai", "max")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "max",
        "provider": "openai",
        "provider_value": "max",
        "supported": True,
    }


def test_mock_effort_is_noop_metadata() -> None:
    cfg = effort_config("mock", "low")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "low",
        "provider": "mock",
        "provider_value": None,
        "supported": False,
    }


def test_zai_effort_is_noop_metadata() -> None:
    cfg = effort_config("zai", "low")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "low",
        "provider": "zai",
        "provider_value": None,
        "supported": False,
    }


def test_qwen_effort_maps_low_medium_and_xhigh() -> None:
    for requested, sent in (("low", "low"), ("medium", "medium"), ("extra-high", "xhigh")):
        cfg = effort_config("qwen", requested)
        assert cfg is not None
        assert cfg.as_summary() == {
            "requested": requested,
            "provider": "qwen",
            "provider_value": sent,
            "supported": True,
        }


def test_qwen_high_is_noop_metadata() -> None:
    # Qwen's enum is low/medium/xhigh with xhigh as the default: "high" does
    # not exist, so it must never be sent (the run executes at the default and
    # the metadata says supported=False).
    cfg = effort_config("qwen", "high")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "high",
        "provider": "qwen",
        "provider_value": None,
        "supported": False,
    }


def test_deepseek_effort_maps_low_high_and_max() -> None:
    for requested, sent in (("low", "low"), ("high", "high"), ("extra-high", "max")):
        cfg = effort_config("deepseek", requested)
        assert cfg is not None
        assert cfg.as_summary() == {
            "requested": requested,
            "provider": "deepseek",
            "provider_value": sent,
            "supported": True,
        }


def test_deepseek_medium_is_noop_metadata() -> None:
    # DeepSeek's enum is low/high/max: "medium" does not exist and the API
    # silently coerces it to "high", so the harness must not send it (the run
    # then executes at the default and the metadata says supported=False).
    cfg = effort_config("deepseek", "medium")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "medium",
        "provider": "deepseek",
        "provider_value": None,
        "supported": False,
    }


def test_meta_effort_maps_low_through_xhigh() -> None:
    for requested, sent in (
        ("low", "low"),
        ("medium", "medium"),
        ("high", "high"),
        ("extra-high", "xhigh"),
    ):
        cfg = effort_config("meta", requested)
        assert cfg is not None
        assert cfg.as_summary() == {
            "requested": requested,
            "provider": "meta",
            "provider_value": sent,
            "supported": True,
        }


def test_meta_max_is_noop_metadata() -> None:
    cfg = effort_config("meta", "max")
    assert cfg is not None
    assert cfg.provider_value is None
    assert cfg.supported is False


def test_kimi_effort_below_max_is_noop_metadata() -> None:
    # Moonshot only ships reasoning_effort="max" today; other levels are
    # recorded but not sent.
    cfg = effort_config("kimi", "low")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "low",
        "provider": "kimi",
        "provider_value": None,
        "supported": False,
    }


def test_kimi_extra_high_maps_to_max() -> None:
    cfg = effort_config("kimi", "extra-high")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "extra-high",
        "provider": "kimi",
        "provider_value": "max",
        "supported": True,
    }


def test_anthropic_effort_maps_to_output_config_values() -> None:
    cfg = effort_config("anthropic", "medium")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "medium",
        "provider": "anthropic",
        "provider_value": "medium",
        "supported": True,
    }
    xhigh = effort_config("anthropic", "extra-high")
    assert xhigh is not None
    assert xhigh.provider_value == "xhigh"


def test_anthropic_max_is_noop_metadata() -> None:
    cfg = effort_config("anthropic", "max")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "max",
        "provider": "anthropic",
        "provider_value": None,
        "supported": False,
    }


def test_unknown_provider_effort_rejected() -> None:
    with pytest.raises(ValueError, match="not supported for provider"):
        effort_config("acme", "medium")
