"""Inter-service JSON HTTP client with a shared retry policy.

Transient failures (connection errors, 503) are retried with backoff; any
other non-2xx becomes :class:`UpstreamError` carrying the upstream's error
body. Timeouts are generous by design: this platform prefers backpressure
(503) to deadline races.
"""

from __future__ import annotations

import time
from typing import Any

import requests

from orderflow.ofkit.log import get_logger

_log = get_logger("client")

RETRIES = 3
BACKOFF_S = 0.05
TIMEOUT_S = 30.0


class UpstreamError(RuntimeError):
    def __init__(self, status: int, body: dict):
        super().__init__(f"upstream {status}: {body.get('error')}")
        self.status = status
        self.body = body


def call(method: str, url: str, json_body: dict[str, Any] | None = None) -> dict:
    last_exc: Exception | None = None
    for attempt in range(1, RETRIES + 1):
        try:
            response = requests.request(method, url, json=json_body, timeout=TIMEOUT_S)
        except requests.RequestException as exc:
            last_exc = exc
            _log.warning("attempt %d %s %s failed: %r", attempt, method, url, exc)
            time.sleep(BACKOFF_S * attempt)
            continue
        if response.status_code == 503:
            last_exc = UpstreamError(503, _body(response))
            time.sleep(BACKOFF_S * attempt)
            continue
        if response.status_code >= 400:
            raise UpstreamError(response.status_code, _body(response))
        return _body(response)
    raise UpstreamError(503, {"error": "unavailable", "message": str(last_exc)})


def _body(response: requests.Response) -> dict:
    try:
        parsed = response.json()
    except ValueError:
        return {"error": "bad_upstream_body", "raw": response.text[:200]}
    return parsed if isinstance(parsed, dict) else {"value": parsed}


def get(url: str) -> dict:
    return call("GET", url)


def post(url: str, json_body: dict[str, Any] | None = None) -> dict:
    return call("POST", url, json_body)
