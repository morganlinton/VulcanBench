"""Probes batch 3: mover3 (S2+ leads vs pseq), ubarn3 (barn key, grouping)."""
import os
import subprocess

TMP = os.environ["TMPDIR"]
OUT = os.path.join(TMP, "probes3.txt")

cases = {
    # S2+ co-movers: y(pseq0,L0,g1) vs x(pseq1,L2,g1)
    # S1 rests empty f2; S2 rests f1 [y,x]
    "mover3": ["N y 10", "N x 10", "F f1 9", "F f2 9", "F f3 9",
               "G y f1", "G x f1", "L x f2", "L x f1", "G z f3",
               "N z 10", "S", "S", "V"],
    # U: BARNED a(g1,early) vs BARNED c(g3,late); H'd m;
    #   barn big90 vs out small10 (grouping?)
    "ubarn3": ["N a 10", "N m 10", "N c 10", "N B 90", "F f1 9",
               "F f2 1", "F f3 1",
               "G a f1", "G c f1", "H c", "G c f1", "H c", "G c f1",
               "G m f1", "G B f2", "H B", "S", "U"],
}

with open(OUT, "w") as fh:
    for name, cmds in cases.items():
        if name == "mover3":
            # N z must come before G z: fix order
            cmds = ["N y 10", "N x 10", "N z 10", "F f1 9", "F f2 9",
                    "F f3 9",
                    "G y f1", "G x f1", "L x f2", "L x f1", "G z f3",
                    "S", "S", "V"]
        p = subprocess.run(["legacy/run"], input="\n".join(cmds) + "\n",
                           capture_output=True, text=True)
        fh.write(f"##### {name}\n")
        fh.write("\n".join(cmds) + "\n-----\n")
        fh.write(p.stdout)
        fh.write("\n")
print("wrote", OUT)
