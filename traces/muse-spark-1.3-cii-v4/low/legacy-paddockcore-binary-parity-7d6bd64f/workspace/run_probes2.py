"""Targeted probes batch 2 (redesigned)."""
import os
import subprocess

TMP = os.environ["TMPDIR"]
OUT = os.path.join(TMP, "probes2.txt")

cases = {
    # affinity-last vs (g,pseq): 3 shifts; a(aff,g3,pseq0) vs x(plain,g3,pseq1)
    "afflast2": ["N a 10", "N x 10", "N p 10", "F f1 9", "F f2 9",
                 "F f9 9",
                 "G a f1", "H a", "G a f1", "H a", "G a f1",
                 "G x f2", "H x", "G x f9", "H x", "G x f2",
                 "G p f9", "S", "S", "S", "V"],
    # U barns: a(g1,early,BARNED) vs c(g2,late,BARNED) vs m(H'd,mid)
    #   + w(relocated-then-H'd: reloc1,barned0)
    "ubarn2": ["N a 10", "N m 10", "N c 10", "N w 10", "N h 10",
               "F f1 9", "F f2 1", "F f3 1",
               "G a f1", "G c f1", "H c", "G c f1",
               "G w f2", "G m f1", "G h f3",
               "S", "H w", "H m", "S", "U"],
    # S1 mover order with leads: y(g1,L0,pseq0) vs x(g1,L2,pseq1), S1
    "mover2": ["N y 10", "N x 10", "N z 10", "F f1 9", "F f2 9",
               "G y f2", "G x f2", "L x f1", "L x f2", "G z f1",
               "S", "V"],
    # S1 build vs g: a(g2,90) vs b(g1,10) co-movers at S1
    "s1ord2": ["N a 90", "N b 10", "N z 10", "F f1 9", "F f2 9",
               "G a f1", "H a", "G a f1", "G b f1", "G z f2",
               "S", "V"],
}

with open(OUT, "w") as fh:
    for name, cmds in cases.items():
        p = subprocess.run(["legacy/run"], input="\n".join(cmds) + "\n",
                           capture_output=True, text=True)
        fh.write(f"##### {name}\n")
        fh.write("\n".join(cmds) + "\n-----\n")
        fh.write(p.stdout)
        fh.write("\n")
print("wrote", OUT)
