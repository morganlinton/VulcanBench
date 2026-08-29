#!/usr/bin/env python3
"""Build-time fixture generator for legacy-pacecore-binary-parity.

Runs the compiled legacy engine over structured corner batches and a large
seeded random corpus, and:

1. verifies the gold Python implementation matches the binary EXACTLY on
   every batch (the build gate for the task itself);
2. verifies the naive base implementation diverges on every fail-to-pass
   family and agrees on every guard batch;
3. writes tests/fixtures.json: {family: [{"input": [...], "expected": [...]}]}.

Usage: python3 builder/gen_fixtures.py <task-root>
"""

from __future__ import annotations

import importlib.util
import json
import random
import subprocess
import sys
from pathlib import Path

ROOT = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
BINARY = ROOT / "builder" / "pacecore-darwin-arm64"


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gold = _load("gold_pacecore", ROOT / "builder" / "gold_pacecore.py")
naive = _load("naive_pacecore", ROOT / "repo" / "pacecore.py")


def run_binary(lines: list[str]) -> list[str]:
    proc = subprocess.run(
        [str(BINARY)],
        input="\n".join(lines) + "\n",
        capture_output=True,
        text=True,
        timeout=60,
    )
    assert proc.returncode == 0, proc.stderr
    return proc.stdout.splitlines()


def run_py(mod, lines: list[str]) -> list[str]:
    return mod.process([line + "\n" for line in lines])


# ---------------------------------------------------------------- families

rng = random.Random(20260829)
families: dict[str, list[list[str]]] = {}


def add(family: str, *batches: list[str]) -> None:
    families.setdefault(family, []).extend(batches)


# guards: nominal behavior where the written spec is accurate.
# Guard territory: budgets under 1,000,000 (or hours 06+ only), 2-digit
# hours, no request exactly matching the remaining allowance, days spent
# evenly enough that the flat audit schedule equals the live grants and
# the carry stays under the cap, single-case campaign ids.
add(
    "guard_live_flows",
    # rolling within-day allowance is documented: back-loaded LIVE grants
    # are guard territory as long as no true-up runs.
    ["C ad1 240000", "S ad1 00 3000", "S ad1 02 5000", "S ad1 02 40000", "S ad1 10 1"],
    ["C ad1 240000", "C ad2 120000", "S ad1 03 9999", "S ad2 03 4999", "S ad2 07 5001"],
    ["C big1 999999", "S big1 06 249990", "S big1 07 500"],  # under damper budget
    ["C ad1 120000", "S ad1 23 119990", "S ad1 23 100"],  # overshoot then P 0
    ["C ad1 240000", "S ad1 00 20000", "S ad1 00 7"],  # overshoot, same hour P 0
)
add(
    "guard_even_trueups",
    # 20 hours x 10000 on a 240000 budget: audit == live, carry 40000 < cap
    ["C ad1 240000"]
    + [f"S ad1 {h:02d} 10000" for h in range(20)]
    + ["U ad1"]
    + [f"S ad1 {h:02d} 9000" for h in range(22)]
    + ["U ad1"],
    # raw carry exactly at the cap (no excess): 24 x 4000 on 120000
    ["C ad2 120000"] + [f"S ad2 {h:02d} 4000" for h in range(24)] + ["U ad2"],
    # fully spent day: carry 0
    ["C ad3 120000"] + [f"S ad3 {h:02d} 5000" for h in range(24)] + ["U ad3", "S ad3 00 4900"],
    ["C ad4 240000", "S ad4 06 9500", "S ad4 07 9500"],  # no true-up at all
)
add(
    "guard_rejects",
    ["C toolong99 240000"],  # id length 9
    ["C ad-1 240000"],  # non-alnum id
    ["C ad1 999"],  # 3-digit budget
    ["C ad1 1234567890"],  # 10-digit budget
    ["C ad1 12x4"],  # non-digit budget
    ["C ad1"],
    ["C"],
    ["C ad1 240000", "C ad1 120000"],  # same-case DUP in both
    ["C ad1 240000", "S ad1 24 100"],  # hour 24
    ["C ad1 240000", "S ad1 2x 100"],  # non-digit hour
    ["C ad1 240000", "S ad1 005 100"],  # 3-digit hour
    ["C ad1 240000", "S ad1 05 0"],  # zero request
    ["C ad1 240000", "S ad1 05 12345678"],  # 8-digit request
    ["C ad1 240000", "S ad1 05 12a"],  # non-digit request
    ["C ad1 240000", "S ad1 05"],
    ["C ad1 240000", "S ad1"],
    ["S"],
    ["S ghost 05 100"],  # unknown campaign
    ["C ad1 240000", "S ad1 10 100", "S ad1 05 100"],  # ORDER
    ["U ghost"],
    ["U bad-id"],
    ["U"],
    ["Q what is this"],
    ["X"],
    ["c ad1 240000"],  # lowercase command letter
    ["s ad1 05 100"],
)
add("guard_empty", [], ["C ad1 240000"], ["", "C ad1 240000", ""], ["C ad1 240000", "U ad1"])
add(
    "guard_trailer",
    [
        "C ad1 240000",
        "C ad2 120000",
        "S ad1 00 3000",
        "S ghost 05 100",
        "S ad1 24 5",
        "U ad1",
        "U ghost",
        "C ad1 999",
        "Q junk",
        "S ad2 06 100",
    ],
)

# f2p families: engine behavior the written spec gets wrong or omits

add(
    "f2p_audit_clamp",
    # back-loaded day: audit clamps each hour to daily/24 flat
    ["C a 240000", "S a 00 1", "S a 12 90000", "U a"],
    # clamp inflates the carry past what the live grants imply
    ["C a 120000", "S a 23 110000", "U a"],
    ["C a 120000", "S a 10 30000", "S a 11 2000", "S a 20 40000", "U a"],
    # two back-loaded days in a row
    ["C a 240000", "S a 20 150000", "U a", "S a 22 100000", "U a"],
    # control: even day, audit equals live grants in both
    ["C a 240000", "S a 00 9000", "S a 01 9000", "U a"],
)
add(
    "f2p_rollover_debt",
    # capped carry pushes hidden debt that throttles hour 00 next day
    ["C a 120000", "S a 23 110000", "U a", "S a 00 20000"],
    # an untouched day generates debt too; it stacks across days
    ["C b 240000", "U b", "S b 00 5000", "U b", "S b 00 5000"],
    # debt survives a day whose hour 00 never accrues
    ["C c 120000", "S c 23 110000", "U c", "S c 05 3000", "U c", "S c 00 9000"],
    # hour 00 skipped: throttled slice still accrues inside a later hour
    ["C d 120000", "S d 23 110000", "U d", "S d 03 25000"],
    # control: carry under the cap, no debt, next day unthrottled
    ["C e 240000"] + [f"S e {h:02d} 10000" for h in range(20)] + ["U e", "S e 00 11000"],
)
add(
    "f2p_overnight_damper",
    # budget at the threshold, hour inside the window: half allowance
    ["C big 1000000", "S big 05 200000"],
    ["C big 1000000", "S big 00 9999999", "S big 01 9999999"],
    # withheld half stays available for later hours
    ["C big 1000000", "S big 00 41666", "S big 01 50000"],
    # dark-pair controls: 999999 at hour 05, and 1000000 at hour 06
    ["C sm 999999", "S sm 05 200000"],
    ["C big 1000000", "S big 06 250000"],
    ["C big 2400000", "S big 03 300000", "S big 06 300000", "U big"],
)
add(
    "f2p_exact_exhaustion",
    # request exactly the remaining allowance: same-hour follow-up rejects
    ["C a 240000", "S a 00 10000", "S a 00 5"],
    ["C a 240000", "S a 03 40000", "S a 03 1", "S a 04 1"],
    # exact match under the damper: half remains but the hour still closes
    ["C big 1000000", "S big 00 20833", "S big 00 1"],
    # a new hour clears the flag; a fresh exact match closes it again
    ["C c 240000", "S c 00 10000", "S c 01 4000", "S c 01 6000", "S c 01 1"],
    # control: overshoot does not set the flag, same-hour re-request grants 0
    ["C b 240000", "S b 00 20000", "S b 00 5"],
    # control: exact-value request in a NEW hour is a plain grant
    ["C d 240000", "S d 00 4000", "S d 01 16000", "S d 02 1"],
)
add(
    "f2p_one_digit_hours",
    ["C a 240000", "S a 5 3000", "S a 06 100"],
    ["C a 240000", "S a 0 100", "S a 1 100"],
    ["C a 120000", "S a 9 45000", "U a"],
    ["C a 240000", "S a 00 100"],  # control: 2-digit hours in both
)
add(
    "f2p_case_fold",
    ["C Abc 240000", "C ABC 120000"],  # DUP vs OK 2
    ["C Abc 240000", "S abc 00 500", "U aBC"],  # folded spend hits the campaign
    ["C ad1 240000", "S AD1 00 500", "U aD1"],
    ["C Abc 240000", "C abd 120000", "S ABD 06 100"],
    ["C abc 240000", "S abc 00 500"],  # control: same case in both
)
add(
    "f2p_lenient_tokens",
    ["C a 240000 extra"],
    ["C a 240000", "S a 00 100 tail junk"],
    ["C a 240000", "U a now please"],
    ["C a 240000", "S a 00 100"],  # control: exact tokens, both OK
)
add(
    "f2p_multiday_carry",
    # back-loaded day inflates carry, which inflates day 2's live slice,
    # while day 2's audit still runs on daily/24 flat
    ["C a 240000", "S a 18 200000", "U a",
     "S a 00 12000", "S a 12 90000", "U a", "S a 00 1000"],
    # three-day chain alternating back-loaded and idle days
    ["C b 120000", "S b 22 100000", "U b", "U b", "S b 00 6000", "S b 10 6000", "U b"],
    # damper campaign across a true-up
    ["C big 2400000", "S big 23 2000000", "U big", "S big 00 100000", "U big"],
    # control: two even days, everything matches
    ["C c 240000"] + [f"S c {h:02d} 10000" for h in range(20)] + ["U c"]
    + [f"S c {h:02d} 9000" for h in range(22)] + ["U c"],
)

# f2p_parity: held-out mixed corpora; any missed quirk fails them
CAMPS = ["a", "ad1", "AD1", "Ad1", "big", "BIG", "camp8x", "zz99", "ghost", "bad-id"]
BUDGETS = ["1000", "50000", "120000", "240000", "999999", "1000000",
           "2400000", "999", "1234567890", "12x4"]
HOURS_T = [f"{h:02d}" for h in range(24)] + ["0", "5", "9", "23", "24", "005", "2x"]
REQS = ["1", "500", "3000", "9999", "41666", "90000", "200000", "9999999",
        "0", "12345678", "12a"]


def parity_batch(n: int) -> list[str]:
    batch = []
    n_true = 0
    for i in range(n):
        roll = rng.random()
        if roll < 0.12:
            cmd = f"C {rng.choice(CAMPS)} {rng.choice(BUDGETS)}"
            if rng.random() < 0.15:
                cmd += " extra"
        elif roll < 0.70:
            cmd = f"S {rng.choice(CAMPS)} {rng.choice(HOURS_T)} {rng.choice(REQS)}"
            if rng.random() < 0.15:
                cmd += " tail junk"
        elif roll < 0.90:
            cmd = f"U {rng.choice(CAMPS)}"
            if rng.random() < 0.15:
                cmd += " now"
            n_true += 1
        else:
            cmd = rng.choice(["Q hey", "X", "c a 1000", "S", "U", "C", "S a"])
        batch.append(cmd)
    assert n_true >= 4, "parity batch must exercise multiple true-ups"
    return batch


add("f2p_parity", parity_batch(160), parity_batch(155))

# ---------------------------------------------------------------- generate

fixtures: dict[str, list[dict]] = {}
gold_mismatch = naive_agree_f2p = 0
divergence_counts: dict[str, int] = {}
for family, batches in families.items():
    fixtures[family] = []
    family_diverges = 0
    for batch in batches:
        expected = run_binary(batch)
        gold_out = run_py(gold, batch)
        if gold_out != expected:
            gold_mismatch += 1
            for got, want in zip(gold_out, expected):
                if got != want:
                    print(f"GOLD MISMATCH [{family}]\n  bin: {want}\n  gold: {got}")
                    break
            else:
                print(f"GOLD MISMATCH [{family}] length {len(gold_out)} vs {len(expected)}")
        naive_out = run_py(naive, batch)
        if naive_out != expected:
            family_diverges += 1
        fixtures[family].append({"input": batch, "expected": expected})
    divergence_counts[family] = family_diverges
    if family.startswith("f2p") and not family_diverges:
        naive_agree_f2p += 1
        print(f"WARNING: naive base PASSES every batch in {family}")
    if family.startswith("guard"):
        for entry, batch in zip(fixtures[family], batches):
            if run_py(naive, batch) != entry["expected"]:
                print(f"GUARD BROKEN AT BASE [{family}]: {batch[:2]}")
                gold_mismatch += 1

# big random parity sweep (not shipped): gold must match binary everywhere
sweep_fail = 0
for i in range(40):
    batch = []
    for _ in range(rng.randrange(1, 50)):
        roll = rng.random()
        if roll < 0.14:
            batch.append(f"C {rng.choice(CAMPS)} {rng.choice(BUDGETS)}"
                         + rng.choice(["", "", " t"]))
        elif roll < 0.68:
            batch.append(f"S {rng.choice(CAMPS)} {rng.choice(HOURS_T)} {rng.choice(REQS)}"
                         + rng.choice(["", "", " j k"]))
        elif roll < 0.90:
            batch.append(f"U {rng.choice(CAMPS)}")
        else:
            batch.append(rng.choice(["Q x", "", "U", "S s", "u a", "C x"]))
    expected = run_binary(batch)
    if run_py(gold, batch) != expected:
        sweep_fail += 1
        print(f"SWEEP MISMATCH batch {i}")

print("naive divergence per family:")
for family, count in divergence_counts.items():
    print(f"  {family}: {count}/{len(families[family])} batches diverge")

problems = gold_mismatch + naive_agree_f2p + sweep_fail
print(f"\nfamilies: {len(fixtures)}, gold mismatches: {gold_mismatch}, "
      f"f2p families naive passes: {naive_agree_f2p}, sweep mismatches: {sweep_fail}")
print(f"problems: {problems}")
if problems:
    sys.exit(1)

out = ROOT / "tests" / "fixtures.json"
out.write_text(json.dumps(fixtures, indent=1) + "\n", encoding="utf-8")
print(f"wrote {out} ({sum(len(v) for v in fixtures.values())} batches)")
