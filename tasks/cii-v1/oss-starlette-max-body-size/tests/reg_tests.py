"""Hidden pass-to-pass guards: everything the limiter must not disturb.

These are the tail mechanism for this task — the functional score is zero if
ANY of these regress, so a limiter that swallows streaming bodies, breaks form
parsing, or reorders the middleware stack scores 0.0 rather than a partial.
"""

from __future__ import annotations

from starlette.applications import Starlette
from starlette.background import BackgroundTask
from starlette.responses import PlainTextResponse, StreamingResponse
from starlette.routing import Mount, Route
from starlette.testclient import TestClient


async def echo_len(request):  # type: ignore[no-untyped-def]
    body = await request.body()
    return PlainTextResponse(str(len(body)))


def test_plain_request_response_unchanged():
    async def hello(request):  # type: ignore[no-untyped-def]
        return PlainTextResponse("hello")

    client = TestClient(Starlette(routes=[Route("/", hello)]))
    res = client.get("/")
    assert res.status_code == 200
    assert res.text == "hello"


def test_large_body_without_limit_still_accepted():
    client = TestClient(Starlette(routes=[Route("/", echo_len, methods=["POST"])]))
    res = client.post("/", content=b"x" * 250_000)
    assert res.status_code == 200
    assert res.text == "250000"


def test_urlencoded_form_parsing():
    async def form_endpoint(request):  # type: ignore[no-untyped-def]
        form = await request.form()
        return PlainTextResponse(form["field"])

    client = TestClient(Starlette(routes=[Route("/", form_endpoint, methods=["POST"])]))
    res = client.post("/", data={"field": "value"})
    assert res.status_code == 200
    assert res.text == "value"


def test_multipart_form_parsing_and_file_contents():
    async def upload(request):  # type: ignore[no-untyped-def]
        form = await request.form()
        upload_file = form["f"]
        content = await upload_file.read()  # type: ignore[union-attr]
        return PlainTextResponse(content.decode())

    client = TestClient(Starlette(routes=[Route("/", upload, methods=["POST"])]))
    res = client.post("/", files=[("f", ("a.txt", b"file-body", "text/plain"))])
    assert res.status_code == 200
    assert res.text == "file-body"


def test_streaming_request_body_read_in_chunks():
    async def stream_endpoint(request):  # type: ignore[no-untyped-def]
        total = 0
        async for chunk in request.stream():
            total += len(chunk)
        return PlainTextResponse(str(total))

    client = TestClient(Starlette(routes=[Route("/", stream_endpoint, methods=["POST"])]))

    def chunks():
        for _ in range(4):
            yield b"z" * 25

    res = client.post("/", content=chunks())
    assert res.status_code == 200
    assert res.text == "100"


def test_streaming_response_still_streams():
    async def streamer(request):  # type: ignore[no-untyped-def]
        async def gen():
            for i in range(3):
                yield f"{i}".encode()

        return StreamingResponse(gen(), media_type="text/plain")

    client = TestClient(Starlette(routes=[Route("/", streamer)]))
    assert client.get("/").text == "012"


def test_exception_handlers_and_background_tasks():
    state = {"ran": False}

    async def boom(request):  # type: ignore[no-untyped-def]
        raise ValueError("kaboom")

    async def with_task(request):  # type: ignore[no-untyped-def]
        def mark() -> None:
            state["ran"] = True

        return PlainTextResponse("queued", background=BackgroundTask(mark))

    async def handler(request, exc):  # type: ignore[no-untyped-def]
        return PlainTextResponse("handled", status_code=500)

    app = Starlette(
        routes=[Route("/boom", boom), Route("/task", with_task)],
        exception_handlers={ValueError: handler},
    )
    client = TestClient(app)
    assert client.get("/boom").text == "handled"
    assert client.get("/task").text == "queued"
    assert state["ran"] is True


def test_mounted_app_routing_unchanged():
    inner = Starlette(routes=[Route("/echo", echo_len, methods=["POST"])])
    client = TestClient(Starlette(routes=[Mount("/sub", app=inner)]))
    assert client.post("/sub/echo", content=b"abc").text == "3"
    assert client.post("/nope", content=b"abc").status_code == 404


# Uses no new API: passes at base too, kept as a guard.
def test_no_limit_by_default():
    app = Starlette(routes=[Route("/", echo_len, methods=["POST"])])
    client = TestClient(app)
    res = client.post("/", content=b"x" * 100_000)
    assert res.status_code == 200
    assert res.text == "100000"
