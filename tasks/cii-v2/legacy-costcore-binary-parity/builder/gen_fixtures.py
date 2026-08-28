#!/usr/bin/env python3
"""Fixture generator for legacy-costcore-binary-parity. Same build-gate
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
BINARY = ROOT / "builder" / "costcore-darwin-arm64"


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gold = _load("gold_costcore", ROOT / "builder" / "gold_costcore.py")
naive = _load("naive_costcore", ROOT / "repo" / "costcore.py")


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
    engine = mod.Engine(out=out)
    for line in lines:
        engine.handle(line)
    engine.finish()
    return out.getvalue().splitlines()


def r(sku, qty, cost):
    return f"R {sku} {qty} {cost}"


def i(sku, qty):
    return f"I {sku} {qty}"


families: dict[str, list[list[str]]] = {}


def add(family, *batches):
    families.setdefault(family, []).extend(batches)


# guards: single-layer flows, two-layer plain-FIFO flows where the first
# layer suffices, exact token counts, distinct costs per SKU, no
# equal-cost consecutive receipts, no emptying issues of 100+ units,
# costs under the cap, quantities over 0, case-distinct SKUs, STOCK
# rejects with qty over twice the on-hand
add(
    "guard_single_layer",
    [r("WID1", 500, 1234), i("WID1", 200), i("WID1", 250)],
    [r("BOLT", 60, 999), i("BOLT", 59)],
    [r("NUT7", 80, 50), i("NUT7", 80)],  # empties with 80 units: no quirk fires
    [r("PLATE", 999999, 4321), i("PLATE", 123456)],
)
add(
    "guard_fifo_two_layer",
    [r("A1", 100, 10), r("A1", 50, 20), i("A1", 100)],
    [r("B2", 200, 10), r("B2", 100, 30), i("B2", 150)],
    [r("C3", 300, 77), r("C3", 300, 78), i("C3", 250), i("C3", 40)],
)
add(
    "guard_rejects",
    ["Z NOISE 5"],
    ["R TOOLONGID9 5 10"],
    ["I TOOLONGID9 5"],
    ["R BAD! 5 10"],
    ["R Q1 1234567 10"],
    ["R Q2 12x 10"],
    ["I Q3 1234567"],
    ["R K9 5 12345678"],
    ["R K8 5 12a45"],
    ["R E1 5"],
    ["I E2"],
    [i("GHOST", 25)],  # unknown SKU: STOCK either way
    [r("LOW", 10, 100), i("LOW", 30)],  # over twice on-hand: STOCK either way
)
add("guard_empty_batch", [])
add(
    "guard_trailer_mix",
    [
        r("MIXA", 400, 11),
        r("MIXB", 300, 22),
        i("MIXA", 150),
        "I MIXC 1234567",
        r("MIXA", 200, 33),
        i("MIXB", 90),
        "R MIXD 5 99999999",
        i("MIXA", 100),  # first layer still suffices (250 left in layer 1)
    ],
)

# f2p families: one per quirk, layered constructions with boundary controls
add(
    "f2p_display_stock",  # Q1: newest layer protected while >1 layers exist
    [r("D1", 100, 10), r("D1", 100, 20), i("D1", 150)],
    [r("D2", 50, 10), r("D2", 50, 20), r("D2", 100, 30), i("D2", 120)],
    [r("D3", 100, 10), r("D3", 50, 20), i("D3", 80)],  # first layer suffices: control
    [r("D4", 60, 10), r("D4", 60, 20), i("D4", 60), i("D4", 60)],  # release: control
    [r("D5", 30, 10), r("D5", 30, 20), r("D5", 30, 30), i("D5", 70)],
)
add(
    "f2p_cost_merge",  # Q2: equal-cost receipt merges into the newest layer
    [r("M1", 50, 10), r("M1", 30, 20), r("M1", 30, 20), i("M1", 60)],
    [r("M2", 50, 10), r("M2", 30, 20), r("M2", 30, 21), i("M2", 50)],  # off-by-one: control
    [r("M3", 40, 15), r("M3", 40, 15), i("M3", 60)],
    [r("M4", 20, 5), r("M4", 30, 9), r("M4", 25, 9), r("M4", 25, 9), i("M4", 30)],
)
add(
    "f2p_shrinkage",  # Q3: emptying issues of 100+ units pay 2% half-up
    [r("S1", 100, 50), i("S1", 100)],
    [r("S2", 99, 50), i("S2", 99)],  # 99 units: control
    [r("S3", 150, 33), i("S3", 150)],
    [r("S4", 101, 25), i("S4", 101)],  # surcharge rounds half-up
    [r("S5", 200, 50), i("S5", 100)],  # does not empty: control
)
add(
    "f2p_cost_cap",  # Q4: unit cost silently capped at 9,000,000
    [r("K1", 10, 9000001), i("K1", 5)],
    [r("K2", 10, 9999999), i("K2", 4)],
    [r("K3", 10, 9000000), i("K3", 3)],  # at the cap: control
)
add(
    "f2p_partial_fill",  # Q5: half-or-more availability partially fills
    [r("P1", 50, 100), i("P1", 100)],  # exactly half: partial fill
    [r("P2", 50, 100), i("P2", 101)],  # just under half: STOCK both ways (control)
    [r("P3", 120, 100), i("P3", 200)],  # partial fill that also empties 100+
    [r("P4", 70, 100), i("P4", 70)],  # exact fill: control
)
add(
    "f2p_case_fold",  # Q6: SKUs fold case, first-seen spelling echoed
    [r("Widget", 50, 100), i("WIDGET", 30)],
    [r("aa1", 40, 10), r("AA1", 40, 20), i("aa1", 40)],
    [i("FOO", 5), r("foo", 10, 7), i("Foo", 4)],
    [r("bb2", 40, 10), r("cc3", 40, 20), i("bb2", 30)],  # distinct SKUs: control
)
add(
    "f2p_zero_qty",  # Q7: zero-quantity lines skipped silently
    [r("Z1", 0, 100)],
    [r("Z2", 10, 100), i("Z2", 0), i("Z2", 5)],
    ["R Z3 0 99999999"],  # skipped before the cost field is judged
    [i("Z4", 0)],
    [r("Z5", 1, 100)],  # quantity 1: control
)
add(
    "f2p_lenient_tokens",  # Q8: trailing tokens tolerated
    ["R L1 10 100 extra"],
    [r("L2", 10, 100), "I L2 5 junk junk2"],
    ["R L3 10 100 5 5 5 5"],
    [r("L4", 10, 100), i("L4", 5)],  # exact tokens: control
)

rng = random.Random(20260828)
SKUS = ["WdgA1", "WDGA1", "wdga1", "BoltB2", "CamC3", "DrumD4", "EEE5", "fff6"]
parity = []
for _ in range(2):
    lines = []
    for n in range(rng.randrange(140, 220)):
        roll = rng.random()
        if roll < 0.55:
            lines.append(
                r(
                    rng.choice(SKUS + [f"N{n:04d}"]),
                    rng.choice([0, 1, 99, 100, 101, rng.randrange(1, 1000), rng.randrange(1, 999999)]),
                    rng.choice(
                        [
                            rng.randrange(1, 100),
                            rng.randrange(8_999_990, 9_000_010),
                            9_000_000,
                            9_999_999,
                            rng.randrange(1, 9_999_999),
                        ]
                    ),
                )
            )
        elif roll < 0.9:
            lines.append(
                i(
                    rng.choice(SKUS),
                    rng.choice([0, 1, 50, 99, 100, 150, rng.randrange(1, 2000), rng.randrange(1, 999999)]),
                )
            )
        else:
            lines.append(
                rng.choice(
                    [
                        "R EXTRAA 10 100 TRAILING",
                        "I EXTRAA 5 T1 T2",
                        "R SHORTY 10",
                        "I SHORTY",
                        "NOISE line here",
                        "R TOOLONGID9 5 10",
                        "R BD2 12x 10",
                        "R BD3 5 99999999",
                        "I BD4 1234567",
                    ]
                )
            )
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
            print(f"GOLD MISMATCH [{family}] {batch[:2]}")
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
                print(f"GUARD BROKEN AT BASE [{family}] {batch[:2]}")
                for got, want in zip(naive_out, expected):
                    if got != want:
                        print(f"  bin:   {want}\n  naive: {got}")
                        break
                else:
                    print(f"  lengths {len(naive_out)} vs {len(expected)}")
        fixtures[family].append({"input": batch, "expected": expected})
    if family.startswith("f2p") and not diverges:
        problems += 1
        print(f"WARNING: naive base PASSES every batch in {family}")

for bi in range(40):
    lines = []
    pool = ["Aa", "AA", "aa", "LONGSKU8", f"S{rng.randrange(100)}", "T0"]
    for _ in range(rng.randrange(20, 90)):
        if rng.random() < 0.5:
            lines.append(
                r(
                    rng.choice(pool),
                    rng.randrange(0, 1000),
                    rng.choice([rng.randrange(0, 50), 9_000_000, 9_000_001, rng.randrange(1, 9_999_999)]),
                )
            )
        else:
            lines.append(i(rng.choice(pool), rng.randrange(0, 2000)))
    if run_py(gold, lines) != run_binary(lines):
        problems += 1
        print(f"SWEEP MISMATCH batch {bi}")

print(f"\nfamilies: {len(fixtures)}, problems: {problems}")
if problems:
    sys.exit(1)
out = ROOT / "tests" / "fixtures.json"
out.write_text(json.dumps(fixtures, indent=1) + "\n", encoding="utf-8")
print(f"wrote {out} ({sum(len(v) for v in fixtures.values())} batches)")
