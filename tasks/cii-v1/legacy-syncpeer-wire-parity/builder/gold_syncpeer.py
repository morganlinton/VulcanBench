"""SyncPeer replication peer, Python implementation (engine-faithful).

Replaces the retired legacy binary. Every deviation from docs/SPEC.md is
marked ``engine:`` - the engine's behavior is the contract.
"""

from __future__ import annotations

import argparse
import re
import socket
import sys

MAX_KEYS = 4096
VAL_KEEP = 48  # engine: values are silently truncated to 48 characters
_INT_RE = re.compile(r"^[+-]?[0-9]+$")


class Peer:
    def __init__(self):
        # engine: append-only slot list; deleted slots stay in place, and
        # KEYS walks the slots in reverse (newest first).
        self.slots: list[dict] = []
        # engine: session ids are per NODE NAME for the process lifetime; a
        # returning node gets its original id back.
        self.node_sessions: dict[str, int] = {}
        self.next_session = 0

    def _find(self, key: str) -> dict | None:
        for slot in self.slots:
            if slot["used"] and slot["key"] == key:
                return slot
        return None

    # -- one connection -----------------------------------------------------

    def serve_connection(self, rfile, wfile) -> None:
        def send(line: str) -> None:
            wfile.write((line + "\n").encode())
            wfile.flush()

        raw = rfile.readline()
        if not raw:
            return
        lines_seen = 1  # engine: the HELLO line itself is counted
        parts = raw.decode(errors="replace").strip().split()
        version = _parse_int(parts[1]) if len(parts) >= 2 else None
        if (
            len(parts) < 3
            or parts[0] != "HELLO"
            or version is None
            or not 1 <= version <= 9
            or not _alnum(parts[2], 1, 8)
        ):
            send("ERR HANDSHAKE")
            return
        negotiated = min(version, 3)
        if negotiated == 2:
            negotiated = 1  # engine: v2 was recalled; negotiates down to 1
        node = parts[2]
        if node in self.node_sessions:
            session = self.node_sessions[node]
        else:
            self.next_session += 1
            session = self.next_session
            self.node_sessions[node] = session
        send(f"WELCOME {negotiated} S{session}")

        for raw in iter(rfile.readline, b""):
            line = raw.decode(errors="replace").rstrip("\r\n")
            if not line:
                continue
            lines_seen += 1
            parts = line.split()
            cmd = parts[0]
            if cmd == "BYE":
                send(f"GOODBYE {lines_seen}")
                return
            if (
                cmd == "PUT"
                and len(parts) >= 3
                and _alnum(parts[1], 1, 16)
                and 1 <= len(parts[2]) <= 64
            ):
                kept = parts[2][:VAL_KEEP]
                existing = self._find(parts[1])
                if existing is not None:
                    # engine: overwriting with a different value echoes the
                    # old one; a no-op overwrite answers plain OK.
                    if existing["value"] != kept:
                        send(f"OK {existing['value']}")
                        existing["value"] = kept
                    else:
                        send("OK")
                elif len(self.slots) < MAX_KEYS:
                    self.slots.append({"key": parts[1], "value": kept, "used": True})
                    send("OK")
                else:
                    send("ERR FULL")
            elif cmd == "GET" and len(parts) >= 2 and _alnum(parts[1], 1, 16):
                entry = self._find(parts[1])
                if entry is not None:
                    send(f"VAL {entry['value']}")
                elif negotiated >= 3:
                    send("ERR NOTFOUND")
                else:
                    send("NIL")  # engine: v1 semantics (and v2 downgrades to v1)
            elif cmd == "DEL" and len(parts) >= 2 and _alnum(parts[1], 1, 16):
                entry = self._find(parts[1])
                if entry is not None:
                    entry["used"] = False
                    send("OK")
                else:
                    send("ERR NOTFOUND")
            elif cmd == "KEYS" and len(parts) >= 2 and len(parts[1]) <= 16:
                prefix = parts[1].lower()  # engine: prefix filter ignores case
                count = 0
                for slot in reversed(self.slots):  # engine: newest first
                    if slot["used"] and slot["key"].lower().startswith(prefix):
                        send(f"KEY {slot['key']}")
                        count += 1
                send(f"END {count}")
            else:
                send("ERR FMT")

    # -- accept loop --------------------------------------------------------

    def run(self, port: int) -> None:
        listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.bind(("127.0.0.1", port))
        listener.listen(8)
        print(f"LISTENING {listener.getsockname()[1]}", flush=True)
        while True:
            conn, _ = listener.accept()
            with conn:
                rfile = conn.makefile("rb")
                wfile = conn.makefile("wb")
                try:
                    self.serve_connection(rfile, wfile)
                except (BrokenPipeError, ConnectionResetError):
                    pass
                finally:
                    rfile.close()
                    try:
                        wfile.close()
                    except (BrokenPipeError, ConnectionResetError):
                        pass


def _alnum(value: str, lo: int, hi: int) -> bool:
    return lo <= len(value) <= hi and value.isalnum()


def _parse_int(token: str) -> int | None:
    if not _INT_RE.match(token):
        return None
    return int(token)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=0)
    args = parser.parse_args()
    try:
        Peer().run(args.port)
    except KeyboardInterrupt:
        sys.exit(0)


if __name__ == "__main__":
    main()
