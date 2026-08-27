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


@pytest.mark.parametrize("provider", ["codex", "claude-code"])
def test_subscription_harness_effort_is_forwarded(provider: str) -> None:
    cfg = effort_config(provider, "extra-high")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "extra-high",
        "provider": provider,
        "provider_value": "xhigh",
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


def test_xai_effort_maps_low_through_xhigh() -> None:
    for requested, sent in (
        ("low", "low"),
        ("medium", "medium"),
        ("high", "high"),
        ("extra-high", "xhigh"),
    ):
        cfg = effort_config("xai", requested)
        assert cfg is not None
        assert cfg.provider_value == sent
        assert cfg.supported is True


def test_xai_minimal_and_max_are_noop_metadata() -> None:
    # xAI's enum has no minimal or max; recording them as unsupported keeps a
    # sweep from stamping effort labels the API never saw.
    for requested in ("minimal", "max"):
        cfg = effort_config("xai", requested)
        assert cfg is not None
        assert cfg.provider_value is None
        assert cfg.supported is False


def test_ollama_effort_is_noop_metadata() -> None:
    cfg = effort_config("ollama", "high")
    assert cfg is not None
    assert cfg.provider_value is None
    assert cfg.supported is False


def test_zai_effort_is_noop_metadata_pre_5_3() -> None:
    # GLM 5.2 (and an unspecified model) have no reasoning_effort knob.
    for model in (None, "glm-5.2"):
        cfg = effort_config("zai", "low", model)
        assert cfg is not None
        assert cfg.as_summary() == {
            "requested": "low",
            "provider": "zai",
            "provider_value": None,
            "supported": False,
        }


def test_zai_glm_5_3_effort_maps_low_high_and_max() -> None:
    # GLM 5.3's documented enum is low/high/max (extra-high reaches the ceiling).
    for requested, sent in (("low", "low"), ("high", "high"), ("extra-high", "max")):
        cfg = effort_config("zai", requested, "glm-5.3")
        assert cfg is not None
        assert cfg.as_summary() == {
            "requested": requested,
            "provider": "zai",
            "provider_value": sent,
            "supported": True,
        }


def test_zai_glm_5_3_medium_is_noop_metadata() -> None:
    # GLM 5.3 has no "medium" level, so it stays recorded-but-not-sent rather
    # than pretending a level the API lacks ran.
    cfg = effort_config("zai", "medium", "glm-5.3")
    assert cfg is not None
    assert cfg.as_summary() == {
        "requested": "medium",
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


def test_meta_effort_maps_minimal_through_xhigh() -> None:
    for requested, sent in (
        ("minimal", "minimal"),
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


def test_pi_effort_maps_thinking_levels() -> None:
    for requested, sent in (
        ("minimal", "minimal"),
        ("low", "low"),
        ("medium", "medium"),
        ("high", "high"),
        ("extra-high", "xhigh"),
    ):
        cfg = effort_config("pi", requested)
        assert cfg is not None
        assert cfg.provider_value == sent
        assert cfg.supported is True
    cfg = effort_config("pi", "max")
    assert cfg is not None
    assert cfg.supported is False


def test_openai_minimal_maps_directly() -> None:
    cfg = effort_config("openai", "minimal")
    assert cfg is not None
    assert cfg.provider_value == "minimal"
    assert cfg.supported is True


def test_minimal_is_noop_where_undocumented() -> None:
    # Only OpenAI and Meta document a minimal level. Everyone else records the
    # label without sending it, including claude-code, whose CLI would reject
    # an --effort value it does not have.
    for provider in ("anthropic", "claude-code", "kimi", "qwen", "deepseek"):
        cfg = effort_config(provider, "minimal")
        assert cfg is not None, provider
        assert cfg.provider_value is None, provider
        assert cfg.supported is False, provider


def test_claude_code_reachable_levels_unchanged() -> None:
    # Guard: adding minimal to the vocabulary must not change what the Claude
    # Code CLI is sent for the previously reachable labels.
    for requested, sent in (
        ("low", "low"),
        ("medium", "medium"),
        ("high", "high"),
        ("extra-high", "xhigh"),
        ("max", "max"),
    ):
        cfg = effort_config("claude-code", requested)
        assert cfg is not None
        assert cfg.provider_value == sent
        assert cfg.supported is True


def test_parse_efforts_accepts_minimal() -> None:
    assert parse_efforts("minimal,low") == ["minimal", "low"]
    # The default sweep is unchanged; minimal is opt-in.
    assert "minimal" not in parse_efforts(None)


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
