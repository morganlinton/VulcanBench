#!/usr/bin/env python3
"""Fixture generator for legacy-qlite-store-parity. Same build-gate
contract as the other opaque-component generators.

Usage: python3 builder/gen_fixtures.py <task-root>
"""

from __future__ import annotations

import importlib.util
import io
import json
import random
import subprocess
import sys
from pathlib import Path

ROOT = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
BINARY = ROOT / "builder" / "qlite-darwin-arm64"


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gold = _load("gold_qlite", ROOT / "builder" / "gold_qlite.py")
naive = _load("naive_qlite", ROOT / "repo" / "qlite.py")


def run_binary(lines):
    proc = subprocess.run(
        [str(BINARY)],
        input="\n".join(lines) + ("\n" if lines else ""),
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert proc.returncode == 0, proc.stderr
    return proc.stdout.splitlines()


def run_py(mod, lines):
    out = io.StringIO()
    store = mod.Store(out=out)
    for line in lines:
        store.handle(line)
    return out.getvalue().splitlines()


families: dict[str, list[list[str]]] = {}


def add(family, *batches):
    families.setdefault(family, []).extend(batches)


# ------------------------------------------------------------------ guards
add(
    "guard_crud",
    ["INS a1 Alice 100", "GET a1", "DEL a1", "GET a1", "DEL a1"],
    ["INS a1 Alice 100", "INS b2 Bob 200", "INS c3 Carol 300", "LIST", "SUM", "AVG"],
    ["GET ghost", "AVG", "SUM", "LIST"],
)
add(
    "guard_find_exact_case",
    ["INS a1 Alice 10", "INS b2 Bob 20", "FIND Alice", "FIND Bob", "FIND Carol", "FIND Al*"],
    ["INS a1 Ann 1", "INS b2 ANNE 2", "FIND *"],
)
add(
    "guard_range_interior",
    # no row sits exactly at hi, so inclusive-vs-exclusive cannot show
    ["INS a1 A 100", "INS b2 B 200", "INS c3 C 300", "RANGE 50 250", "RANGE 300 300"],
    ["INS a1 A 0", "RANGE -10 5", "RANGE 0 0"],
)
add(
    "guard_fmt_errors",
    ["INS toolongid9 Name 5"],
    ["INS a1 ThisNameIsWayTooLong 5"],
    ["INS a1 Name 1000000"],
    ["INS a1 Name -1000000"],
    ["INS a1 Name x5"],
    ["INS a1"],
    ["RANGE 5"],
    ["NOPE what"],
    ["FIND " + "A" * 18],
)

# ------------------------------------------------------------------- f2p
add(
    "f2p_slot_reuse",
    ["INS a1 A 1", "INS b2 B 2", "INS c3 C 3", "DEL b2", "INS d4 D 4", "LIST"],
    [
        "INS a1 A 1", "INS b2 B 2", "INS c3 C 3",
        "DEL a1", "DEL c3",
        "INS d4 D 4", "INS e5 E 5",
        "LIST", "FIND *", "RANGE -100 100",
    ],
    # deleted-then-refilled slot, then the freed one reused again
    ["INS a1 A 1", "DEL a1", "INS b2 B 2", "INS c3 C 3", "DEL b2", "INS d4 D 4", "LIST"],
)
add(
    "f2p_find_case",
    ["INS a1 Alice 1", "FIND alice", "FIND ALICE", "FIND AlIcE", "FIND aLICE"],
    ["INS a1 bob 1", "FIND Bob", "FIND bOB"],
)
add(
    "f2p_wildcard",
    ["INS a1 Alice 1", "INS b2 Ane 2", "FIND A*e", "FIND A*", "FIND *e", "FIND *"],
)
add(
    "f2p_range_bounds",
    ["INS a1 A 100", "INS b2 B 200", "INS c3 C 300", "RANGE 100 300", "RANGE 100 301"],
    ["INS a1 A -50", "INS b2 B -20", "RANGE -50 -20", "RANGE -20 -20"],
)
add(
    "f2p_dup_update",
    ["INS a1 Alice 10", "INS a1 Bob 99", "GET a1", "LIST"],
    ["INS a1 Alice 10", "INS a1 Bob 1000000", "GET a1"],  # invalid dup: FMT, untouched
)
add(
    "f2p_name_truncation",
    [
        "INS a1 AlphabetSoupXYZ 5",
        "GET a1",
        "FIND AlphabetSoupXYZ",
        "FIND AlphabetSoup",
        "FIND AlphabetSoup*",
    ],
)
add(
    "f2p_avg_truncation",
    ["INS a1 A -5", "INS b2 B -4", "AVG"],
    ["INS a1 A -7", "INS b2 B 4", "AVG", "SUM"],
    ["INS a1 A 7", "INS b2 B 4", "AVG"],  # control: same both ways
)
add(
    "f2p_sum_wrap",
    [f"INS r{i:05d} N 999999" for i in range(2400)] + ["SUM", "AVG"],
)

rng = random.Random(20260829)
parity = []
for _ in range(2):
    lines = []
    ids = []
    counter = 0
    for _ in range(rng.randrange(150, 260)):
        roll = rng.random()
        if roll < 0.45 or not ids:
            counter += 1
            rid = f"r{counter:04d}" if rng.random() < 0.85 else (rng.choice(ids) if ids else "x1")
            name = rng.choice(
                ["Alice", "alice", "Bob", "AlphabetSoupXYZ", "Zed", "annA", "Q" * 16, "m"]
            )
            score = rng.choice([rng.randrange(-999999, 1000000), -1, 0, 1, 999999, -999999])
            lines.append(f"INS {rid} {name} {score}")
            ids.append(rid)
        elif roll < 0.6:
            lines.append(f"DEL {rng.choice(ids)}")
        elif roll < 0.75:
            lines.append(f"GET {rng.choice(ids)}")
        elif roll < 0.85:
            lines.append(
                "FIND " + rng.choice(["Alice", "alice", "A*", "a*", "*", "A*e", "Q*", "m"])
            )
        elif roll < 0.93:
            lo = rng.randrange(-1000, 1000)
            lines.append(f"RANGE {lo} {lo + rng.randrange(0, 2000)}")
        else:
            lines.append(rng.choice(["LIST", "SUM", "AVG", "INS bad 5_0 5", "GET x y z", "LIST 9"]))
    parity.append(lines)
add("f2p_parity", *parity)

# ---------------------------------------------------------------- generate
fixtures = {}
problems = 0
for family, batches in families.items():
    fixtures[family] = []
    diverges = False
    for batch in batches:
        expected = run_binary(batch)
        gold_out = run_py(gold, batch)
        if gold_out != expected:
            problems += 1
            print(f"GOLD MISMATCH [{family}]")
            for got, want in zip(gold_out, expected):
                if got != want:
                    print(f"  bin:  {want}\n  gold: {got}")
                    break
            else:
                print(f"  lengths {len(gold_out)} vs {len(expected)}")
        naive_out = run_py(naive, batch)
        if naive_out != expected:
            diverges = True
            if family.startswith("guard"):
                problems += 1
                print(f"GUARD BROKEN AT BASE [{family}] input={batch[:3]}")
        fixtures[family].append({"input": batch, "expected": expected})
    if family.startswith("f2p") and not diverges:
        problems += 1
        print(f"WARNING: naive base PASSES every batch in {family}")

for i in range(40):
    lines = []
    counter = 0
    ids = []
    for _ in range(rng.randrange(40, 160)):
        counter += 1
        roll = rng.random()
        if roll < 0.5:
            rid = f"s{counter:04d}" if rng.random() < 0.8 else (rng.choice(ids) if ids else "x")
            lines.append(
                f"INS {rid} {rng.choice(['a', 'Bb', 'X' * 16, 'MixedCase', 'zz9'])} "
                f"{rng.randrange(-999999, 1000000)}"
            )
            ids.append(rid)
        elif ids and roll < 0.7:
            lines.append(rng.choice(["DEL ", "GET "]) + rng.choice(ids))
        else:
            lines.append(
                rng.choice(
                    ["LIST", "SUM", "AVG", "FIND *", "FIND M*", "FIND mixedcase",
                     f"RANGE {rng.randrange(-100, 0)} {rng.randrange(0, 100)}"]
                )
            )
    if run_py(gold, lines) != run_binary(lines):
        problems += 1
        print(f"SWEEP MISMATCH batch {i}")

print(f"\nfamilies: {len(fixtures)}, problems: {problems}")
if problems:
    sys.exit(1)
out = ROOT / "tests" / "fixtures.json"
out.write_text(json.dumps(fixtures, indent=1) + "\n", encoding="utf-8")
print(f"wrote {out} ({sum(len(v) for v in fixtures.values())} batches)")
