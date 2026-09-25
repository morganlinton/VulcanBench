"""Adapter for TypeSafe AI's System One API (Jev).

Request and response shapes follow docs.typesafe.ai as read on 2026-09-19:
``POST /v1/systemone`` with ``state``, ``model`` and ``questions``; every
question carries ``type``, ``instructions`` and ``criteria``; answers come
back under ``answers`` keyed by question id. Not yet exercised against the
live service: run the preflight before publishing any number from it.
"""

from __future__ import annotations

import json
import os
import time
import urllib.error
import urllib.request
from typing import Any

from harness.verdict.scoring import noul_probs

API_KEY_ENV = "TYPESAFE_API_KEY"
BASE_URL = os.environ.get("TYPESAFE_BASE_URL", "https://api.typesafe.ai")
ENDPOINT_PATH = "/v1/systemone"
# Pinned: jev-latest is an alias that moves, and calibration is per version.
DEFAULT_MODEL = "jev-1.13.0"
USD_PER_INPUT_MTOK = 0.042  # list price at launch; output tokens are free
QUESTION_KEY = "q"
RETRY_STATUSES = frozenset({408, 429, 529}) | frozenset(range(500, 600))
MAX_ATTEMPTS = 3


class TypeSafeNotConfigured(RuntimeError):
    pass


def build_request(item: dict[str, Any], model: str = DEFAULT_MODEL) -> dict[str, Any]:
    spec = item["question"]
    question: dict[str, Any] = {"type": spec["type"], "instructions": spec["instructions"]}
    if spec["type"] == "choice":
        descriptions = spec.get("descriptions") or {}
        question["criteria"] = {option: descriptions.get(option) for option in spec["options"]}
    return {"state": item["state"], "model": model, "questions": {QUESTION_KEY: question}}


def parse_answer(item: dict[str, Any], body: dict[str, Any]) -> dict[str, float]:
    answer = body["answers"][QUESTION_KEY]
    if item["question"]["type"] == "noul":
        return noul_probs(float(answer["noul"]))
    return {str(k): float(v) for k, v in answer["probabilities"].items()}


def predict(
    item: dict[str, Any], model: str = DEFAULT_MODEL, timeout_s: float = 30.0
) -> dict[str, Any]:
    key = os.environ.get(API_KEY_ENV)
    if not key:
        raise TypeSafeNotConfigured(f"set {API_KEY_ENV} before calling the live service")
    request = urllib.request.Request(
        BASE_URL.rstrip("/") + ENDPOINT_PATH,
        data=json.dumps(build_request(item, model)).encode(),
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
    )
    for attempt in range(1, MAX_ATTEMPTS + 1):
        started = time.monotonic()
        try:
            with urllib.request.urlopen(request, timeout=timeout_s) as response:
                body = json.loads(response.read())
                request_id = response.headers.get("x-typesafe-request-id")
            break
        except urllib.error.HTTPError as error:
            if error.code not in RETRY_STATUSES or attempt == MAX_ATTEMPTS:
                raise
            time.sleep(float(error.headers.get("retry-after") or 0.5 * 2**attempt))
    # Latency is the successful attempt only, wall clock from this machine, so it
    # includes the network round trip to the service (US West Coast at launch).
    latency_ms = (time.monotonic() - started) * 1000
    input_tokens = (body.get("usage") or {}).get("input_tokens")
    return {
        "item_id": item["item_id"],
        "probs": parse_answer(item, body),
        "latency_ms": latency_ms,
        "input_tokens": input_tokens,
        "cost_usd": input_tokens * USD_PER_INPUT_MTOK / 1e6 if input_tokens is not None else None,
        "model": body.get("model", model),
        "request_id": request_id,
        "attempts": attempt,
    }
