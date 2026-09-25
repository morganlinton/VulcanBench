"""Scan all V-obs for cross-field same-g same-build ties; report orders."""
import os
import pickle
import sys

sys.path.insert(0, ".")
from fuzz_fit import parse

TMP = os.environ["TMPDIR"]
from tieutil import replay_fields

cases = []
for path in (f"{TMP}/fz11.txt", f"{TMP}/ft2.txt"):
    for i, (cmds, outs) in enumerate(parse(path)):
        try:
            ob = replay_fields(cmds, outs)
        except AssertionError:
            continue
        for names, ctx in ob:
            # group adjacent same-(build? we lack build here; use g only) g runs
            # need build: re-derive from cmds N lines
            builds = {}
            for c in cmds:
                t = c.split()
                if t[0] == "N":
                    builds[t[1]] = int(t[2])
            j = 0
            while j < len(names):
                k = j
                while (k + 1 < len(names)
                       and builds[names[k + 1]] == builds[names[j]]
                       and ctx[names[k + 1]]["g"] == ctx[names[j]]["g"]
                       and ctx[names[k + 1]]["l"] == 0 and ctx[names[j]]["l"] == 0):
                    k += 1
                if k > j:
                    grp = names[j:k + 1]
                    flds = {ctx[p]["fld"] for p in grp}
                    if len(flds) > 1:
                        pseqs = [ctx[p]["pseq"] for p in grp]
                        asc = all(a < b for a, b in zip(pseqs, pseqs[1:]))
                        cases.append((f"{path}#{i}", grp,
                                      [(p, ctx[p]["fld"], ctx[p]["pseq"],
                                        ctx[p]["reloc"], ctx[p]["ev"])
                                       for p in grp], asc))
                j = k + 1
print(f"cross-field ties: {len(cases)}")
nasc = sum(1 for c in cases if c[3])
print(f"pseq-ASC: {nasc}, non-ASC: {len(cases) - nasc}")
for tag, grp, det, asc in cases:
    print(("ASC  " if asc else "NONASC"), tag, grp, det)
