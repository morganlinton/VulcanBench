"""Hidden test for oss-anyio-max-bytes-guard.

Every byte-stream ``receive(max_bytes)`` implementation in anyio must reject a
non-positive ``max_bytes`` with ``ValueError("max_bytes must be a positive
integer")``, on both the asyncio and trio backends and in every stream wrapper
(buffered, file, stapled, TLS). Passing a valid ``max_bytes`` must continue to
work. Expected behavior captured from the upstream fix (agronholm/anyio PR #1191).

Tests drive the async code through ``anyio.run`` so no pytest plugin is needed.
Run with PYTHONPATH=src so the workspace's src/anyio is the anyio under test.
"""
from __future__ import annotations

import tempfile
from pathlib import Path

import anyio
from anyio import connect_tcp, create_tcp_listener
from anyio.abc import SocketAttribute
from anyio.streams.buffered import BufferedByteReceiveStream
from anyio.streams.file import FileReadStream
from anyio.streams.stapled import StapledByteStream


class _DummyByteReceive:
    async def receive(self, max_bytes: int = 65536) -> bytes:
        return b"payload"

    async def send(self, item: bytes) -> None:
        pass

    async def send_eof(self) -> None:
        pass

    async def aclose(self) -> None:
        pass


async def _assert_guard(receive) -> None:
    for bad in (0, -1):
        try:
            await receive(bad)
        except ValueError as e:
            assert "max_bytes must be a positive integer" in str(e), f"wrong message: {e!r}"
        else:
            raise AssertionError(f"receive({bad}) did not raise ValueError")


def test_buffered_stream_rejects_nonpositive():
    async def run():
        await _assert_guard(BufferedByteReceiveStream(_DummyByteReceive()).receive)

    anyio.run(run)


def test_stapled_stream_rejects_nonpositive():
    async def run():
        await _assert_guard(StapledByteStream(_DummyByteReceive(), _DummyByteReceive()).receive)

    anyio.run(run)


def test_file_stream_rejects_nonpositive():
    async def run():
        with tempfile.NamedTemporaryFile(delete=False) as f:
            f.write(b"contents")
            path = f.name
        try:
            stream = await FileReadStream.from_path(path)
            try:
                await _assert_guard(stream.receive)
            finally:
                await stream.aclose()
        finally:
            Path(path).unlink()

    anyio.run(run)


def test_socket_stream_rejects_nonpositive():
    async def run():
        async def handle(stream):
            async with stream:
                await stream.receive()

        listener = await create_tcp_listener(local_host="127.0.0.1")
        port = listener.extra(SocketAttribute.local_port)
        async with anyio.create_task_group() as tg:
            tg.start_soon(listener.serve, handle)
            await anyio.sleep(0.05)
            stream = await connect_tcp("127.0.0.1", port)
            try:
                await _assert_guard(stream.receive)
                await stream.send(b"hello")
            finally:
                await stream.aclose()
            await anyio.sleep(0.05)
            tg.cancel_scope.cancel()

    anyio.run(run)


def test_valid_receive_still_works():
    async def run():
        stream = BufferedByteReceiveStream(_DummyByteReceive())
        assert await stream.receive(4) == b"payl"

    anyio.run(run)
