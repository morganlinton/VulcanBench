"""Hidden fail-to-pass tests: offload large GZip compression to a worker thread.

Graded through the public middleware surface: the new setting must exist and
be honored, and compression must stay correct at every size.
"""

from __future__ import annotations

from starlette.applications import Starlette
from starlette.middleware import Middleware
from starlette.middleware.gzip import GZipMiddleware
from starlette.responses import PlainTextResponse, StreamingResponse
from starlette.routing import Route
from starlette.testclient import TestClient


def _app(body: bytes, **gzip_kwargs):  # type: ignore[no-untyped-def]
    async def endpoint(request):  # type: ignore[no-untyped-def]
        return PlainTextResponse(body)

    return Starlette(
        routes=[Route("/", endpoint)],
        middleware=[Middleware(GZipMiddleware, minimum_size=10, **gzip_kwargs)],
    )


def test_thread_minimum_size_setting_accepted():
    client = TestClient(_app(b"a" * 5000, thread_minimum_size=1024))
    res = client.get("/", headers={"accept-encoding": "gzip"})
    assert res.status_code == 200
    assert res.headers["content-encoding"] == "gzip"
    assert res.text == "a" * 5000


def test_large_body_compresses_correctly_when_offloaded():
    # Body well above any sane offload threshold: the round trip must be exact.
    body = bytes(bytearray((i * 7) % 251 for i in range(300_000)))

    async def endpoint(request):  # type: ignore[no-untyped-def]
        from starlette.responses import Response

        return Response(body, media_type="application/octet-stream")

    app = Starlette(
        routes=[Route("/", endpoint)],
        middleware=[Middleware(GZipMiddleware, minimum_size=10, thread_minimum_size=1024)],
    )
    client = TestClient(app)
    res = client.get("/", headers={"accept-encoding": "gzip"})
    assert res.headers["content-encoding"] == "gzip"
    assert res.content == body


def test_small_body_still_compressed_inline():
    client = TestClient(_app(b"b" * 2000, thread_minimum_size=1_000_000))
    res = client.get("/", headers={"accept-encoding": "gzip"})
    assert res.headers["content-encoding"] == "gzip"
    assert res.text == "b" * 2000


def test_streaming_response_offload_round_trips():
    chunk = b"c" * 200_000

    async def endpoint(request):  # type: ignore[no-untyped-def]
        async def gen():
            for _ in range(3):
                yield chunk

        return StreamingResponse(gen(), media_type="text/plain")

    app = Starlette(
        routes=[Route("/", endpoint)],
        middleware=[Middleware(GZipMiddleware, minimum_size=10, thread_minimum_size=1024)],
    )
    client = TestClient(app)
    res = client.get("/", headers={"accept-encoding": "gzip"})
    assert res.headers["content-encoding"] == "gzip"
    assert res.content == chunk * 3
