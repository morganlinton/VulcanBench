#!/usr/bin/env python3
"""Control run: a frontier LLM answers Verdict v1 items from the same inputs Jev saw.

The question this answers is whether an item family is answerable at all
from its state (bug report plus fix). If a strong reasoning model given the
identical text also sits at the majority floor, the family cannot separate
"this model cannot judge code" from "nobody could judge this from these
inputs", and its results should not be read as a verdict on the model.

The model runs through ``codex exec`` on a ChatGPT subscription, in an empty
read-only directory, with no repository, binary or test access: it sees
exactly the ``state`` and question text sent to Jev and returns a
probability. Resumable; development split by default, so nothing here
touches the published test split.

    python scripts/verdict-v1/run_llm_control.py --family patch-verdict --workers 3
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from harness.settings import check_effort_allowed  # noqa: E402
from harness.verdict.items import load_items  # noqa: E402

PROMPT = """You are reviewing a code change. Use only the text below: do not run \
commands, open files or use any tool.

{state}

## Question

Statement: {instructions}

How likely is it that this statement is true? Reply with only a JSON object, \
{{"p_true": <number between 0 and 1>}}, and nothing else."""


def ask(item: dict, model: str, effort: str, timeout_s: float) -> dict:
    prompt = PROMPT.format(
        state=item["state"].rstrip(), instructions=item["question"]["instructions"]
    )
    with tempfile.TemporaryDirectory(prefix="verdict-control-") as empty:
        started = time.monotonic()
        result = subprocess.run(
            [
                "codex",
                "exec",
                "--json",
                "--ephemeral",
                "--ignore-user-config",
                "--skip-git-repo-check",
                "--sandbox",
                "read-only",
                "--cd",
                empty,
                "--model",
                model,
                "--config",
                f'model_reasoning_effort="{effort}"',
                "-",
            ],
            input=prompt,
            capture_output=True,
            text=True,
            timeout=timeout_s,
            check=False,
        )
        latency_ms = (time.monotonic() - started) * 1000
    reply, usage, tool_calls = None, None, 0
    for line in result.stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        kind = event.get("item", {}).get("type")
        if event.get("type") == "item.completed" and kind == "agent_message":
            reply = event["item"]["text"]
        elif event.get("type") == "item.completed" and kind not in (
            None,
            "agent_message",
            "reasoning",
        ):
            tool_calls += 1
        elif event.get("type") == "turn.completed":
            usage = event.get("usage")
    if reply is None:
        raise RuntimeError(f"no answer (exit {result.returncode}): {result.stderr.strip()[-300:]}")
    start, end = reply.find("{"), reply.rfind("}")
    p_true = float(json.loads(reply[start : end + 1])["p_true"])
    if not 0.0 <= p_true <= 1.0:
        raise ValueError(f"probability out of range: {p_true}")
    return {
        "item_id": item["item_id"],
        "probs": {"true": p_true, "false": 1.0 - p_true},
        "latency_ms": latency_ms,
        "model": f"codex:{model}",
        "effort": effort,
        "usage": usage,
        "tool_calls": tool_calls,
        "queried_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=REPO / "verdict-v1-items" / "items.jsonl")
    parser.add_argument("--family", default="patch-verdict")
    parser.add_argument("--split", default="dev")
    parser.add_argument("--model", default="gpt-6-astra")
    parser.add_argument("--effort", default="high")
    parser.add_argument("--workers", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=600.0)
    parser.add_argument("-o", "--out", type=Path, default=None)
    args = parser.parse_args()
    check_effort_allowed(args.effort)
    out = (
        args.out
        or REPO
        / "verdict-v1-items"
        / f"control-{args.model}-{args.effort}-{args.family}-{args.split}.jsonl"
    )

    items = [
        i
        for i in load_items(args.items)
        if i["family"] == args.family
        and i["split"] == args.split
        and i["question"]["type"] == "noul"
    ]
    done = (
        {json.loads(line)["item_id"] for line in out.read_text().splitlines() if line.strip()}
        if out.exists()
        else set()
    )
    todo = [i for i in items if i["item_id"] not in done]
    print(
        f"{len(done)} done, {len(todo)} to go: {args.family}/{args.split} via codex:{args.model} at {args.effort}",
        flush=True,
    )

    lock = threading.Lock()
    failures = 0
    with out.open("a") as handle, ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = {
            pool.submit(ask, item, args.model, args.effort, args.timeout): item for item in todo
        }
        for n, future in enumerate(as_completed(futures), 1):
            item = futures[future]
            try:
                record = future.result()
            except Exception as error:
                failures += 1
                print(
                    f"[{n}/{len(todo)}] {item['item_id']} failed: {type(error).__name__}: {error}",
                    flush=True,
                )
                continue
            with lock:
                handle.write(json.dumps(record) + "\n")
                handle.flush()
            if n % 10 == 0 or n == len(todo):
                print(f"[{n}/{len(todo)}] written", flush=True)
    print(f"finished: {len(todo) - failures} written, {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
