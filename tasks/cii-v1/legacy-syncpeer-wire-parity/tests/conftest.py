"""Fixtures for the syncpeer parity tests.

Every session's expected replies were captured from the production legacy
peer at task build time. Each fixture session boots a FRESH workspace
server process (state is process-scoped) and drives the scripted
connections over real TCP. The legacy binary is quarantined for the whole
test session: production does not ship it, so a replacement that shells
out to it must fail here.
"""

from __future__ import annotations

import json
import shutil
import socket
import subprocess
import sys
from pathlib import Path

import pytest

FIXTURES = json.loads((Path(__file__).resolve().parent / "fixtures.json").read_text())
WORKSPACE = Path.cwd()


@pytest.fixture(scope="session", autouse=True)
def quarantine_legacy():
    legacy = WORKSPACE / "legacy"
    stash = WORKSPACE / ".legacy_quarantined"
    moved = False
    if legacy.exists():
        if stash.exists():
            shutil.rmtree(stash)
        legacy.rename(stash)
        moved = True
    try:
        yield
    finally:
        if moved and stash.exists() and not legacy.exists():
            stash.rename(legacy)


def drive_workspace_server(connections: list[list[str]]) -> list[list[str]]:
    proc = subprocess.Popen(
        [sys.executable, "syncpeer.py"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd=WORKSPACE,
    )
    try:
        banner = (proc.stdout.readline() or "").strip()
        assert banner.startswith("LISTENING "), (
            f"server did not announce a port: {banner!r} {proc.stderr.read()[:300] if proc.poll() is not None else ''}"
        )
        port = int(banner.split()[1])
        transcripts = []
        for lines in connections:
            replies: list[str] = []
            with socket.create_connection(("127.0.0.1", port), timeout=10) as conn:
                rfile = conn.makefile("r")
                closed = False
                for line in lines:
                    if closed:
                        break
                    conn.sendall((line + "\n").encode())
                    reply = rfile.readline()
                    if not reply:
                        closed = True
                        break
                    reply = reply.rstrip("\n")
                    replies.append(reply)
                    if reply.startswith("KEY "):
                        while not reply.startswith("END "):
                            reply = rfile.readline().rstrip("\n")
                            replies.append(reply)
                    if reply.startswith("GOODBYE") or reply == "ERR HANDSHAKE":
                        closed = True
                rfile.close()
            transcripts.append(replies)
        return transcripts
    finally:
        proc.kill()
        proc.wait()


def assert_family(family: str) -> None:
    for case in FIXTURES[family]:
        got = drive_workspace_server(case["connections"])
        assert got == case["expected"], (
            f"{family}: wire parity failure\n"
            f"connections: {case['connections']}\n"
            f"expected:    {case['expected']}\n"
            f"got:         {got}"
        )
