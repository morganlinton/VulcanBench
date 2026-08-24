"""Hidden guards: the service stack itself is reachable and manifest sane."""

import json
import socket
from pathlib import Path

import app


def test_manifest_present_with_project():
    manifest = json.loads(Path(".vb_services.json").read_text())
    assert manifest["project"].startswith("vb-")
    assert "redis" in manifest["services"]


def test_redis_answers_ping():
    with socket.create_connection(("127.0.0.1", app.redis_port()), timeout=5) as s:
        s.sendall(b"*1\r\n$4\r\nPING\r\n")
        assert s.recv(64).startswith(b"+PONG")
