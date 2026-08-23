"""Hidden pass-to-pass guards: everything the offload must not disturb.

The first guard is the point of this task. `GZipResponder` is public and is
instantiated directly by third-party middleware; it must stay constructible
with the arguments it has always taken. A failing guard zeroes the functional
score, so an implementation that makes the new setting a required parameter of
the responder scores 0.0 — which is precisely the mistake the upstream change
shipped with and had to repair the same day.
"""

from __future__ import annotations

from starlette.applications import Starlette
from starlette.middleware import Middleware
from starlette.middleware.gzip import GZipMiddleware, GZipResponder
from starlette.responses import PlainTextResponse
from starlette.routing import Route
from starlette.testclient import TestClient


async def _endpoint(request):  # type: ignore[no-untyped-def]
    return PlainTextResponse("payload " * 500)


def _plain_app():  # type: ignore[no-untyped-def]
    return Starlette(routes=[Route("/", _endpoint)])


def test_responder_constructible_with_historical_arguments():
    # Third-party code does exactly this; the signature must stay compatible.
    responder = GZipResponder(_plain_app(), 500)
    client = TestClient(responder)
    res = client.get("/", headers={"accept-encoding": "gzip"})
    assert res.status_code == 200
    assert res.text == "payload " * 500


def test_responder_constructible_with_compresslevel():
    responder = GZipResponder(_plain_app(), 500, compresslevel=6)
    client = TestClient(responder)
    res = client.get("/", headers={"accept-encoding": "gzip"})
    assert res.status_code == 200
    assert res.text == "payload " * 500


def test_middleware_without_new_setting():
    app = Starlette(
        routes=[Route("/", _endpoint)],
        middleware=[Middleware(GZipMiddleware, minimum_size=500)],
    )
    res = TestClient(app).get("/", headers={"accept-encoding": "gzip"})
    assert res.headers["content-encoding"] == "gzip"
    assert res.text == "payload " * 500


def test_minimum_size_still_skips_small_bodies():
    async def small(request):  # type: ignore[no-untyped-def]
        return PlainTextResponse("tiny")

    app = Starlette(
        routes=[Route("/", small)],
        middleware=[Middleware(GZipMiddleware, minimum_size=500)],
    )
    res = TestClient(app).get("/", headers={"accept-encoding": "gzip"})
    assert res.status_code == 200
    assert "content-encoding" not in res.headers
    assert res.text == "tiny"


def test_no_compression_without_accept_encoding():
    app = Starlette(
        routes=[Route("/", _endpoint)],
        middleware=[Middleware(GZipMiddleware, minimum_size=500)],
    )
    res = TestClient(app).get("/", headers={"accept-encoding": "identity"})
    assert "content-encoding" not in res.headers
    assert res.text == "payload " * 500


def test_vary_header_set():
    app = Starlette(
        routes=[Route("/", _endpoint)],
        middleware=[Middleware(GZipMiddleware, minimum_size=500)],
    )
    res = TestClient(app).get("/", headers={"accept-encoding": "gzip"})
    assert "accept-encoding" in res.headers.get("vary", "").lower()
