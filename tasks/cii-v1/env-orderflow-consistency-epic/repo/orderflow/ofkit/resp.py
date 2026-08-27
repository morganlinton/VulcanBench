"""A small RESP2 client for the shared Redis, sufficient for our cache use.

Not a general client: single connection, blocking, no pipelining, no pubsub.
Commands used in production code: GET SET SETNX DEL INCRBY EXPIRE EXISTS.
"""

from __future__ import annotations

import socket

from orderflow.ofkit import topology


class RespError(RuntimeError):
    pass


class Redis:
    def __init__(self, timeout_s: float = 5.0) -> None:
        self._sock = socket.create_connection(
            ("127.0.0.1", topology.infra_port("redis", "6379")), timeout=timeout_s
        )
        self._buf = b""

    # -- wire format --------------------------------------------------------

    def _send(self, *parts: str | bytes) -> None:
        out = [f"*{len(parts)}\r\n".encode()]
        for part in parts:
            data = part.encode() if isinstance(part, str) else part
            out.append(f"${len(data)}\r\n".encode() + data + b"\r\n")
        self._sock.sendall(b"".join(out))

    def _read_line(self) -> bytes:
        while b"\r\n" not in self._buf:
            chunk = self._sock.recv(65536)
            if not chunk:
                raise RespError("connection closed by redis")
            self._buf += chunk
        line, self._buf = self._buf.split(b"\r\n", 1)
        return line

    def _read_exact(self, n: int) -> bytes:
        while len(self._buf) < n + 2:
            chunk = self._sock.recv(65536)
            if not chunk:
                raise RespError("connection closed by redis")
            self._buf += chunk
        data, self._buf = self._buf[:n], self._buf[n + 2 :]
        return data

    def _reply(self):
        line = self._read_line()
        kind, rest = line[:1], line[1:]
        if kind == b"+":
            return rest.decode()
        if kind == b":":
            return int(rest)
        if kind == b"$":
            length = int(rest)
            return None if length == -1 else self._read_exact(length).decode()
        if kind == b"-":
            raise RespError(rest.decode())
        if kind == b"*":
            return [self._reply() for _ in range(int(rest))]
        raise RespError(f"unexpected reply frame {line!r}")

    def command(self, *parts: str | bytes):
        self._send(*parts)
        return self._reply()

    # -- the commands we actually use ---------------------------------------

    def get(self, key: str) -> str | None:
        return self.command("GET", key)

    def set(self, key: str, value: str, ttl_s: int | None = None) -> None:
        if ttl_s is None:
            self.command("SET", key, value)
        else:
            self.command("SET", key, value, "EX", str(ttl_s))

    def setnx(self, key: str, value: str) -> bool:
        return int(self.command("SETNX", key, value)) == 1

    def delete(self, *keys: str) -> int:
        return int(self.command("DEL", *keys))

    def incrby(self, key: str, delta: int) -> int:
        return int(self.command("INCRBY", key, str(delta)))

    def exists(self, key: str) -> bool:
        return int(self.command("EXISTS", key)) == 1

    def flush_all(self) -> None:
        self.command("FLUSHALL")

    def close(self) -> None:
        self._sock.close()

    def __enter__(self) -> Redis:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()
