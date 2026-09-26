"""Read-only, cache-aware API-equivalent solver cost ledger.

The frozen benchmark scores and original usage receipts are not modified.
Astra receipts aggregate API calls: publish standard-rate estimates plus a
conservative long-context bound, not an exact reconstructed invoice. Claude
modelUsage is cumulative within a session: retain only the final receipt and
price each actual model, including fallbacks and auxiliary CLI calls.
"""

from __future__ import annotations

import csv
import json
import math
import statistics
from collections import Counter, defaultdict
from pathlib import Path

from harness.panel_comparison import OUTPUT, read, require
from harness.retrospective_judging import LEVELS, digest, save

VERIFIED_DATE = "2026-09-22"
SOURCES = {
    "astra": "https://developers.openai.com/api/docs/models/gpt-6-astra",
    "claude": "https://platform.claude.com/docs/en/about-claude/pricing",
    "fable": "https://platform.claude.com/docs/en/models/fable-5-1/overview",
}
# USD per million tokens: uncached input, cache read, 5m write, 1h write, output.
CLAUDE_RATES = {
    "claude-fable-5-1": (10, 0.25, 12.5, 20, 50),
    "claude-opus-5-5": (4, 0.2, 5, 8, 20),
    "claude-opus-5": (5, 0.5, 6.25, 10, 25),
    "claude-opus-4-8": (5, 0.5, 6.25, 10, 25),
    "claude-haiku-4-5-20251001": (1, 0.1, 1.25, 2, 5),
}
ASTRA_RATES = {"input": 10, "cache_read": 1, "cache_write": 12.5, "output": 50}


def astra_cost(usage):
    for key in ("input_tokens", "cached_input_tokens", "output_tokens"):
        require(type(usage[key]) is int and usage[key] >= 0, "Invalid Astra usage")
    cached, writes = usage["cached_input_tokens"], usage.get("cache_write_input_tokens", 0)
    require(type(writes) is int and writes >= 0, "Invalid cache writes")
    # This corpus reports zero writes, so no inference about write inclusion is needed.
    require(writes == 0, "Nonzero cache writes require schema verification")
    uncached = usage["input_tokens"] - cached
    require(uncached >= 0, "Cache is a subset of Astra input")
    input_cost = (uncached * 10 + cached) / 1e6
    output_cost = usage["output_tokens"] * 50 / 1e6
    base = input_cost + output_cost
    # Do not apply the long-context threshold to cumulative input as if one request.
    upper = (2 * input_cost + 1.5 * output_cost) if usage["input_tokens"] > 272000 else base
    return {
        "estimated_usd": base,
        "long_context_upper_usd": upper,
        "input_cost_usd": input_cost,
        "output_cost_usd": output_cost,
        "method": "Standard rates; per-request long-context tier unavailable",
    }


def claude_model_cost(model, usage, writes_5m):
    require(model in CLAUDE_RATES, f"Unknown model price: {model}")
    ip, cr, w5, w1, op = CLAUDE_RATES[model]
    for key in ("inputTokens", "outputTokens", "cacheReadInputTokens", "cacheCreationInputTokens"):
        require(type(usage[key]) is int and usage[key] >= 0, "Invalid Claude usage")
    writes = usage["cacheCreationInputTokens"]
    require(0 <= writes_5m <= writes, "Unreconciled 5m cache writes")
    rest = (
        usage["inputTokens"] * ip + usage["cacheReadInputTokens"] * cr + usage["outputTokens"] * op
    )
    return {
        "calculated_usd": (rest + writes_5m * w5 + (writes - writes_5m) * w1) / 1e6,
        "all_5m_usd": (rest + writes * w5) / 1e6,
        "all_1h_usd": (rest + writes * w1) / 1e6,
    }


def claude_cost(events):
    final = {}
    ttl = defaultdict(Counter)
    messages = set()
    result_ids = set()
    for event in events:
        if event.get("type") == "assistant":
            msg = event["message"]
            key = (event["session_id"], msg["id"])
            if key not in messages:
                messages.add(key)
                ttl[event["session_id"], msg["model"]].update(
                    msg["usage"].get("cache_creation", {})
                )
        if event.get("type") != "result":
            continue
        require(
            event.get("subtype") == "success" and not event.get("is_error"), "Failed cost receipt"
        )
        require(event["uuid"] not in result_ids, "Duplicate cost receipt")
        result_ids.add(event["uuid"])
        require(
            event["usage"].get("service_tier") == "standard"
            and event["usage"].get("speed") == "standard"
            and event.get("fast_mode_state") == "off",
            "Nonstandard price modifier",
        )
        sid = event["session_id"]
        if sid in final:
            require(
                event["total_cost_usd"] >= final[sid]["total_cost_usd"], "Cumulative cost decreased"
            )
        final[sid] = event
    require(final, "Missing final Claude receipt")
    total = 0.0
    parts = []
    for sid, event in final.items():
        session_total = 0.0
        for model, usage in event["modelUsage"].items():
            require(
                usage["costBasis"] == "list" and usage["provider"] == "firstParty",
                "Unknown Claude cost basis",
            )
            require(usage.get("webSearchRequests", 0) == 0, "Server tool fee requires pricing")
            calculated = claude_model_cost(
                model, usage, ttl[sid, model]["ephemeral_5m_input_tokens"]
            )
            reported = usage["costUSD"]
            require(
                math.isfinite(reported)
                and calculated["all_5m_usd"] - 1e-6 <= reported <= calculated["all_1h_usd"] + 1e-6,
                "Receipt outside verified list-price bounds",
            )
            matched = math.isclose(calculated["calculated_usd"], reported, abs_tol=1e-6)
            # The only missing model-specific TTL is an internal Opus 5 call.
            require(
                matched
                or (
                    model == "claude-opus-5"
                    and math.isclose(calculated["all_5m_usd"], reported, abs_tol=1e-6)
                ),
                "Unexpected rate reconciliation difference",
            )
            parts.append(
                {
                    "model": model,
                    "session_id": sid,
                    "usage": usage,
                    "observed_cache_ttl": dict(ttl[sid, model]),
                    **calculated,
                    "estimated_usd": calculated["calculated_usd"] if matched else reported,
                    "method": "Recomputed from verified rates and cache split"
                    if matched
                    else "CLI list receipt retained; auxiliary cache TTL absent, matches 5m pricing",
                }
            )
            session_total += parts[-1]["estimated_usd"]
        require(
            math.isclose(session_total, event["total_cost_usd"], abs_tol=1e-6),
            "Session cost mismatch",
        )
        total += session_total
    return {
        "estimated_usd": total,
        "parts": parts,
        "sessions": len(final),
        "result_receipts": len(result_ids),
        "method": "Final per-session modelUsage, cache-aware list rates",
    }


def main():
    source = OUTPUT / "comparison.json"
    data = read(source)
    require(data["complete"] and data["runs"] == 230, "Incomplete comparison")
    rows = []
    for row in data["rows"]:
        stream = Path(row["source_directory"]) / "cli-agent-stream.jsonl"
        raw = stream.read_bytes()
        require(digest(raw) == row["solver_receipt"]["stream_sha256"], "Changed source stream")
        if row["model"] == "astra":
            cost = astra_cost(row["solver_receipt"]["usage"])
        else:
            cost = claude_cost([json.loads(line) for line in raw.splitlines() if line.strip()])
            require(
                math.isclose(
                    cost["estimated_usd"],
                    row["solver_receipt"]["cli_api_equivalent_estimate_usd"],
                    abs_tol=1e-6,
                ),
                "Frozen receipt mismatch",
            )
        rows.append(
            {
                "model": row["model"],
                "effort": row["effort"],
                "task": row["task"],
                "run_id": row["run_id"],
                "stream_sha256": digest(raw),
                **cost,
            }
        )
    groups = []
    for model in ("astra", "fable"):
        for effort in LEVELS:
            subset = [r for r in rows if (r["model"], r["effort"]) == (model, effort)]
            require(len(subset) == 23, "Partial cost coverage")
            values = [r["estimated_usd"] for r in subset]
            upper = sum(r.get("long_context_upper_usd", r["estimated_usd"]) for r in subset)
            groups.append(
                {
                    "model": model,
                    "effort": effort,
                    "n": 23,
                    "total_usd": sum(values),
                    "mean_usd": statistics.mean(values),
                    "se_usd": statistics.stdev(values) / math.sqrt(23),
                    "long_context_upper_total_usd": upper,
                    "long_context_upper_mean_usd": upper / 23,
                }
            )
    totals = {
        model: sum(g["total_usd"] for g in groups if g["model"] == model)
        for model in ("astra", "fable")
    }
    upper = sum(g["long_context_upper_total_usd"] for g in groups if g["model"] == "astra")
    out = {
        "pricing_verified": VERIFIED_DATE,
        "sources": SOURCES,
        "source_sha256": digest(source.read_bytes()),
        "astra_rates_per_million": ASTRA_RATES,
        "claude_rates_columns": [
            "input",
            "cache_read",
            "cache_write_5m",
            "cache_write_1h",
            "output",
        ],
        "claude_rates_per_million": CLAUDE_RATES,
        "rows": rows,
        "groups": groups,
        "totals_usd": totals,
        "astra_long_context_upper_total_usd": upper,
        "astra_lower_cost_pct_standard": 100 * (1 - totals["astra"] / totals["fable"]),
        "astra_lower_cost_pct_conservative": 100 * (1 - upper / totals["fable"]),
        "scope": "Solver inference only; not subscription cash, excludes post-hoc judges and local infrastructure",
        "limitations": [
            "Astra input includes cache reads; output includes reasoning. Neither is counted twice.",
            "Astra records zero cache writes. Per-request input sizes are absent in ephemeral CLI logs.",
            "Astra central estimate assumes standard short-context rates, no Batch/Flex/Fast discounts or premiums.",
            "Astra bound applies 2x input/cache and 1.5x output to all usage in runs exceeding 272k cumulative input; actual per-request premiums may be lower.",
            "Claude includes observed 5m/1h caches, Fable, Opus 5, Opus 4.8 and auxiliary Haiku usage.",
            "One internal Opus 5 receipt lacks a model-specific cache TTL; its list cost matches the 5m rate and is retained.",
            "CLI auxiliary accounting differs. Estimates cover exposed receipts, not an independently observed API invoice.",
        ],
    }
    save(OUTPUT / "api-equivalent-costs.json", out)
    with (OUTPUT / "api-equivalent-costs.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(groups[0]))
        writer.writeheader()
        writer.writerows(groups)
    print(
        json.dumps(
            {"totals": totals, "astra_long_context_upper": upper, "groups": groups}, indent=2
        )
    )


if __name__ == "__main__":
    main()
