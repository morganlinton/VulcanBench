#!/usr/bin/env python3
"""Build-time fixture generator for legacy-replaycore-binary-parity.

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
BINARY = ROOT / "builder" / "replaycore-darwin-arm64"


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gold = _load("gold_replaycore", ROOT / "builder" / "gold_replaycore.py")
naive = _load("naive_replaycore", ROOT / "repo" / "replaycore.py")


def E(acct, delta, seq) -> str:
    return f"E {acct} {delta} {seq}"


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

rng = random.Random(20260828)
families: dict[str, list[list[str]]] = {}


def add(family: str, *batches: list[str]) -> None:
    families.setdefault(family, []).extend(batches)


# guards: nominal behavior where the written spec is accurate.
# Constraints for a guard flow: single spelling per account, nonzero
# deltas, no duplicate seqs, no extra tokens, and no Y after any FLOOR
# reject (with no logged clamps, replay really is a no-op in both).
add(
    "guard_nominal",
    [E("led1", "+100", "1"), E("led1", "-40", "2"), E("led1", "+7", "3")],
    [E("a", "+1", "1"), E("ZZ99ZZ99", "+99999999", "999999"), E("a", "-1", "2")],
    [E("b7", "+500", "10"), E("b7", "-500", "20"), E("b7", "+3", "21")],
    [E("acct01", "+05", "007"), E("acct01", "+5", "8")],  # leading zeros, distinct seqs
    [E("m1", "+10", "1"), E("m2", "+20", "1"), E("m1", "-10", "2"), E("m2", "+1", "3")],
)
add(
    "guard_replay_noop",
    ["Y"],  # empty log
    [E("led1", "+100", "1"), "Y", E("led1", "-40", "2"), "Y"],
    [E("m1", "+10", "1"), E("m2", "+20", "1"), "Y", E("m1", "-5", "2"), "Y", "Y"],
    [E("q", "+0000001", "1"), E("q", "-1", "2"), "Y"],  # balance back to 0, still listed
)
add(
    "guard_rejects",
    ["E"],
    [E("toolong99x", "+5", "1")],  # 10 chars: FMT
    [E("AC-1", "+5", "1")],
    ["E led1 +5"],  # missing seq: FMT, acct echoed
    ["Q what is this"],
    ["YIELD"],
    [E("led1", "5", "1")],  # missing sign: DELTA
    [E("led1", "+123456789", "1")],  # 9 digits: DELTA
    [E("led1", "++5", "1")],
    [E("led1", "+", "1")],
    [E("led1", "+5", "1234567")],  # 7 digits: SEQ
    [E("led1", "+5", "12x")],
    [E("led1", "+5", "-1")],
    [E("led1", "+5", "5"), E("led1", "+9", "3")],  # lower seq: ORDER both
    [E("led1", "+5", "5"), E("led1", "+9", "5")],  # same seq, different delta: ORDER both
    [E("led1", "+5", "5"), E("led1", "-9", "6")],  # FLOOR, no Y afterwards
    [E("led1", "-1", "1")],  # first event negative: FLOOR, no Y afterwards
)
add("guard_empty_batch", [])
add(
    "guard_trailer",
    [
        E("led1", "+100", "1"),
        E("led2", "+50", "1"),
        "Y",
        E("led1", "-30", "2"),
        E("led1", "+5", "1"),  # lower seq: ORDER
        E("BAD!", "+1", "1"),  # FMT
        E("led2", "9", "2"),  # DELTA
        "Y",
        "? junk",
    ],
)

# f2p families: engine behavior the written spec gets wrong or omits.

add(
    "f2p_floor_ghost",  # Q1: FLOOR rejects are logged as clamps
    [E("led1", "+100", "1"), E("led1", "-150", "2"), "Y"],
    [E("led1", "+100", "1"), E("led1", "-150", "2"), E("led1", "+7", "3"), "Y"],
    [E("led1", "+100", "1"), E("led1", "-100", "2"), "Y"],  # control: exact drain accepted, no clamp
    [E("led1", "-5", "1"), "Y"],  # first event FLOOR: zero-delta clamp entry
    [E("led1", "+10", "1"), E("led1", "-11", "2"), E("led1", "-11", "3"), "Y"],  # two clamps
    [E("led1", "+100", "1"), "Y"],  # control: no rejects, replay no-op in both
)
add(
    "f2p_saturating_replay",  # Q3: clamp entry then accepted debit saturates on replay
    [E("led1", "+10", "1"), E("led1", "-15", "2"), E("led1", "-8", "3"), "Y"],
    [E("led1", "+10", "1"), E("led1", "-15", "2"), E("led1", "-8", "3"), E("led1", "+4", "4"), "Y"],
    [E("s", "+20", "1"), E("s", "-30", "2"), E("s", "-19", "3"), E("s", "-1", "4"), "Y"],
)
add(
    "f2p_post_replay_state",  # replay REPLACES live state with diverged balances
    [E("led1", "+100", "1"), E("led1", "-150", "2"), "Y", E("led1", "-60", "3")],
    [E("led1", "+100", "1"), E("led1", "-150", "2"), "Y", E("led1", "-60", "3"), "Y"],
    [E("led1", "+100", "1"), E("led1", "-150", "2"), "Y", E("led1", "+1", "3"), "Y"],
    [E("led1", "+9", "1"), "Y", E("led1", "-9", "2")],  # control: no rejects before Y
)
add(
    "f2p_idempotent_redelivery",  # Q2: exact duplicate of last accepted event acked
    [E("led1", "+100", "5"), E("led1", "+100", "5")],
    [E("led1", "+100", "5"), E("led1", "+100", "5"), E("led1", "+1", "6")],
    [E("led1", "+100", "5"), E("led1", "+99", "5")],  # control: delta differs, ORDER both
    [E("led1", "+100", "5"), E("led1", "+100", "4")],  # control: lower seq, ORDER both
    [E("led1", "+5", "5"), E("led1", "+05", "5")],  # value-equal token spelling
    [E("led1", "+100", "5"), E("led1", "+1", "6"), E("led1", "+100", "5")],  # stale dup: ORDER both
    [E("led1", "+100", "5"), E("led1", "+100", "5"), "Y"],  # dup not logged twice
    [E("led1", "-0", "1"), E("led1", "+0", "1")],  # zero dup, value-equal across signs
)
add(
    "f2p_zero_delta",  # Q5: +0 / -0 accepted, logged, no-op
    [E("led1", "+0", "1")],
    [E("led1", "-0", "1")],
    [E("led1", "+0", "1"), E("led1", "+7", "2"), "Y"],
    [E("z", "+0", "1"), E("a", "+5", "1"), "Y"],  # zero creates the first log entry
    [E("led1", "+0", "1x")],  # engine SEQ vs naive DELTA
    [E("led1", "+1", "1")],  # control: nonzero accepted in both
)
add(
    "f2p_case_folding",  # Q6: case-insensitive accounts, first-seen spelling
    [E("Led1", "+100", "1"), E("LED1", "+50", "2")],
    [E("Led1", "+100", "1"), E("LED1", "+50", "2"), "Y"],
    [E("led1", "+100", "1"), E("LED1", "-150", "2"), "Y"],  # folded FLOOR ghost
    [E("Led1", "+100", "5"), E("lEd1", "+100", "5")],  # folded idempotent dup
    [E("AcctX", "+5", "1"), E("acctx", "+5", "1"), E("ACCTX", "+2", "2"), "Y"],
    [E("led1", "+100", "1"), E("led1", "+50", "2")],  # control: single spelling, both agree
)
add(
    "f2p_replay_ordering",  # Q4: Y order is first-log-append order incl. clamps
    [E("ghost", "-5", "1"), E("led1", "+10", "1"), "Y"],
    [E("a1", "+10", "1"), E("b2", "-3", "1"), E("c3", "+7", "1"), E("b2", "+4", "2"), "Y"],
    [E("a1", "+10", "1"), E("b2", "-3", "1"), E("c3", "+7", "1"), "Y", E("b2", "+4", "2"), "Y"],
    [E("a1", "+10", "1"), E("b2", "+3", "1"), E("c3", "+7", "1"), "Y"],  # control: same order
)
add(
    "f2p_lenient_tokens",  # Q7: trailing tokens tolerated on E and Y
    [E("led1", "+5", "1") + " IGNORED"],
    [E("led1", "+5", "1") + " a b c", E("led1", "+5", "2") + " tail"],
    [E("led1", "+5", "1"), "Y now please"],
    [E("led1", "+5", "1"), "Y "],
    [E("led1", "+5", "1")],  # control: both accept
)

# f2p_parity: held-out mixed corpora; any missed quirk fails them.


def parity_batch(n: int) -> list[str]:
    accts = ["led1", "LED1", "Led1", "aud7", "AUD7", "x", "ZZ99ZZ99",
             "ghost", "Ghost", "toolong99", "AC-1"]
    deltas = ["+5", "-5", "+05", "+0", "-0", "+100", "-150", "+99999999",
              "-99999999", "5", "+123456789", "++1", "+"]
    seqs = [str(s) for s in (1, 2, 3, 4, 5, 9, 10, 42, 999999)] + ["007", "1234567", "1x", "-1"]
    batch: list[str] = []
    for _ in range(n):
        roll = rng.random()
        if roll < 0.08:
            batch.append(rng.choice(["Y", "Y", "Y now", "Y "]))
        elif roll < 0.12:
            batch.append(rng.choice(["E", "E led1 +5", "Q junk", "YIELD", "E led1"]))
        else:
            cmd = E(rng.choice(accts), rng.choice(deltas), rng.choice(seqs))
            if rng.random() < 0.12:
                cmd += rng.choice([" extra", " a b", " tail tokens"])
            batch.append(cmd)
    batch.append("Y")
    return batch


add("f2p_parity", parity_batch(155), parity_batch(160))

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
                print(f"GUARD BROKEN AT BASE [{family}]: {batch[:3]}")
                gold_mismatch += 1

# big random parity sweep (not shipped): gold must match binary everywhere
sweep_fail = 0
for i in range(40):
    batch = []
    for _ in range(rng.randrange(1, 60)):
        roll = rng.random()
        if roll < 0.10:
            batch.append(rng.choice(["Y", "Y trailing", "Y  "]))
        elif roll < 0.16:
            batch.append(rng.choice(["E", "E a", "E a +5", "junk line", "X 0 0 0", ""]))
        else:
            acct = rng.choice(["a", "A", "aB3", "AB3", "ab3", "ledger9",
                               "LEDGER9", "wayTooLongAcct", "a$b", "zz"])
            sign = rng.choice(["+", "-", "", "+-"])
            mag = rng.choice(["0", "1", "5", "17", "150", "00000000",
                              "99999999", "123456789", "x9"])
            seq = rng.choice(["0", "1", "2", "3", "7", "007", "999999",
                              "1000000", "5x", ""])
            cmd = f"E {acct} {sign}{mag} {seq}".rstrip()
            if rng.random() < 0.10:
                cmd += " tail"
            batch.append(cmd)
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
