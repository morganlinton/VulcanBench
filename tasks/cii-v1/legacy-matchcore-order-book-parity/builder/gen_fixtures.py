#!/usr/bin/env python3
"""Fixture generator for legacy-matchcore-order-book-parity.

Same build-gate contract as the settlecore generator: gold must match the
binary on every batch and a large random scenario sweep; the naive base
must diverge on every f2p family and agree on every guard batch.

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
BINARY = ROOT / "builder" / "matchcore-darwin-arm64"


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gold = _load("gold_matchcore", ROOT / "builder" / "gold_matchcore.py")
naive = _load("naive_matchcore", ROOT / "repo" / "matchcore.py")


def run_binary(lines: list[str]) -> list[str]:
    proc = subprocess.run(
        [str(BINARY)],
        input="\n".join(lines) + ("\n" if lines else ""),
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert proc.returncode == 0, proc.stderr
    return proc.stdout.splitlines()


def run_py(mod, lines: list[str]) -> list[str]:
    out = io.StringIO()
    engine = mod.Engine(out=out)
    for line in lines:
        engine.handle(line)
    return out.getvalue().splitlines()


families: dict[str, list[list[str]]] = {}


def add(family: str, *batches: list[str]) -> None:
    families.setdefault(family, []).extend(batches)


# ------------------------------------------------------------------ guards
add(
    "guard_basic_matching",
    ["N AAAA S1 S L 1000 500", "N BBBB B1 B L 1000 200", "E"],
    ["N AAAA S1 S L 1000 200", "N BBBB B1 B L 1000 500", "E"],  # partial + rest
    ["N AAAA S1 S L 1005 300", "N AAAA S2 S L 1000 300", "N BBBB B1 B L 1005 600", "E"],
    ["N AAAA B1 B L 995 300", "N BBBB S1 S L 990 100", "E"],  # price improvement
    ["N AAAA S1 S L 1000 300", "N BBBB B1 B I 1000 500", "E"],  # IOC remainder
    ["N AAAA S1 S L 1000 300", "N BBBB B1 B M 0 200", "E"],  # market small sweep
    ["N AAAA B1 B L 980 400", "N AAAA S9 S L 1020 400", "E"],  # resting both sides
)
add(
    "guard_time_priority_small",
    # equal price, both below the (undocumented) priority threshold: FIFO
    ["N AAAA S1 S L 1000 300", "N BBBB S2 S L 1000 300", "N CCCC B1 B L 1000 400", "E"],
)
add(
    "guard_rejects",
    ["N AAAA B1 B L 0 100"],
    ["N AAAA B1 B L 9999995 100", "E"],
    ["N AAAA B1 B L 10000000 100"],
    ["N AAAA B1 B L 1000 0"],
    ["N AAAA B1 B L 1000 1000000"],
    ["C AAAA NOPE"],
    ["N TOOLONGAC B1 B L 1000 100"],
    ["N AAAA VERYLONGID9 B L 1000 100"],
    ["N AAAA B1 Q L 1000 100"],
    ["N AAAA B1 B Z 1000 100"],
    ["Z hello world"],
    ["E", "E"],
)
add(
    "guard_cancel_unfilled",
    ["N AAAA B1 B L 950 400", "C AAAA B1", "C AAAA B1", "E"],
)

# ------------------------------------------------------------------- f2p
add(
    "f2p_priority_class",
    # big order arrives later at same price but matches first
    ["N AAAA S1 S L 1000 300", "N BBBB S2 S L 1000 1500", "N CCCC B1 B L 1000 1600", "E"],
    # class is by ORIGINAL qty: partially-filled big order keeps priority
    [
        "N AAAA S1 S L 1000 1200",
        "N DDDD B0 B L 1000 700",  # S1 partially filled to 500 remaining
        "N BBBB S2 S L 1000 600",
        "N CCCC B1 B L 1000 800",
        "E",
    ],
    # exactly at the threshold boundary
    ["N AAAA S1 S L 1000 300", "N BBBB S2 S L 1000 1000", "N CCCC B1 B L 1000 100", "E"],
    ["N AAAA S1 S L 1000 300", "N BBBB S2 S L 1000 999", "N CCCC B1 B L 1000 100", "E"],
)
add(
    "f2p_self_trade",
    ["N AAAA S1 S L 1000 300", "N AAAA B1 B L 1000 300", "E"],
    # resting cancelled, matching continues to the next level
    ["N AAAA S1 S L 1000 300", "N BBBB S2 S L 1001 300", "N AAAA B1 B L 1001 300", "E"],
)
add(
    "f2p_cancel_replace",
    ["N AAAA B1 B L 990 300", "N AAAA B1 B L 995 400", "E"],
    # invalid replacement leaves the original untouched
    ["N AAAA B1 B L 990 300", "N AAAA B1 B L 990 1000000", "E"],
    # replacement id live on the OTHER account is not a duplicate
    ["N AAAA B1 B L 990 300", "N BBBB B1 B L 991 300", "E"],
)
add(
    "f2p_cancel_ack_qty",
    ["N AAAA S1 S L 1000 500", "N BBBB B1 B L 1000 200", "C AAAA S1", "E"],
)
add(
    "f2p_volatility_band",
    # market sweep stops when price departs >500 from the first fill
    [
        "N AAAA S1 S L 1000 100",
        "N AAAA S2 S L 1400 100",
        "N AAAA S3 S L 1501 100",
        "N BBBB B1 B M 0 300",
        "E",
    ],
    # exactly 500 away still fills
    [
        "N AAAA S1 S L 1000 100",
        "N AAAA S2 S L 1500 100",
        "N BBBB B1 B M 0 200",
        "E",
    ],
    # limit remainder rests after the band trips
    [
        "N AAAA S1 S L 1000 100",
        "N AAAA S2 S L 1600 100",
        "N BBBB B1 B L 1600 200",
        "E",
    ],
)
add(
    "f2p_tick_regime",
    ["N AAAA B1 B L 100002 100"],
    ["N AAAA B1 B L 100005 100", "E"],
    ["N AAAA B1 B L 99999 100", "E"],
    ["N AAAA B1 B L 100000 100", "E"],
)
add(
    "f2p_book_hash",
    # sub-100 remainders exist on the book but not in the hash
    ["N AAAA S1 S L 1000 99", "N BBBB B1 B L 900 100", "E"],
    ["N AAAA S1 S L 1000 150", "N BBBB B1 B L 1000 51", "E"],  # filled down to 99
    ["N AAAA S1 S L 1000 150", "N BBBB B1 B L 1000 50", "E"],  # 100 stays in
)
add(
    "f2p_lenient_tokens",
    # engine tolerates trailing tokens; the spec-faithful rewrite must too
    ["N AAAA B1 B L 1000 100 EXTRA", "E"],
    ["N AAAA S7 S L 1000 200", "C AAAA S7 NOTE", "E"],
    ["N AAAA B1 B L 1000"],  # missing qty echoes the parsed id
)

# f2p_parity: long scripted sessions mixing everything
rng = random.Random(20260828)
parity_sessions = []
for _ in range(3):
    lines = []
    accounts = ["AAAA", "BBBB", "CCCC", "DDDD"]
    live_ids: list[tuple[str, str]] = []
    counter = 0
    for _ in range(rng.randrange(120, 200)):
        roll = rng.random()
        acct = rng.choice(accounts)
        if roll < 0.72 or not live_ids:
            counter += 1
            reuse = live_ids and rng.random() < 0.12
            if reuse:
                acct, clordid = rng.choice(live_ids)
            else:
                clordid = f"O{counter:05d}"
            side = rng.choice("BS")
            type_ = rng.choice(["L", "L", "L", "I", "M"])
            price = rng.choice(
                [
                    rng.randrange(900, 1101),
                    rng.randrange(400, 1701),
                    rng.choice([99998, 99999, 100000, 100001, 100005, 100010]),
                    0,
                ]
            )
            qty = rng.choice(
                [rng.randrange(1, 200), rng.randrange(50, 1500), 999, 1000, 1001, 99, 100]
            )
            lines.append(f"N {acct} {clordid} {side} {type_} {price} {qty}")
            live_ids.append((acct, clordid))
        elif roll < 0.9:
            acct, clordid = rng.choice(live_ids)
            lines.append(f"C {acct} {clordid}")
        else:
            lines.append(
                rng.choice(
                    [
                        "N AAAA BADQTYX B L 1000 0",
                        "N AAAA LONGID999 B L 1000 100",
                        "C AAAA GHOST",
                        "Z noise",
                        "N AAAA TRAIL B L 1000 100 X Y",
                    ]
                )
            )
        if rng.random() < 0.04:
            lines.append("E")
    lines.append("E")
    parity_sessions.append(lines)
add("f2p_parity", *parity_sessions)

# ---------------------------------------------------------------- generate
fixtures: dict[str, list[dict]] = {}
problems = 0
for family, batches in families.items():
    fixtures[family] = []
    diverges = False
    for batch in batches:
        expected = run_binary(batch)
        gold_out = run_py(gold, batch)
        if gold_out != expected:
            problems += 1
            print(f"GOLD MISMATCH [{family}] input={batch}")
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
                print(f"GUARD BROKEN AT BASE [{family}] input={batch}")
        fixtures[family].append({"input": batch, "expected": expected})
    if family.startswith("f2p") and not diverges:
        problems += 1
        print(f"WARNING: naive base PASSES every batch in {family}")

# random sweep: gold-vs-binary parity only
for i in range(40):
    lines = []
    counter = 0
    live: list[tuple[str, str]] = []
    for _ in range(rng.randrange(30, 120)):
        counter += 1
        if live and rng.random() < 0.25:
            acct, cid = rng.choice(live)
            lines.append(f"C {acct} {cid}")
            continue
        acct = rng.choice(["AAAA", "BBBB", "CCCC"])
        cid = f"S{counter:05d}" if rng.random() < 0.9 else rng.choice(live or [("AAAA", "X1")])[1]
        lines.append(
            f"N {acct} {cid} {rng.choice('BS')} {rng.choice(['L','I','M'])} "
            f"{rng.randrange(0, 110000)} {rng.randrange(0, 1100)}"
        )
        live.append((acct, cid))
        if rng.random() < 0.05:
            lines.append("E")
    lines.append("E")
    if run_py(gold, lines) != run_binary(lines):
        problems += 1
        print(f"SWEEP MISMATCH batch {i}")

print(f"\nfamilies: {len(fixtures)}, problems: {problems}")
if problems:
    sys.exit(1)
out = ROOT / "tests" / "fixtures.json"
out.write_text(json.dumps(fixtures, indent=1) + "\n", encoding="utf-8")
print(f"wrote {out} ({sum(len(v) for v in fixtures.values())} batches)")
