"""Final fit: V = (-build,-ledflag,-fresh,+g,+ever,+pseq)."""
import os
import sys

sys.path.insert(0, ".")
from fuzz_fit import parse
from scan_h import replay2
import subprocess

TMP = os.environ["TMPDIR"]
VOBS = []
for path in (f"{TMP}/fz11.txt", f"{TMP}/ft2.txt"):
    for cmds, outs in parse(path):
        try:
            for n, c in replay2(cmds, outs):
                VOBS.append((cmds, n, c))
        except AssertionError:
            pass
for h in ["wc2", "ex2", "pp", "gs", "ws", "ses6", "gl", "la", "ph",
          "l2", "av", "av2", "rf", "rf2", "ma2", "am", "ub2", "ws",
          "ses", "ses2", "ses3", "ses4", "ses5", "ses6"]:
    try:
        with open(f"{TMP}/{h}.txt") as fh:
            cmds = fh.read().splitlines()
    except OSError:
        continue
    p = subprocess.run(["legacy/run"], input="\n".join(cmds) + "\n",
                       capture_output=True, text=True)
    try:
        for n, c in replay2(cmds, p.stdout.splitlines()):
            VOBS.append((cmds, n, c))
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
        return (-builds[p], -int(a["led"]), -int(a["fresh"]), a["g"],
                int(a["ev"]), a["pseq"])
    if sorted(names, key=key) != names:
        bad += 1
        print("BAD:", names, "got", sorted(names, key=key))
        print("   ", {p: (builds[p], ctx[p]) for p in names})
        print("   ", cmds)
print(f"V obs={len(VOBS)} bad={bad}")
