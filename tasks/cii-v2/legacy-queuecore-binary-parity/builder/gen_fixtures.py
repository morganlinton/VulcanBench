#!/usr/bin/env python3
"""Fixture generator for legacy-queuecore-binary-parity. Same build-gate
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
BINARY = ROOT / "builder" / "queuecore-darwin-arm64"


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gold = _load("gold_queuecore", ROOT / "builder" / "gold_queuecore.py")
naive = _load("naive_queuecore", ROOT / "repo" / "queuecore.py")


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


families: dict[str, list[list[str]]] = {}


def add(family, *batches):
    families.setdefault(family, []).extend(batches)


# guards: pure enqueue/dequeue flows (no F ever accepted, so no penalty,
# no undo activity, no dead letters), same-spelling ids distinct beyond
# case, exact token counts, and reject lanes that bounce identically in
# both implementations (F only in STATE-reject positions)
add(
    "guard_order",
    ["N A1 500", "N B1 700", "N C1 600", "D", "D", "D", "D"],
    ["N A1 300", "N B1 300", "N C1 300", "D", "D", "D"],  # pure FIFO tie
    ["N A1 999", "N B1 1", "D", "D"],
    ["N A1 400", "D", "N B1 500", "N A1 450", "D", "D"],  # re-enqueue after dequeue
    ["D", "D"],
    ["N LONE 250", "D", "D"],
)
add(
    "guard_drain",
    ["K", "K"],  # empty dead-letter drains
    ["N A1 500", "K", "D", "K", "D"],
    ["N A1 500", "N B1 600", "K", "D", "D", "D"],
)
add(
    "guard_rejects",
    ["N A1"],
    ["N"],
    ["N TOOLONG99 500"],
    ["N A_1 500"],
    ["Z noise line"],
    ["QQ A1 500"],
    ["N A1 0"],
    ["N A1 000"],
    ["N A1 1000"],
    ["N A1 12a"],
    ["N A1 500", "N A1 600"],  # duplicate live item: STATE in both
    ["F A1"],  # nothing ever dequeued
    ["F"],
    ["N A1 500", "F A1"],  # live but never dequeued
    ["N A1 500", "N B1 400", "D", "F B1"],  # wrong item (most recent is A1)
    ["N A1 500", "N B1 600", "D", "F A1"],  # most recent dequeue is B1
)

# f2p families
add(
    "f2p_requeue_penalty",
    # engine requeues at original minus 10 per accumulated failure; the
    # only observable is which item drains first afterwards
    ["N A1 500", "N B1 495", "D", "F A1", "D", "D", "D"],
    ["N A1 500", "N B1 485", "D", "F A1", "D", "F A1", "D", "D", "D"],  # 2nd fail
    ["N A1 500", "D", "F A1", "D"],  # alone in queue: order control
)
add(
    "f2p_tie_aging",
    # requeued items keep their ORIGINAL arrival sequence (jump the FIFO
    # line at the new priority); K-drained items get FRESH sequence
    ["N B1 505", "N A1 515", "N C1 505", "D", "F A1", "D", "D", "D"],
    [
        "N A1 300", "D", "F A1", "D", "F A1", "D", "F A1",  # third fail: DLQ
        "N B1 300", "N C1 300", "K", "D", "D", "D",  # drained A1 goes LAST
    ],
    ["N B1 500", "N A1 500", "D", "F B1", "D", "D"],  # requeue below the tie: control
)
add(
    "f2p_sentinel",
    ["N A1 110", "N B1 90", "D", "F A1", "D", "D"],  # 110-10=100: instant DLQ
    ["N A1 210", "D", "F A1", "D"],  # 210-10=200: instant DLQ
    ["N A1 120", "D", "F A1", "D", "F A1", "D"],  # second fail lands on 100
    ["N A1 110", "D", "F A1", "K", "D", "D"],  # drain the sentinel victim
    ["N A1 111", "D", "F A1", "D"],  # 101: control
    ["N A1 105", "D", "F A1", "D"],  # 95: control
)
add(
    "f2p_cap999",
    # the requeue penalty computes from an internal 1000 for prio 999
    ["N B1 990", "N A1 999", "D", "F A1", "D", "D"],  # engine: B1 wins the tie
    ["N A1 999", "N B1 990", "D", "F A1", "D", "D"],  # engine: A1 keeps its old seq
    ["N A1 999", "N B1 980", "D", "F A1", "D", "F A1", "D", "D"],  # 2nd requeue: 980
    ["N A1 998", "N B1 989", "D", "F A1", "D", "D"],  # 998-10=988: control
)
add(
    "f2p_undo_two_deep",
    # the engine accepts F for the second-most-recent dequeue once the
    # most recent one was failed
    ["N A1 500", "N B1 400", "D", "D", "F B1", "F A1", "D", "D", "D"],
    ["N A1 500", "N B1 400", "N C1 300", "D", "D", "D", "F C1", "F B1", "F A1"],
    ["N A1 500", "N B1 400", "D", "D", "F A1"],  # no first fail: STATE control
)
add(
    "f2p_item_case",
    ["N Job1 500", "N JOB1 400"],
    ["N Job1 500", "D", "F JOB1", "D"],
    ["N aa 300", "D", "N AA 300", "D"],  # first-seen spelling echoed
    ["N aa 300", "N bb 300", "D", "D"],  # distinct ids: control
)
add(
    "f2p_trailing",
    ["N T1 500 urgent", "D"],
    ["N T2 400", "D now"],
    ["K all"],
    ["N T3 300", "D", "F T3 oops", "D"],
)

ITEMS = ["JobA", "joba", "JOBA", "wrk1", "WRK1", "deep99", "mx", "MX"]
PRIOS = [999, 990, 980, 110, 111, 120, 210, 105, 90, 100, 200, 300, 500, 1]
BAD = [
    "N EXTRAA 500 TRAILING",
    "N SHORTY",
    "NOISE",
    "N BADP 1000",
    "N BADP 12a",
    "N TOOLONG99 500",
    "D twice",
    "K all",
    "F",
    "F NOSUCH",
]


def build_random_batch(rng, n):
    """Random session; F commands are steered at the gold engine's undo
    buffer often enough to guarantee fail/requeue/dead-letter activity."""
    eng = gold.Engine(out=io.StringIO())
    lines = []
    for i in range(n):
        r = rng.random()
        if r < 0.40:
            line = f"N {rng.choice(ITEMS + [f'R{i:04d}'])} {rng.choice(PRIOS + [rng.randrange(1, 1000)])}"
        elif r < 0.70:
            line = "D"
        elif r < 0.88:
            target = None
            if rng.random() < 0.75:
                if eng.u1 is not None and eng.u1_state == 1:
                    target = eng.seen[eng.u1]
                elif eng.u2 is not None and eng.u2_state == 1:
                    target = eng.seen[eng.u2]
            if target is None:
                target = rng.choice(ITEMS)
            if rng.random() < 0.2:
                target = target.swapcase()
            line = f"F {target}"
        elif r < 0.94:
            line = "K"
        else:
            line = rng.choice(BAD)
        lines.append(line)
        eng.handle(line)
    return lines


rng = random.Random(20260828)
parity = [build_random_batch(rng, rng.randrange(150, 220)) for _ in range(2)]
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
        fixtures[family].append({"input": batch, "expected": expected})
    if family.startswith("f2p") and not diverges:
        problems += 1
        print(f"WARNING: naive base PASSES every batch in {family}")

for i in range(40):
    lines = build_random_batch(rng, rng.randrange(20, 90))
    if run_py(gold, lines) != run_binary(lines):
        problems += 1
        print(f"SWEEP MISMATCH batch {i}")

print(f"\nfamilies: {len(fixtures)}, problems: {problems}")
if problems:
    sys.exit(1)
out = ROOT / "tests" / "fixtures.json"
out.write_text(json.dumps(fixtures, indent=1) + "\n", encoding="utf-8")
print(f"wrote {out} ({sum(len(v) for v in fixtures.values())} batches)")
