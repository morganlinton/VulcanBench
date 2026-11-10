import json

import pytest

from harness.solver_receipts import claude_receipts, solver_receipt


def receipt(ident, cost, output=10):
    return {
        "uuid": ident,
        "session_id": "s",
        "subtype": "success",
        "is_error": False,
        "total_cost_usd": cost,
        "usage": {
            "input_tokens": 1,
            "cache_read_input_tokens": 100,
            "cache_creation_input_tokens": 20,
            "output_tokens": output,
        },
    }


def test_raw_tokens_not_price_discounted():
    assert claude_receipts([receipt("a", 2)])["raw_tokens"] == 131


def test_multiturn_usage_summed_but_cumulative_cost_not_doubled():
    result = claude_receipts([receipt("a", 2), receipt("b", 3)])
    assert result["raw_tokens"] == 262
    assert result["cli_api_equivalent_estimate_usd"] == 3


def test_duplicate_receipt_rejected():
    with pytest.raises(ValueError, match="Duplicate"):
        claude_receipts([receipt("a", 2), receipt("a", 2)])


def test_decreasing_cumulative_cost_rejected():
    with pytest.raises(ValueError, match="cumulative"):
        claude_receipts([receipt("a", 3), receipt("b", 2)])


def test_fable_52_sums_every_result_and_its_thinking_tokens(tmp_path):
    first, second = receipt("a", 1.26, output=12654), receipt("b", 1.34, output=982)
    first["usage"]["output_tokens_details"] = {"thinking_tokens": 7000}
    second["usage"]["output_tokens_details"] = {"thinking_tokens": 0}
    lines = [
        {"type": "system", "subtype": "init"},
        {"type": "result", **first},
        {"type": "system", "subtype": "init"},
        {"type": "result", **second},
    ]
    (tmp_path / "cli-agent-stream.jsonl").write_text("".join(json.dumps(e) + "\n" for e in lines))
    result = solver_receipt(tmp_path, {"model": "claude-code:claude-fable-5-2"})
    assert result["result_receipts"] == 2
    assert result["usage"]["output_tokens"] == 13636
    assert result["thinking_tokens"] == 7000
    assert result["cli_api_equivalent_estimate_usd"] == 1.34
