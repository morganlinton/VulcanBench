"""Hidden fail-to-pass tests: configurable request-body size limits.

Each test builds its app inside the test, so the base-time TypeError (no such
keyword / no such module) fails every test individually. Behavior is graded
through the public ASGI surface with TestClient; no error-text assertions.
"""

from __future__ import annotations

import pytest
from starlette.applications import Starlette
from starlette.responses import PlainTextResponse
from starlette.routing import Mount, Route
from starlette.testclient import TestClient


async def echo_len(request):  # type: ignore[no-untyped-def]
    body = await request.body()
    return PlainTextResponse(str(len(body)))


def test_app_level_limit_rejects_oversized_body():
    app = Starlette(routes=[Route("/", echo_len, methods=["POST"])], max_body_size=100)
    client = TestClient(app)
    assert client.post("/", content=b"x" * 50).status_code == 200
    assert client.post("/", content=b"x" * 500).status_code == 413


def test_route_level_limit():
    app = Starlette(
        routes=[
            Route("/small", echo_len, methods=["POST"], max_body_size=10),
            Route("/big", echo_len, methods=["POST"]),
        ]
    )
    client = TestClient(app)
    assert client.post("/small", content=b"x" * 100).status_code == 413
    # A route without its own limit is unaffected by another route's limit.
    assert client.post("/big", content=b"x" * 100).status_code == 200


def test_mount_level_limit():
    inner = Starlette(routes=[Route("/echo", echo_len, methods=["POST"])])
    app = Starlette(routes=[Mount("/sub", app=inner, max_body_size=10)])
    client = TestClient(app)
    assert client.post("/sub/echo", content=b"x" * 100).status_code == 413
    assert client.post("/sub/echo", content=b"x" * 5).status_code == 200


def test_standalone_middleware_on_raw_asgi_app():
    from starlette.middleware.body_limit import RequestBodyLimitMiddleware

    async def raw_app(scope, receive, send):  # type: ignore[no-untyped-def]
        assert scope["type"] == "http"
        while True:
            message = await receive()
            if not message.get("more_body", False):
                break
        await send({"type": "http.response.start", "status": 200, "headers": []})
        await send({"type": "http.response.body", "body": b"ok"})

    client = TestClient(RequestBodyLimitMiddleware(raw_app, max_body_size=20))
    assert client.post("/", content=b"x" * 5).status_code == 200
    assert client.post("/", content=b"x" * 200).status_code == 413


def test_limit_counts_streamed_bytes_not_content_length():
    # A chunked/streamed body carries no Content-Length, so a limiter that only
    # inspects the header lets it through. The limit must count actual bytes.
    app = Starlette(routes=[Route("/", echo_len, methods=["POST"])], max_body_size=100)
    client = TestClient(app)

    def chunks():
        for _ in range(20):
            yield b"x" * 50

    assert client.post("/", content=chunks()).status_code == 413


def test_multipart_upload_counts_toward_limit():
    async def form_endpoint(request):  # type: ignore[no-untyped-def]
        form = await request.form()
        return PlainTextResponse(str(len(form)))

    app = Starlette(
        routes=[Route("/", form_endpoint, methods=["POST"])],
        max_body_size=200,
    )
    client = TestClient(app)
    big = ("f", ("big.bin", b"y" * 5000, "application/octet-stream"))
    assert client.post("/", files=[big]).status_code == 413
