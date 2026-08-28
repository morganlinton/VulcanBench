#!/usr/bin/env python3
"""Fixture generator for legacy-syncpeer-wire-parity.

Fixtures are SESSIONS: an ordered list of connections against one server
process, each connection an ordered list of client lines. Expected output
is the per-connection list of server reply lines. The same socket driver
used here ships with the hidden tests.

Build gate: gold must match the binary on every session and a random
sweep; the naive base must diverge on every f2p family and agree on every
guard session.

Usage: python3 builder/gen_fixtures.py <task-root>
"""

from __future__ import annotations

import importlib.util
import json
import random
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
BINARY = ROOT / "builder" / "syncpeer-darwin-arm64"


def drive_server(argv: list[str], connections: list[list[str]]) -> list[list[str]]:
    """Start a server process, run the scripted connections, return replies."""
    proc = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    try:
        banner = proc.stdout.readline().strip()
        assert banner.startswith("LISTENING "), banner
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
                    if reply.startswith("KEY ") or (
                        line.split()[:1] == ["KEYS"] and reply.startswith("KEY ")
                    ):
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


def run_binary(connections):
    return drive_server([str(BINARY)], connections)


def run_py(path: Path, connections):
    return drive_server([sys.executable, str(path)], connections)


GOLD = ROOT / "builder" / "gold_syncpeer.py"
NAIVE = ROOT / "repo" / "syncpeer.py"

families: dict[str, list[list[list[str]]]] = {}


def add(family, *sessions):
    families.setdefault(family, []).extend(sessions)


H = "HELLO 3 nodeA"

# ------------------------------------------------------------------ guards
add(
    "guard_basic_kv",
    [[H, "PUT k1 hello", "GET k1", "DEL k1", "GET k1", "DEL k1"]],
    [[H, "PUT a1 x", "PUT b2 y", "GET a1", "GET b2"]],
)
add(
    "guard_handshake_errors",
    [["HELLO 0 nodeA"], ["HELLO 10 nodeA"], ["HELLO 3 toolongnode9"], ["NOTHELLO 3 nodeA"], ["HELLO x nodeA"]],
)
add(
    "guard_fmt_errors",
    [[H, "PUT onlykey", "GET", "PUT k1 " + "v" * 65, "PUT " + "k" * 17 + " v", "WHAT k"]],
)
add(
    "guard_persistence_across_connections",
    [
        ["HELLO 3 nodeA", "PUT k1 v1"],
        ["HELLO 3 nodeB", "GET k1"],
    ],
)

# ------------------------------------------------------------------- f2p
add(
    "f2p_version_negotiation",
    [
        ["HELLO 2 nodeA", "GET missing", "BYE"],   # v2 -> v1: NIL, WELCOME 1
        ["HELLO 1 nodeB", "GET missing", "BYE"],   # v1: NIL (naive: NOTFOUND)
        ["HELLO 3 nodeC", "GET missing", "BYE"],   # v3: NOTFOUND (control)
        ["HELLO 9 nodeD", "GET missing", "BYE"],   # >3 -> 3 (control)
    ],
)
add(
    "f2p_session_identity",
    [
        ["HELLO 3 nodeA", "BYE"],
        ["HELLO 3 nodeB", "BYE"],
        ["HELLO 3 nodeA", "BYE"],  # returning node keeps S1
        ["HELLO 3 nodeC", "BYE"],  # next fresh node gets S3
    ],
)
add(
    "f2p_keys_ordering",
    [
        [
            H,
            "PUT alpha 1", "PUT beta 2", "PUT ALPHA2 3", "PUT gamma 4",
            "DEL beta",
            "PUT beta 5",  # re-inserted: newest slot
            "KEYS a", "KEYS AL", "KEYS beta", "KEYS x",
            "BYE",
        ]
    ],
)
add(
    "f2p_put_overwrite_echo",
    [
        [
            H,
            "PUT k1 first", "PUT k1 second", "PUT k1 second", "GET k1",
            "BYE",
        ]
    ],
)
add(
    "f2p_value_truncation",
    [
        [
            H,
            "PUT k1 " + "A" * 64,
            "GET k1",
            "PUT k1 " + "A" * 48,  # identical after truncation: plain OK
            "PUT k2 " + "B" * 49,
            "GET k2",
            "BYE",
        ]
    ],
)
add(
    "f2p_goodbye_count",
    [
        [H, "BYE"],
        [H, "PUT k9 v", "GET k9", "BYE"],
    ],
)
add(
    "f2p_lenient_tokens",
    [
        [H, "PUT k1 v1 extra", "GET k1 extra", "DEL k1 now please", "BYE"],
        [["HELLO", "3", "nodeZ", "trailing"][0] + " 3 nodeZ trailing", "BYE"],
    ],
)

rng = random.Random(20260830)
parity_sessions = []
for _ in range(2):
    connections = []
    for c in range(rng.randrange(4, 8)):
        node = rng.choice(["nodeA", "nodeB", "nodeC", "fresh" + str(rng.randrange(10))])
        version = rng.choice(["1", "2", "3", "4", "9"])
        lines = [f"HELLO {version} {node}"]
        for _ in range(rng.randrange(3, 14)):
            roll = rng.random()
            key = rng.choice(["alpha", "Alpha", "beta", "b2", "Zed", "k" + str(rng.randrange(6))])
            if roll < 0.45:
                lines.append(f"PUT {key} {rng.choice(['v1', 'v2', 'X' * rng.randrange(1, 64)])}")
            elif roll < 0.65:
                lines.append(f"GET {key}")
            elif roll < 0.78:
                lines.append(f"DEL {key}")
            elif roll < 0.92:
                lines.append("KEYS " + rng.choice(["a", "A", "k", "", "Z"]).strip() or "KEYS a")
            else:
                lines.append(rng.choice(["PUT solo", "NOPE x", "GET " + "q" * 17]))
        lines.append("BYE")
        connections.append(lines)
    parity_sessions.append(connections)
add("f2p_parity", *parity_sessions)

# ---------------------------------------------------------------- generate
fixtures = {}
problems = 0
for family, sessions in families.items():
    fixtures[family] = []
    diverges = False
    for connections in sessions:
        expected = run_binary(connections)
        gold_out = run_py(GOLD, connections)
        if gold_out != expected:
            problems += 1
            print(f"GOLD MISMATCH [{family}]\n  bin:  {expected}\n  gold: {gold_out}")
        naive_out = run_py(NAIVE, connections)
        if naive_out != expected:
            diverges = True
            if family.startswith("guard"):
                problems += 1
                print(f"GUARD BROKEN AT BASE [{family}]\n  bin:   {expected}\n  naive: {naive_out}")
        fixtures[family].append({"connections": connections, "expected": expected})
    if family.startswith("f2p") and not diverges:
        problems += 1
        print(f"WARNING: naive base PASSES every session in {family}")

for i in range(12):
    connections = []
    for _ in range(rng.randrange(2, 6)):
        lines = [f"HELLO {rng.choice(['1', '2', '3', '5'])} n{rng.randrange(5)}"]
        for _ in range(rng.randrange(2, 10)):
            lines.append(
                rng.choice(
                    [
                        f"PUT key{rng.randrange(8)} val{rng.randrange(9)}",
                        f"GET key{rng.randrange(8)}",
                        f"DEL key{rng.randrange(8)}",
                        "KEYS key",
                        "KEYS KEY",
                        "BADCMD",
                    ]
                )
            )
        lines.append("BYE")
        connections.append(lines)
    if run_py(GOLD, connections) != run_binary(connections):
        problems += 1
        print(f"SWEEP MISMATCH session-set {i}")

print(f"\nfamilies: {len(fixtures)}, problems: {problems}")
if problems:
    sys.exit(1)
out = ROOT / "tests" / "fixtures.json"
out.write_text(json.dumps(fixtures, indent=1) + "\n", encoding="utf-8")
print(f"wrote {out} ({sum(len(v) for v in fixtures.values())} sessions)")
