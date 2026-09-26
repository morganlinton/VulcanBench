#!/usr/bin/env python3
"""Reference row: a frontier LLM answers Verdict v2 items from the same inputs Jev sees.

It shows each family is answerable (the admission gate) and is published as
a labelled reference row, not a leaderboard entry. The model runs through
``codex exec`` on the ChatGPT subscription, in an empty read-only directory
with no tools: it sees exactly the ``state``, the question and the options
sent to Jev, and returns a probability for every option. Resumable.

    python scripts/verdict-v2/run_reference.py --items verdict-v2-items/pilot.jsonl --workers 3
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
from typing import Any

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

from harness.settings import check_effort_allowed  # noqa: E402
from harness.verdict.v2.items import labels_for, load_items  # noqa: E402

ITEMS = REPO / "verdict-v2-items"
PROMPT = """Answer the question using only the text below. Do not run commands, \
open files or use any tool.

{state}

## Question

{question}

Reply with only a JSON object giving your probability for every label, \
{{"probs": {{{example}}}}}, with probabilities between 0 and 1 that sum to 1, \
and nothing else."""


def render_question(question: dict[str, Any]) -> str:
    kind = question["type"]
    if kind == "noul":
        return (
            f"Statement: {question['instructions']}\n\n"
            'Labels: "true" if the statement is true, "false" if not.'
        )
    if kind == "choice":
        descriptions = question.get("descriptions") or {}
        lines = [
            f'- "{option}": {descriptions[option]}' if descriptions.get(option) else f'- "{option}"'
            for option in question["options"]
        ]
        return f"{question['instructions']}\n\nLabels:\n" + "\n".join(lines)
    levels = "\n".join(f'- "{level}"' for level in question["levels"])
    return f"{question['instructions']}\n\nLabels, ordered low to high:\n{levels}"


def build_prompt(item: dict[str, Any]) -> str:
    labels = labels_for(item["question"])
    example = ", ".join(f'"{label}": <p>' for label in labels[:3])
    if len(labels) > 3:
        example += ", ..."
    return PROMPT.format(
        state=item["state"].rstrip(), question=render_question(item["question"]), example=example
    )


def parse_reply(item: dict[str, Any], reply: str) -> dict[str, float]:
    start, end = reply.find("{"), reply.rfind("}")
    probs = json.loads(reply[start : end + 1])["probs"]
    labels = set(labels_for(item["question"]))
    unknown = set(probs) - labels
    if unknown:
        raise ValueError(f"labels not in the question: {sorted(unknown)[:5]}")
    values = {str(k): float(v) for k, v in probs.items()}
    if any(not 0.0 <= v <= 1.0 for v in values.values()) or sum(values.values()) <= 0:
        raise ValueError("probabilities out of range")
    return values


def ask(item: dict[str, Any], model: str, effort: str, timeout_s: float) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="verdict-v2-ref-") as empty:
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
            input=build_prompt(item),
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
        elif event.get("type") == "item.completed" and kind not in (None, "reasoning"):
            tool_calls += 1
        elif event.get("type") == "turn.completed":
            usage = event.get("usage")
    if reply is None:
        raise RuntimeError(f"no answer (exit {result.returncode}): {result.stderr.strip()[-300:]}")
    return {
        "item_id": item["item_id"],
        "probs": parse_reply(item, reply),
        "latency_ms": latency_ms,
        "model": f"codex:{model}",
        "effort": effort,
        "usage": usage,
        # Any tool call breaks the same-inputs rule; the scorer can drop these.
        "tool_calls": tool_calls,
        "queried_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--items", type=Path, default=ITEMS / "pilot.jsonl")
    parser.add_argument("--family", action="append", help="only these families (repeatable)")
    parser.add_argument("--split", default=None, help="dev or test; default both")
    parser.add_argument("--model", default="gpt-6-astra")
    parser.add_argument("--effort", default="high")
    parser.add_argument("--workers", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=600.0)
    parser.add_argument("-o", "--out", type=Path, default=None)
    args = parser.parse_args()
    check_effort_allowed(args.effort)
    out = args.out or ITEMS / f"reference-{args.model}-{args.effort}-{args.items.stem}.jsonl"

    items = [
        i
        for i in load_items(args.items)
        if (not args.family or i["family"] in args.family) and args.split in (None, i["split"])
    ]
    done = (
        {json.loads(line)["item_id"] for line in out.read_text().splitlines() if line.strip()}
        if out.exists()
        else set()
    )
    todo = [i for i in items if i["item_id"] not in done]
    print(
        f"{len(done)} done, {len(todo)} to go via codex:{args.model} at {args.effort}", flush=True
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
                    f"[{n}/{len(todo)}] {item['item_id']} failed: {type(error).__name__}: {error}"
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
