"""Router provider, routing ledger, router economics and quota snapshots."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

import harness.agent.providers as P
from harness.agent.loop import run_agent
from harness.agent.providers import (
    ProviderError,
    RouterProvider,
    reports_live_cost,
    route_manifest,
)
from harness.agent.quota_probe import (
    parse_rate_limit_info,
    parse_stream_lines,
    quota_block,
    window_delta,
)
from harness.agent.routing import RoutingLedger
from harness.economics import router_receipt, subscription_receipt
from harness.effort import effort_config

ROOT = Path(__file__).resolve().parents[1]


def test_chat_completions_parse_keeps_served_model_and_cost() -> None:
    body = {
        "id": "gen-123",
        "model": "anthropic/claude-haiku-4.5",
        "choices": [{"message": {"content": "ok"}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 2, "cost": 0.00042},
    }
    resp = P._parse_chat_completions_response(body)
    assert resp.served_model == "anthropic/claude-haiku-4.5"
    assert resp.response_id == "gen-123"
    assert resp.reported_cost_usd == pytest.approx(0.00042)


def test_chat_completions_parse_without_cost_is_none_not_zero() -> None:
    body = {"choices": [{"message": {"content": "ok"}}], "usage": {"prompt_tokens": 1}}
    resp = P._parse_chat_completions_response(body)
    assert resp.served_model is None
    assert resp.reported_cost_usd is None


def test_router_provider_reads_named_env_and_sends_usage_include(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("ROUTER_MARTIAN_BASE_URL", "https://router.example/v1/")
    monkeypatch.setenv("ROUTER_MARTIAN_API_KEY", "rk-test-0123456789")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["url"] = url
        seen["payload"] = payload
        seen["auth"] = headers["Authorization"]
        return {
            "id": "r1",
            "model": "openai/gpt-6.1-terra",
            "choices": [{"message": {"content": "hi"}}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 1, "cost": 0.001},
        }

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    resp = RouterProvider("martian/auto").complete([{"role": "user", "content": "x"}], [])
    assert seen["url"] == "https://router.example/v1/chat/completions"
    assert seen["auth"] == "Bearer rk-test-0123456789"
    payload = seen["payload"]
    assert isinstance(payload, dict)
    assert payload["model"] == "auto"
    assert payload["usage"] == {"include": True}
    assert "temperature" not in payload
    assert resp.served_model == "openai/gpt-6.1-terra"
    assert resp.reported_cost_usd == 0.001


def test_router_provider_usage_include_can_be_disabled(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ROUTER_NOTDIAMOND_BASE_URL", "https://nd.example/v1")
    monkeypatch.setenv("ROUTER_NOTDIAMOND_API_KEY", "nd-key-0123456789")
    monkeypatch.setenv("ROUTER_NOTDIAMOND_USAGE_INCLUDE", "0")
    seen: dict[str, object] = {}

    def fake_post(url, headers, payload, timeout=120):  # type: ignore[no-untyped-def]
        seen["payload"] = payload
        return {"choices": [{"message": {"content": "hi"}}], "usage": {}}

    monkeypatch.setattr(P, "_http_post_json", fake_post)
    RouterProvider("notdiamond/router").complete([{"role": "user", "content": "x"}], [])
    assert "usage" not in seen["payload"]  # type: ignore[operator]


def test_router_provider_requires_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("ROUTER_X_API_KEY", raising=False)
    with pytest.raises(ProviderError, match="ROUTER_X_API_KEY"):
        RouterProvider("x/auto").complete([{"role": "user", "content": "x"}], [])
    monkeypatch.setenv("ROUTER_X_API_KEY", "k-0123456789")
    monkeypatch.delenv("ROUTER_X_BASE_URL", raising=False)
    with pytest.raises(ProviderError, match="ROUTER_X_BASE_URL"):
        RouterProvider("x/auto").complete([{"role": "user", "content": "x"}], [])


def test_router_model_must_name_router() -> None:
    with pytest.raises(ProviderError, match="<name>/<model>"):
        RouterProvider("auto").complete([{"role": "user", "content": "x"}], [])


def test_router_route_manifest_and_effort(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ROUTER_MARTIAN_BASE_URL", "https://router.example/v1")
    manifest = route_manifest("router:martian/auto")
    assert manifest == {
        "via": "router",
        "router": "martian",
        "base_url": "https://router.example/v1",
        "wire_model": "auto",
        "pinned": False,
    }
    cfg = effort_config("router", "high", "martian/auto")
    assert cfg.requested == "high" and cfg.supported is False


def test_routing_ledger_counts_switches_and_withholds_partial_cost() -> None:
    ledger = RoutingLedger()
    ledger.record("a", 0.1)
    ledger.record("a", 0.2)
    ledger.record("b", 0.3)
    assert ledger.served == {"a": 2, "b": 1}
    assert ledger.switches == 1
    assert ledger.router_reported_cost_usd == pytest.approx(0.6)
    ledger.record("b", None)
    assert ledger.router_reported_cost_usd is None
    assert ledger.as_summary()["router_cost_steps"] == 3


def test_run_agent_records_routing_and_router_economics(tmp_path: Path) -> None:
    res = run_agent(
        task_id="hello-world",
        model="mock:routed",
        output_dir=tmp_path,
        max_steps=6,
        tasks_root=ROOT / "tasks" / "v1",
    )
    summary = res["summary"]
    assert summary["scores"]["functional"] == 1.0
    assert set(summary["served_models"]) == {"mock/cheap", "mock/frontier"}
    assert summary["served_model_switches"] >= 1
    steps = summary["steps"]
    assert summary["router_reported_cost_usd"] == pytest.approx(0.001 * steps)
    assert summary["cost_detail"]["cost_basis"] == "router-reported"
    assert summary["cost_detail"]["agent"] == pytest.approx(0.001 * steps)
    assert summary["economics"]["cost_basis"] == "router-reported"
    assert summary["economics"]["marginal_cash_usd"] == pytest.approx(summary["cost_usd"])
    run_dir = tmp_path / res["run_id"]
    events = [json.loads(line) for line in (run_dir / "trace.jsonl").read_text().splitlines()]
    responses = [e for e in events if e["type"] == "llm_response"]
    assert responses[0]["data"]["usage"]["served_model"] == "mock/cheap"
    assert responses[0]["data"]["usage"]["reported_cost_usd"] == 0.001


def test_plain_mock_run_has_empty_routing(tmp_path: Path) -> None:
    summary = run_agent(
        task_id="hello-world",
        model="mock:synthetic",
        output_dir=tmp_path,
        max_steps=6,
        tasks_root=ROOT / "tasks" / "v1",
    )["summary"]
    assert summary["served_models"] == {}
    assert summary["router_reported_cost_usd"] is None
    assert summary["economics"]["cost_basis"] == "metered-api-pricing"


def test_router_receipt_keeps_list_price_counterfactual() -> None:
    receipt = router_receipt(
        reported_cost_usd=0.5, judges_cost_usd=0.0, api_equivalent_cost_usd=0.9
    ).as_summary()
    assert receipt["billing_mode"] == "api-metered"
    assert receipt["cost_basis"] == "router-reported"
    assert receipt["marginal_cash_usd"] == 0.5
    assert receipt["api_equivalent_cost_usd"] == 0.9
    assert receipt["measurement_quality"]["marginal_cash_usd"] == "router-reported"


RATE_LIMIT_LINE = json.dumps(
    {
        "type": "rate_limit_event",
        "rate_limit_info": {
            "status": "allowed",
            "isUsingOverage": False,
            "unifiedWindows": {
                "five_hour": {"utilization": 0.13, "resetsAt": 1760000000},
                "seven_day": {"utilization": 0.541},
            },
        },
    }
)


def test_parse_rate_limit_info_normalizes_windows() -> None:
    parsed = parse_rate_limit_info(json.loads(RATE_LIMIT_LINE)["rate_limit_info"])
    assert parsed is not None
    assert parsed["status"] == "allowed" and parsed["overage"] is False
    assert parsed["windows"]["five_hour"] == {"utilization": 0.13, "resets_at": 1760000000}
    assert parsed["windows"]["seven_day"] == {"utilization": 0.541}
    assert parse_rate_limit_info({"status": "allowed"}) is None
    assert parse_rate_limit_info("junk") is None


def test_parse_stream_lines_takes_last_event() -> None:
    later = json.loads(RATE_LIMIT_LINE)
    later["rate_limit_info"]["unifiedWindows"]["seven_day"]["utilization"] = 0.55
    lines = ['{"type": "system"}', RATE_LIMIT_LINE, "not json", json.dumps(later)]
    parsed = parse_stream_lines(lines)
    assert parsed is not None
    assert parsed["windows"]["seven_day"]["utilization"] == 0.55
    assert parse_stream_lines(['{"type": "system"}']) is None


def test_window_delta_and_quota_block_prefer_probe() -> None:
    before = parse_rate_limit_info(json.loads(RATE_LIMIT_LINE)["rate_limit_info"])
    after_raw = json.loads(RATE_LIMIT_LINE)["rate_limit_info"]
    after_raw["unifiedWindows"]["five_hour"]["utilization"] = 0.05  # window reset
    after_raw["unifiedWindows"]["seven_day"]["utilization"] = 0.56
    after = parse_rate_limit_info(after_raw)
    assert window_delta(before, after) == {"five_hour": None, "seven_day": pytest.approx(0.019)}
    assert window_delta(before, None) is None

    block = quota_block(
        stream_first=before, stream_last=before, probe_before=before, probe_after=after
    )
    assert block is not None
    assert block["source"] == "probe"
    assert block["delta"]["seven_day"] == pytest.approx(0.019)
    assert block["before"]["seven_day"] == 0.541
    stream_only = quota_block(stream_first=before, stream_last=after)
    assert stream_only is not None and stream_only["source"] == "stream"
    assert quota_block(stream_first=None, stream_last=None) is None


def test_subscription_receipt_carries_quota() -> None:
    block = {"source": "stream", "before": {}, "after": {}, "delta": None}
    receipt = subscription_receipt(
        api_equivalent_cost_usd=1.0,
        grading_cash_usd=None,
        grading_api_equivalent_usd=0.0,
        plan_name="max",
        cli_reported_cost_usd=1.0,
        quota=block,
    ).as_summary()
    assert receipt["quota"] == block
    assert receipt["measurement_quality"]["quota"] == "in-stream-windows"
    none = subscription_receipt(
        api_equivalent_cost_usd=1.0,
        grading_cash_usd=None,
        grading_api_equivalent_usd=0.0,
        plan_name="max",
        cli_reported_cost_usd=1.0,
    ).as_summary()
    assert none["measurement_quality"]["quota"] == "unavailable"


def test_reports_live_cost_only_for_gateways() -> None:
    assert reports_live_cost("router:martian/auto")
    assert reports_live_cost("openrouter:openrouter/auto")
    assert not reports_live_cost("openai:gpt-6.1-terra")
    assert not reports_live_cost("not-a-spec")
