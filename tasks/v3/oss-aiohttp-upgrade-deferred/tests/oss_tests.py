"""Hidden test for oss-aiohttp-upgrade-deferred.

An HTTP/1.1 request that asks for a protocol upgrade (``Connection: Upgrade``)
and also carries a body must have its body fully readable, and the parser must
switch to the upgraded protocol only after the body has been consumed
(RFC 9110 section 7.8). On the buggy build the body reads back empty and the
bytes following the headers are treated as upgraded-protocol data. Expected
behavior captured from the upstream fix (aio-libs/aiohttp PR #13016).

Tests exercise the pure-Python parser (``aiohttp.http_parser``) directly.
Run with PYTHONPATH=. so the workspace's aiohttp package is under test.
"""
from __future__ import annotations

import asyncio
from unittest import mock

from aiohttp.base_protocol import BaseProtocol
from aiohttp.http_parser import HttpRequestParser


def _make_parser(loop):
    protocol = mock.create_autospec(BaseProtocol, spec_set=True, instance=True)
    return HttpRequestParser(
        protocol, loop, 2**16, max_line_size=8190, max_headers=128, max_field_size=8190
    )


BODY = b"foobarbaz\r\n\r\n"
AFTER = b"GET /after HTTP/1.1\r\nHost: a\r\n\r\n"


def _upgrade_request(framing: bytes, body: bytes) -> bytes:
    return (
        b"GET /ws HTTP/1.1\r\nHost: a\r\n"
        b"Connection: Upgrade\r\nUpgrade: websocket\r\n" + framing + b"\r\n" + body + AFTER
    )


async def _check_upgrade_with_body(text: bytes) -> None:
    parser = _make_parser(asyncio.get_event_loop())
    messages, upgrade, tail = parser.feed_data(text)
    assert len(messages) == 1
    msg, payload = messages[0]
    assert msg.method == "GET"
    assert msg.path == "/ws"
    assert msg.upgrade
    # The body must be fully readable.
    assert await payload.read() == BODY
    # The connection switches protocols only after the body is fully read.
    assert upgrade
    assert tail == AFTER


def test_upgrade_with_content_length_body():
    framing = b"Content-Length: %d\r\n" % len(BODY)
    asyncio.run(_check_upgrade_with_body(_upgrade_request(framing, BODY)))


def test_upgrade_with_chunked_body():
    chunked = b"%x\r\n%s\r\n0\r\n\r\n" % (len(BODY), BODY)
    asyncio.run(_check_upgrade_with_body(_upgrade_request(b"Transfer-Encoding: chunked\r\n", chunked)))


def test_upgrade_empty_body_switches_immediately():
    async def check() -> None:
        parser = _make_parser(asyncio.get_event_loop())
        text = (
            b"GET /ws HTTP/1.1\r\nHost: a\r\n"
            b"Connection: Upgrade\r\nUpgrade: websocket\r\n"
            b"Content-Length: 0\r\n\r\n"
            b"some raw data"
        )
        messages, upgrade, tail = parser.feed_data(text)
        assert messages[0][0].upgrade
        assert upgrade
        assert tail == b"some raw data"

    asyncio.run(check())


def test_plain_request_parsing_stable():
    async def check() -> None:
        parser = _make_parser(asyncio.get_event_loop())
        messages, upgrade, tail = parser.feed_data(
            b"GET /path HTTP/1.1\r\nHost: a\r\nContent-Length: 2\r\n\r\nhi"
        )
        msg, payload = messages[0]
        assert msg.path == "/path"
        assert not upgrade
        assert tail == b""
        assert await payload.read() == b"hi"

    asyncio.run(check())


def test_upgrade_body_split_across_feeds():
    # The protocol switch must stay deferred while the body is still arriving.
    # At the unfixed base the parser marks the upgrade as soon as the headers
    # are complete, so the first feed already reports upgrade=True and the body
    # bytes leak out as the upgraded-protocol tail.
    async def check() -> None:
        parser = _make_parser(asyncio.get_event_loop())
        head = (
            b"GET /ws HTTP/1.1\r\nHost: a\r\n"
            b"Connection: Upgrade\r\nUpgrade: websocket\r\n"
            b"Content-Length: %d\r\n\r\n" % len(BODY)
        )
        first, second = BODY[:4], BODY[4:]

        messages, upgrade, tail = parser.feed_data(head + first)
        assert len(messages) == 1
        msg, payload = messages[0]
        assert msg.upgrade
        # Body is incomplete: no switch yet, and nothing spills into the tail.
        assert not upgrade
        assert tail == b""

        _, upgrade2, tail2 = parser.feed_data(second + AFTER)
        assert upgrade2
        assert tail2 == AFTER
        assert await payload.read() == BODY

    asyncio.run(check())
