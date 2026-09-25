"""Final fit check: V = (-build,-led,+g,+ever,+pseq); U barns."""
import os
import pickle
import sys

sys.path.insert(0, ".")
from tieutil import replay_fields
from fuzz_fit import parse
import subprocess

TMP = os.environ["TMPDIR"]
VOBS = []
for path in (f"{TMP}/fz11.txt", f"{TMP}/ft2.txt"):
    for cmds, outs in parse(path):
        try:
            VOBS.extend([(cmds, n, c) for n, c in replay_fields(cmds, outs)])
        except AssertionError:
            pass
hard = ["wc2", "ex2", "pp", "gs", "ws", "ses6", "gl", "la", "ph",
        "l2", "av", "av2", "rf", "rf2", "ma2", "ses.txt", "ses2",
        "ses3", "ses4", "ses5"]
for h in hard:
    try:
        with open(f"{TMP}/{h}.txt") as fh:
            cmds = fh.read().splitlines()
    except OSError:
        continue
    p = subprocess.run(["legacy/run"], input="\n".join(cmds) + "\n",
                       capture_output=True, text=True)
    try:
        VOBS.extend([(cmds, n, c) for n, c in replay_fields(cmds, p.stdout.splitlines())])
    except AssertionError as e:
        print("skip", h, e)

bad = 0
for cmds, names, ctx in VOBS:
    builds = {}
    for c in cmds:
        t = c.split()
        if t[0] == "N":
            builds[t[1]] = int(t[2])

    def key(p, builds=builds):
        a = ctx[p]
        return (-builds[p], -(1 if a["l"] > 0 else 0), a["g"],
                (1 if a["ev"] else 0), a["pseq"])
    if sorted(names, key=key) != names:
        bad += 1
        print("BAD:", names, "got", sorted(names, key=key))
        print("   ", {p: (builds[p], ctx[p]) for p in names})
        print("   ", cmds)
print(f"V obs={len(VOBS)} bad={bad}")
