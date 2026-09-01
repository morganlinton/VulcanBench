"""Demo app: publishes a flag value into the run's Redis service."""

import json
import socket
from pathlib import Path


def flag_value() -> str:
    return "wrong-value"


def redis_port() -> int:
    manifest = json.loads(Path(".vb_services.json").read_text())
    return int(manifest["services"]["redis"]["6379"])


def _resp(sock: socket.socket, *parts: str) -> str:
    payload = f"*{len(parts)}\r\n" + "".join(f"${len(p)}\r\n{p}\r\n" for p in parts)
    sock.sendall(payload.encode())
    return sock.recv(4096).decode()


def publish_flag() -> str:
    with socket.create_connection(("127.0.0.1", redis_port()), timeout=5) as s:
        _resp(s, "SET", "vb:flag", flag_value())
        reply = _resp(s, "GET", "vb:flag")
    return reply.split("\r\n")[1]
