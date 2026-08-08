"""Tests for the LLM provider interface."""

from __future__ import annotations

import pytest

from harness.agent import providers as P
from harness.agent.providers import (
    AnthropicProvider,
    DeepSeekProvider,
    KimiProvider,
    LLMResponse,
    MetaProvider,
    MockProvider,
    OpenAIProvider,
    QwenProvider,
    TokenUsage,
    ZaiProvider,
    get_provider,
    parse_model_spec,
)


@pytest.mark.parametrize(
    ("spec", "expected"),
    [
        ("openai:gpt-4o", ("openai", "gpt-4o")),
        ("anthropic:claude-opus-4-8", ("anthropic", "claude-opus-4-8")),
        ("zai:glm-5.2", ("zai", "glm-5.2")),
        ("qwen:qwen3.7-plus", ("qwen", "qwen3.7-plus")),
        ("deepseek:deepseek-v4-flash", ("deepseek", "deepseek-v4-flash")),
        ("meta:muse-spark-1.2", ("meta", "muse-spark-1.2")),
        ("mock:synthetic", ("mock", "synthetic")),
        ("openai:gpt-4o:extra", ("openai", "gpt-4o:extra")),
    ],
)
def test_parse_model_spec(spec: str, expected: tuple[str, str]) -> None:
    assert parse_model_spec(spec) == expected


@pytest.mark.parametrize("bad", ["gpt-4o", "openai:", ":model", ""])
def test_parse_model_spec_rejects_bad(bad: str) -> None:
    with pytest.raises(ValueError):
        parse_model_spec(bad)


def test_get_provider_unknown() -> None:
    with pytest.raises(ValueError, match="unknown provider"):
        get_provider("nope:x")


def test_get_provider_returns_mock() -> None:
    p = get_provider("mock:synthetic")
    assert isinstance(p, MockProvider)
    assert p.name == "mock"
    assert p.spec == "mock:synthetic"


def test_get_provider_returns_zai() -> None:
    p = get_provider("zai:glm-5.2")
    assert isinstance(p, ZaiProvider)
    assert p.name == "zai"
    assert p.spec == "zai:glm-5.2"


def test_get_provider_returns_kimi() -> None:
    p = get_provider("kimi:kimi-k3")
    assert isinstance(p, KimiProvider)
    assert p.name == "kimi"
    assert p.spec == "kimi:kimi-k3"


def test_get_provider_returns_qwen() -> None:
    p = get_provider("qwen:qwen3.7-plus")
    assert isinstance(p, QwenProvider)
    assert p.name == "qwen"
    assert p.spec == "qwen:qwen3.7-plus"


def test_get_provider_returns_deepseek() -> None:
    p = get_provider("deepseek:deepseek-v4-flash")
    assert isinstance(p, DeepSeekProvider)
    assert p.name == "deepseek"
    assert p.spec == "deepseek:deepseek-v4-flash"


def test_get_provider_returns_meta() -> None:
    p = get_provider("meta:muse-spark-1.2")
    assert isinstance(p, MetaProvider)
    assert p.name == "meta"
    assert p.spec == "meta:muse-spark-1.2"


def test_token_usage_total() -> None:
    assert TokenUsage(prompt_tokens=10, completion_tokens=5).total == 15


def test_llm_response_wants_tools() -> None:
    assert LLMResponse().wants_tools is False


def test_mock_provider_scripted_policy() -> None:
    """Mock walks read -> edit -> test -> finish based on tool-result count."""
    p = MockProvider("synthetic")
    msgs: list[dict[str, object]] = [{"role": "user", "content": "issue"}]

    r0 = p.complete(msgs, [])
    assert r0.tool_calls[0].name == "read_file"

    msgs.append({"role": "tool", "content": "..."})
    r1 = p.complete(msgs, [])
    assert r1.tool_calls[0].name == "edit_file"

    msgs.append({"role": "tool", "content": "..."})
    r2 = p.complete(msgs, [])
    assert r2.tool_calls[0].name == "run_tests"

    msgs.append({"role": "tool", "content": "..."})
    r3 = p.complete(msgs, [])
    assert not r3.wants_tools
    assert r3.content is not None and "FINISH" in r3.content


def test_loads_args() -> None:
    assert P._loads_args('{"a": 1}') == {"a": 1}
    assert P._loads_args({"a": 1}) == {"a": 1}
    assert P._loads_args("not json") == {}
    assert P._loads_args(None) == {}


def test_openai_tool_to_anthropic() -> None:
    tool = {"function": {"name": "read_file", "description": "d", "parameters": {"type": "object"}}}
    out = P._openai_tool_to_anthropic(tool)
    assert out["name"] == "read_file"
    assert out["input_schema"] == {"type": "object"}


def test_to_anthropic_messages_conversion() -> None:
    messages = [
        {"role": "system", "content": "be good"},
        {"role": "user", "content": "do it"},
        {
            "role": "assistant",
            "content": "calling",
            "tool_calls": [
                {"id": "t1", "function": {"name": "read_file", "arguments": '{"path": "x"}'}}
            ],
        },
        {"role": "tool", "tool_call_id": "t1", "content": "result"},
    ]
    system, converted = P._to_anthropic_messages(messages)
    assert system == "be good"
    # assistant turn carries text then a tool_use block; tool turn -> tool_result
    assistant_blocks = converted[1]["content"]
    assert {b["type"] for b in assistant_blocks} == {"text", "tool_use"}
    tool_use = next(b for b in assistant_blocks if b["type"] == "tool_use")
    assert tool_use["name"] == "read_file"
    assert converted[2]["content"][0]["type"] == "tool_result"
    assert converted[2]["content"][0]["tool_use_id"] == "t1"


def test_openai_complete_parses_tool_calls(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["payload"] = payload
        assert "chat/completions" in url
        assert payload["tools"]  # tools forwarded
        return {
            "choices": [
                {
                    "message": {
                        "content": "ok",
                        "tool_calls": [
                            {
                                "id": "c1",
                                "function": {"name": "read_file", "arguments": '{"path": "a"}'},
                            }
                        ],
                    }
                }
            ],
            "usage": {"prompt_tokens": 11, "completion_tokens": 3},
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = OpenAIProvider("gpt-4o").complete(
        [{"role": "user", "content": "hi"}], [{"function": {"name": "read_file"}}]
    )
    assert resp.content == "ok"
    assert resp.tool_calls[0].name == "read_file"
    assert resp.tool_calls[0].arguments == {"path": "a"}
    assert resp.usage.total == 14
    payload = seen["payload"]
    assert isinstance(payload, dict)
    assert payload["temperature"] == 0


def test_openai_gpt5_omits_temperature(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["payload"] = payload
        return {"choices": [{"message": {"content": "ok"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = OpenAIProvider("gpt-5.5").complete([{"role": "user", "content": "hi"}], [])
    assert resp.content == "ok"
    payload = seen["payload"]
    assert isinstance(payload, dict)
    assert "temperature" not in payload


def test_openai_complete_uses_budgeted_http_timeout(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test")
    seen: list[float] = []

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen.append(timeout)
        return {"choices": [{"message": {"content": "ok"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = OpenAIProvider("gpt-4o").complete([], [], timeout_s=3.2)

    assert resp.content == "ok"
    assert seen == [3.2]


def test_openai_effort_uses_responses_payload(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        seen["payload"] = payload
        return {
            "output": [
                {
                    "type": "function_call",
                    "call_id": "c1",
                    "name": "read_file",
                    "arguments": '{"path": "a"}',
                },
                {
                    "type": "message",
                    "content": [{"type": "output_text", "text": "ok"}],
                },
            ],
            "usage": {"input_tokens": 11, "output_tokens": 3},
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = OpenAIProvider("gpt-5.1").complete(
        [{"role": "user", "content": "hi"}],
        [{"function": {"name": "read_file", "description": "read", "parameters": {}}}],
        effort="xhigh",
    )

    payload = seen["payload"]
    assert isinstance(payload, dict)
    assert "responses" in seen["url"]
    assert payload["reasoning"] == {"effort": "xhigh"}
    assert payload["tools"][0]["name"] == "read_file"
    assert resp.content == "ok"
    assert resp.tool_calls[0].arguments == {"path": "a"}
    assert resp.usage.total == 14


def test_openai_max_effort_uses_distinct_responses_value(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["payload"] = payload
        return {
            "output": [{"type": "message", "content": [{"type": "output_text", "text": "ok"}]}],
            "usage": {},
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    OpenAIProvider("gpt-5.6-terra").complete([], [], effort="max")

    payload = seen["payload"]
    assert isinstance(payload, dict)
    assert payload["reasoning"] == {"effort": "max"}


def test_openai_responses_discounts_cached_prompt_tokens(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # OpenAI reports input_tokens as the FULL prompt, with the cached portion in
    # input_tokens_details.cached_tokens (billed ~0.1x on the GPT-5 series). The
    # effective prompt count must fold cache reads at 0.1x, not bill them full price.
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test")

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        return {
            "output": [{"type": "message", "content": [{"type": "output_text", "text": "ok"}]}],
            "usage": {
                "input_tokens": 10000,
                "input_tokens_details": {"cached_tokens": 9000},
                "output_tokens": 100,
            },
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = OpenAIProvider("gpt-5.5").complete(
        [{"role": "user", "content": "hi"}], [], effort="high"
    )
    # 1000 uncached + 9000 cached * 0.1 = 1000 + 900 = 1900 effective prompt tokens
    # (a full-price count would have been 10000).
    assert resp.usage.prompt_tokens == 1900
    assert resp.usage.completion_tokens == 100


def test_openai_complete_requires_key(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("OPENAI_API_KEY", raising=False)
    with pytest.raises(P.ProviderError, match="OPENAI_API_KEY"):
        OpenAIProvider("gpt-4o").complete([], [])


def test_meta_complete_uses_responses_api(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("MODEL_API_KEY", "meta-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen.update(url=url, headers=headers, payload=payload)
        return {
            "output": [
                {
                    "type": "function_call",
                    "call_id": "m1",
                    "name": "run_tests",
                    "arguments": "{}",
                }
            ],
            "usage": {
                "input_tokens": 1000,
                "input_tokens_details": {"cached_tokens": 500},
                "output_tokens": 25,
            },
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    response = MetaProvider("muse-spark-1.2").complete(
        [{"role": "user", "content": "fix it"}],
        [{"function": {"name": "run_tests", "description": "test", "parameters": {}}}],
        effort="high",
    )

    assert seen["url"] == "https://api.meta.ai/v1/responses"
    assert seen["headers"] == {
        "Authorization": "Bearer meta-test",
        "Content-Type": "application/json",
    }
    payload = seen["payload"]
    assert isinstance(payload, dict)
    assert payload["model"] == "muse-spark-1.2"
    assert payload["reasoning"] == {"effort": "high"}
    assert payload["tools"][0]["name"] == "run_tests"
    assert response.tool_calls[0].name == "run_tests"
    # 500 uncached + 500 cached at Meta's 0.12 relative standard-tier rate.
    assert response.usage.prompt_tokens == 560
    assert response.usage.completion_tokens == 25


def test_meta_complete_requires_key(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("MODEL_API_KEY", raising=False)
    with pytest.raises(P.ProviderError, match="MODEL_API_KEY"):
        MetaProvider("muse-spark-1.2").complete([], [])


def test_anthropic_complete_parses(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ANTHROPIC_API_KEY", "sk-ant")

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        assert "/v1/messages" in url
        return {
            "content": [
                {"type": "text", "text": "thinking"},
                {"type": "tool_use", "id": "u1", "name": "edit_file", "input": {"path": "z"}},
            ],
            "usage": {"input_tokens": 5, "output_tokens": 2},
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = AnthropicProvider("claude-opus-4-8").complete(
        [{"role": "user", "content": "hi"}], [{"function": {"name": "edit_file", "parameters": {}}}]
    )
    assert resp.content == "thinking"
    assert resp.tool_calls[0].name == "edit_file"
    assert resp.usage.total == 7


def test_anthropic_requires_key(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)
    with pytest.raises(P.ProviderError, match="ANTHROPIC_API_KEY"):
        AnthropicProvider("claude-opus-4-8").complete([], [])


def test_anthropic_effort_and_no_sampling_params(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ANTHROPIC_API_KEY", "sk-ant")
    seen: dict = {}  # type: ignore[type-arg]

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen.update(payload)
        return {"content": [{"type": "text", "text": "ok"}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = AnthropicProvider("claude-opus-4-8").complete(
        [{"role": "user", "content": "hi"}], [], effort="low"
    )
    assert resp.content == "ok"
    assert seen["output_config"] == {"effort": "low"}
    # Sampling params are rejected with a 400 by Opus 4.7+ — never send them.
    assert "temperature" not in seen
    assert "top_p" not in seen


def test_anthropic_no_effort_omits_output_config(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ANTHROPIC_API_KEY", "sk-ant")
    seen: dict = {}  # type: ignore[type-arg]

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen.update(payload)
        return {"content": [{"type": "text", "text": "ok"}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    AnthropicProvider("claude-opus-4-8").complete([{"role": "user", "content": "hi"}], [])
    assert "output_config" not in seen


def test_zai_complete_parses_tool_calls(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ZAI_API_KEY", "zai-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        assert payload["tools"]
        return {
            "choices": [
                {
                    "message": {
                        "content": "ok",
                        "tool_calls": [
                            {
                                "id": "c1",
                                "function": {"name": "read_file", "arguments": '{"path": "a"}'},
                            }
                        ],
                    }
                }
            ],
            "usage": {"prompt_tokens": 11, "completion_tokens": 3},
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = ZaiProvider("glm-5.2").complete(
        [{"role": "user", "content": "hi"}], [{"function": {"name": "read_file"}}]
    )
    assert "chat/completions" in seen["url"]
    assert resp.content == "ok"
    assert resp.tool_calls[0].name == "read_file"
    assert resp.tool_calls[0].arguments == {"path": "a"}
    assert resp.usage.total == 14


def test_zai_complete_uses_custom_base_url(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ZAI_API_KEY", "zai-test")
    monkeypatch.setenv("ZAI_BASE_URL", "https://custom.z.ai/v1")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        return {"choices": [{"message": {"content": "ok"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = ZaiProvider("glm-5.2").complete([], [])
    assert seen["url"] == "https://custom.z.ai/v1/chat/completions"
    assert resp.content == "ok"


def test_zai_complete_requires_key(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("ZAI_API_KEY", raising=False)
    with pytest.raises(P.ProviderError, match="ZAI_API_KEY"):
        ZaiProvider("glm-5.2").complete([], [])


def test_zai_ignores_effort(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ZAI_API_KEY", "zai-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        return {"choices": [{"message": {"content": "ok"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = ZaiProvider("glm-5.2").complete([], [], effort="high")
    assert "chat/completions" in seen["url"]
    assert "responses" not in str(seen["url"])
    assert resp.content == "ok"


def test_anthropic_max_tokens_scales_with_effort(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ANTHROPIC_API_KEY", "ant-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["max_tokens"] = payload["max_tokens"]
        return {"content": [{"type": "text", "text": "ok"}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    AnthropicProvider("claude-opus-5").complete([{"role": "user", "content": "hi"}], [])
    assert seen["max_tokens"] == 32_000
    AnthropicProvider("claude-opus-5").complete(
        [{"role": "user", "content": "hi"}], [], effort="xhigh"
    )
    assert seen["max_tokens"] == 128_000
    AnthropicProvider("claude-opus-5").complete(
        [{"role": "user", "content": "hi"}], [], effort="medium"
    )
    assert seen["max_tokens"] == 64_000


def test_anthropic_truncated_response_raises_without_retry(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # A step whose whole output budget went to thinking carries no action;
    # it must raise (harness-visible) and must NOT be retried (re-bills the
    # same failure at up to 128K output tokens per attempt).
    monkeypatch.setenv("ANTHROPIC_API_KEY", "ant-test")
    calls = {"n": 0}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        calls["n"] += 1
        return {"content": [], "stop_reason": "max_tokens", "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    with pytest.raises(P.NonRetryableProviderError, match="max_tokens"):
        AnthropicProvider("claude-fable-5").complete([{"role": "user", "content": "hi"}], [])
    assert calls["n"] == 1


def test_anthropic_truncated_with_tool_call_still_returns(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # If a complete tool_use block survived the cutoff the step is actionable.
    monkeypatch.setenv("ANTHROPIC_API_KEY", "ant-test")

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        return {
            "content": [{"type": "tool_use", "id": "t1", "name": "run_tests", "input": {}}],
            "stop_reason": "max_tokens",
            "usage": {},
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = AnthropicProvider("claude-fable-5").complete([{"role": "user", "content": "hi"}], [])
    assert resp.tool_calls[0].name == "run_tests"


def test_kimi_complete_omits_temperature(monkeypatch: pytest.MonkeyPatch) -> None:
    # kimi-k3 rejects sampling params; the payload must not carry temperature.
    monkeypatch.setenv("MOONSHOT_API_KEY", "kimi-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        seen["payload"] = payload
        seen["auth"] = headers["Authorization"]
        return {
            "choices": [
                {
                    "message": {
                        "content": "ok",
                        "tool_calls": [
                            {
                                "id": "c1",
                                "function": {"name": "read_file", "arguments": '{"path": "a"}'},
                            }
                        ],
                    }
                }
            ],
            "usage": {"prompt_tokens": 11, "completion_tokens": 3},
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = KimiProvider("kimi-k3").complete(
        [{"role": "user", "content": "hi"}], [{"function": {"name": "read_file"}}]
    )
    assert seen["url"] == "https://api.moonshot.ai/v1/chat/completions"
    assert seen["auth"] == "Bearer kimi-test"
    assert "temperature" not in seen["payload"]  # type: ignore[operator]
    assert resp.tool_calls[0].name == "read_file"
    assert resp.usage.total == 14


def test_kimi_complete_uses_custom_base_url(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("MOONSHOT_API_KEY", "kimi-test")
    monkeypatch.setenv("MOONSHOT_BASE_URL", "https://api.moonshot.cn/v1")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        return {"choices": [{"message": {"content": "ok"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = KimiProvider("kimi-k3").complete([], [])
    assert seen["url"] == "https://api.moonshot.cn/v1/chat/completions"
    assert resp.content == "ok"


def test_http_post_json_wraps_read_timeout(monkeypatch: pytest.MonkeyPatch) -> None:
    # A socket timeout during resp.read() raises raw TimeoutError (not
    # URLError); it must become a retryable ProviderError.
    def fake_urlopen(req, timeout=120):  # type: ignore[no-untyped-def]
        raise TimeoutError("The read operation timed out")

    monkeypatch.setattr(P.urllib.request, "urlopen", fake_urlopen)
    with pytest.raises(P.ProviderError, match="read timeout"):
        P._http_post_json("https://example.invalid/v1/chat/completions", {}, {})


def test_kimi_complete_requires_key(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("MOONSHOT_API_KEY", raising=False)
    with pytest.raises(P.ProviderError, match="MOONSHOT_API_KEY"):
        KimiProvider("kimi-k3").complete([], [])


def test_kimi_sends_reasoning_effort_when_given(monkeypatch: pytest.MonkeyPatch) -> None:
    # The loop only passes effort when effort_config says supported, and it
    # passes the provider value ("max"); the provider sends it verbatim.
    monkeypatch.setenv("MOONSHOT_API_KEY", "kimi-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["payload"] = payload
        return {"choices": [{"message": {"content": "ok"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    KimiProvider("kimi-k3").complete([], [], effort="max")
    assert seen["payload"]["reasoning_effort"] == "max"  # type: ignore[index]
    KimiProvider("kimi-k3").complete([], [])
    assert "reasoning_effort" not in seen["payload"]  # type: ignore[operator]


def test_qwen_complete_parses_tool_calls(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("DASHSCOPE_API_KEY", "qwen-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        seen["auth"] = headers["Authorization"]
        assert payload["tools"]
        return {
            "choices": [
                {
                    "message": {
                        "content": "ok",
                        "tool_calls": [
                            {
                                "id": "c1",
                                "function": {"name": "read_file", "arguments": '{"path": "a"}'},
                            }
                        ],
                    }
                }
            ],
            "usage": {"prompt_tokens": 11, "completion_tokens": 3},
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = QwenProvider("qwen3.7-plus").complete(
        [{"role": "user", "content": "hi"}], [{"function": {"name": "read_file"}}]
    )
    assert seen["url"] == (
        "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions"
    )
    assert seen["auth"] == "Bearer qwen-test"
    assert resp.content == "ok"
    assert resp.tool_calls[0].name == "read_file"
    assert resp.tool_calls[0].arguments == {"path": "a"}
    assert resp.usage.total == 14


def test_qwen_complete_uses_custom_base_url(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("DASHSCOPE_API_KEY", "qwen-test")
    monkeypatch.setenv("DASHSCOPE_BASE_URL", "https://dashscope.aliyuncs.com/compatible-mode/v1")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        return {"choices": [{"message": {"content": "ok"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = QwenProvider("qwen3.7-plus").complete([], [])
    assert seen["url"] == "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
    assert resp.content == "ok"


def test_qwen_complete_requires_key(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("DASHSCOPE_API_KEY", raising=False)
    with pytest.raises(P.ProviderError, match="DASHSCOPE_API_KEY"):
        QwenProvider("qwen3.7-plus").complete([], [])


def test_qwen_sends_reasoning_effort_when_given(monkeypatch: pytest.MonkeyPatch) -> None:
    # The loop only passes effort when effort_config says supported, and it
    # passes the provider value ("low"/"medium"/"xhigh"); sent verbatim.
    monkeypatch.setenv("DASHSCOPE_API_KEY", "qwen-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["payload"] = payload
        return {"choices": [{"message": {"content": "ok"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    QwenProvider("qwen3.8-max").complete([], [], effort="xhigh")
    assert seen["payload"]["reasoning_effort"] == "xhigh"  # type: ignore[index]
    QwenProvider("qwen3.8-max").complete([], [])
    assert "reasoning_effort" not in seen["payload"]  # type: ignore[operator]
    assert "enable_thinking" not in seen["payload"]  # type: ignore[operator]


def test_deepseek_complete_parses_tool_calls(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("DEEPSEEK_API_KEY", "ds-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        seen["auth"] = headers["Authorization"]
        assert payload["tools"]
        return {
            "choices": [
                {
                    "message": {
                        "content": "ok",
                        "tool_calls": [
                            {
                                "id": "c1",
                                "function": {"name": "read_file", "arguments": '{"path": "a"}'},
                            }
                        ],
                    }
                }
            ],
            "usage": {"prompt_tokens": 11, "completion_tokens": 3},
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = DeepSeekProvider("deepseek-v4-flash").complete(
        [{"role": "user", "content": "hi"}], [{"function": {"name": "read_file"}}]
    )
    assert seen["url"] == "https://api.deepseek.com/chat/completions"
    assert seen["auth"] == "Bearer ds-test"
    assert resp.content == "ok"
    assert resp.tool_calls[0].name == "read_file"
    assert resp.tool_calls[0].arguments == {"path": "a"}
    assert resp.usage.total == 14


def test_deepseek_complete_uses_custom_base_url(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("DEEPSEEK_API_KEY", "ds-test")
    monkeypatch.setenv("DEEPSEEK_BASE_URL", "https://api.deepseek.com/v1")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        return {"choices": [{"message": {"content": "ok"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = DeepSeekProvider("deepseek-v4-flash").complete([], [])
    assert seen["url"] == "https://api.deepseek.com/v1/chat/completions"
    assert resp.content == "ok"


def test_deepseek_complete_requires_key(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("DEEPSEEK_API_KEY", raising=False)
    with pytest.raises(P.ProviderError, match="DEEPSEEK_API_KEY"):
        DeepSeekProvider("deepseek-v4-flash").complete([], [])


def test_deepseek_sends_reasoning_effort_when_given(monkeypatch: pytest.MonkeyPatch) -> None:
    # The loop only passes effort when effort_config says supported, and it
    # passes the provider value ("low"/"high"/"max"); sent verbatim.
    monkeypatch.setenv("DEEPSEEK_API_KEY", "ds-test")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["payload"] = payload
        return {"choices": [{"message": {"content": "ok"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    DeepSeekProvider("deepseek-v4-flash").complete([], [], effort="high")
    assert seen["payload"]["reasoning_effort"] == "high"  # type: ignore[index]
    DeepSeekProvider("deepseek-v4-flash").complete([], [])
    assert "reasoning_effort" not in seen["payload"]  # type: ignore[operator]


def test_providers_do_not_stream_yet() -> None:
    assert get_provider("mock:synthetic").supports_streaming is False
    assert OpenAIProvider("gpt-4o").supports_streaming is False
    assert AnthropicProvider("claude-opus-4-8").supports_streaming is False
    assert ZaiProvider("glm-5.2").supports_streaming is False
    assert KimiProvider("kimi-k3").supports_streaming is False
    assert DeepSeekProvider("deepseek-v4-flash").supports_streaming is False
    assert QwenProvider("qwen3.7-plus").supports_streaming is False


# --- per-request timeout ceiling -------------------------------------------
#
# Regression cover for a bug where the run's *remaining budget* was used as the
# socket timeout for a single request, and supplying a budget also disabled
# retries. One stalled request then consumed an entire task: observed runs spent
# 21s working and 1785s blocked on one call, scoring 0.


def test_http_timeout_caps_a_large_budget() -> None:
    # A generous remaining budget must not become the socket timeout.
    assert P._http_timeout(1800.0) == P.DEFAULT_MAX_REQUEST_TIMEOUT_S


def test_http_timeout_respects_a_smaller_budget() -> None:
    # The budget still bounds the request when it is the tighter constraint.
    assert P._http_timeout(30.0) == 30.0


def test_http_timeout_honours_a_provider_specific_cap() -> None:
    assert P._http_timeout(1800.0, 900.0) == 900.0


def test_http_timeout_rejects_an_exhausted_budget() -> None:
    with pytest.raises(P.ProviderError):
        P._http_timeout(0)


def test_budgeted_attempts_fit_the_remaining_budget() -> None:
    assert P._budgeted_attempts(None, 300.0) == P._MAX_ATTEMPTS  # no budget: full allowance
    assert P._budgeted_attempts(1800.0, 300.0) == P._MAX_ATTEMPTS  # room for the cap
    assert P._budgeted_attempts(600.0, 300.0) == 2  # only two requests fit
    assert P._budgeted_attempts(100.0, 300.0) == 1  # never fewer than one


def test_request_ceiling_clears_the_slowest_observed_real_response() -> None:
    # Round trips grow with context: the slowest response seen actually complete
    # was 333s, on an xlarge repo. A ceiling at or below that aborts real work.
    assert P.DEFAULT_MAX_REQUEST_TIMEOUT_S > 333


def test_stalled_request_is_retried_rather_than_consuming_the_run(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("ANTHROPIC_API_KEY", "k")
    calls: list[float] = []

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        calls.append(timeout)
        if len(calls) == 1:  # first attempt stalls out
            raise P.ProviderError(f"read timeout after {timeout:.0f}s calling {url}")
        return {"content": [{"type": "text", "text": "ok"}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    monkeypatch.setattr(P, "_WAIT", lambda *a, **k: 0)
    resp = AnthropicProvider("claude-opus-5").complete([], [], timeout_s=1800.0)

    assert resp.content == "ok"
    assert len(calls) == 2, "a stalled request must be retried"
    assert all(t == P.DEFAULT_MAX_REQUEST_TIMEOUT_S for t in calls), calls
