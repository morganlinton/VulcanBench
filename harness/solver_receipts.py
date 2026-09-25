"""Additive raw-token accounting from original CLI result receipts.

Fable's historical summary folds cache tokens into price-equivalent units.
This reader does not alter those summaries. It sums per-turn usage receipts
and takes only the final cumulative CLI cost for each Claude session.
"""

import json
import math

from harness.retrospective_judging import digest

CLAUDE_KEYS = (
    "input_tokens",
    "cache_read_input_tokens",
    "cache_creation_input_tokens",
    "output_tokens",
)


def claude_receipts(results):
    if not results:
        raise ValueError("Missing CLI result")
    tokens = {key: 0 for key in CLAUDE_KEYS}
    costs = {}
    seen = set()
    for event in results:
        if event.get("subtype") != "success" or event.get("is_error"):
            raise ValueError("Unsuccessful solver receipt")
        identity = event.get("uuid")
        if identity is not None:
            if identity in seen:
                raise ValueError("Duplicate solver receipt")
            seen.add(identity)
        for key in tokens:
            value = event["usage"][key]
            if type(value) is not int or value < 0:
                raise ValueError("Invalid raw solver token receipt")
            tokens[key] += value
        session = event["session_id"]
        cost = event["total_cost_usd"]
        if (
            not isinstance(cost, (int, float))
            or not math.isfinite(cost)
            or cost < costs.get(session, 0)
        ):
            raise ValueError("Invalid cumulative CLI cost")
        costs[session] = cost
    return {
        "raw_tokens": sum(tokens.values()),
        "usage": tokens,
        "result_receipts": len(results),
        "cli_api_equivalent_estimate_usd": sum(costs.values()),
        "sessions": len(costs),
    }


def devin_receipt(run, summary):
    # Devin's print mode streams no usage; the adapter harvests per-request
    # receipts from the CLI's session store and records their sum in the trace.
    usage = None
    with (run / "trace.jsonl").open() as trace:
        for line in trace:
            event = json.loads(line)
            if event.get("type") == "devin_usage":
                usage = event["data"]
    if usage is None:
        raise ValueError("Missing Devin usage receipt")
    keys = ("input_tokens", "output_tokens", "cache_read_tokens", "cache_creation_tokens")
    for key in keys:
        if type(usage[key]) is not int or usage[key] < 0:
            raise ValueError("Invalid Devin token receipt")
    prompt = usage["input_tokens"] + usage["cache_read_tokens"] + usage["cache_creation_tokens"]
    tokens = summary["tokens"]
    if prompt != tokens["prompt"] or usage["output_tokens"] != tokens["completion"]:
        raise ValueError("Devin receipt/summary mismatch")
    result = {
        "raw_tokens": prompt + usage["output_tokens"],
        "usage": {key: usage[key] for key in keys},
        "result_receipts": usage["requests"],
        "served_models": usage.get("served_models"),
        "devin_credit_cost": usage.get("total_credit_cost"),
        "devin_acu_cost": usage.get("total_acu_cost"),
        "historical_summary_unit": "Raw input (uncached, cache read, cache creation) plus output",
    }
    return result


def solver_receipt(run, summary):
    path = run / "cli-agent-stream.jsonl"
    events = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    if summary["model"] == "claude-code:claude-fable-5-1":
        results = [e for e in events if e.get("type") == "result"]
        result = claude_receipts(results)
        last = results[-1]["usage"]
        effective = (
            round(
                last["input_tokens"]
                + last["cache_read_input_tokens"] * 0.1
                + last["cache_creation_input_tokens"] * 1.25
            )
            + last["output_tokens"]
        )
        if effective != summary["total_tokens"]:
            raise ValueError("Historical token summary no longer matches its cache-price fold")
        result["historical_summary_unit"] = (
            "Cache-price-weighted units from last result, not raw tokens"
        )
    elif summary["model"].startswith("codex:"):
        receipts = [e["usage"] for e in events if e.get("type") == "turn.completed"]
        if len(receipts) != 1:
            raise ValueError("Unexpected Astra result count")
        usage = receipts[0]
        total = usage["input_tokens"] + usage["output_tokens"]
        if total != summary["total_tokens"] or usage["cached_input_tokens"] > usage["input_tokens"]:
            raise ValueError("Astra receipt/summary mismatch")
        result = {
            "raw_tokens": total,
            "usage": usage,
            "result_receipts": 1,
            "historical_summary_unit": "Raw input plus output; cached input included in input",
        }
    elif summary["model"].startswith("devin:"):
        result = devin_receipt(run, summary)
    elif summary["model"].startswith("muse-code:"):
        # Muse Code folds its per-request usage into the run summary's token block
        # (prompt includes cache reads; completion includes reasoning output).
        tokens = summary["tokens"]
        for key in ("prompt", "completion", "cached_input"):
            if type(tokens.get(key)) is not int or tokens[key] < 0:
                raise ValueError("Invalid Muse token summary")
        if tokens["prompt"] + tokens["completion"] != summary["total_tokens"]:
            raise ValueError("Muse token summary does not sum to its total")
        result = {
            "raw_tokens": tokens["prompt"] + tokens["completion"],
            "usage": {
                "input_tokens": tokens["prompt"],
                "cached_input_tokens": tokens["cached_input"],
                "output_tokens": tokens["completion"],
                "reasoning_output_tokens": tokens.get("reasoning_output", 0),
            },
            "result_receipts": None,
            "historical_summary_unit": "Raw input plus output; cached input included in input",
        }
    else:
        raise ValueError("Unknown solver")
    return {**result, "stream_sha256": digest(path.read_bytes())}
