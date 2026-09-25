"""Targeted probes for remaining distinctions."""
import os
import subprocess

TMP = os.environ["TMPDIR"]
OUT = os.path.join(TMP, "probes.txt")

cases = {
    # affinity-last vs (g,pseq): a(aff,g3,pseq-early) vs x(plain,g3,pseq-late)
    "afflast": ["N a 10", "N x 10", "N p 10", "F f1 9", "F f2 9", "F f3 9",
                "G a f1", "H a", "G a f1", "H a", "G a f1",
                "G x f2", "H x", "G x f3", "H x", "G x f1",
                "G p f2", "S", "S", "V"],
    # freshness: q(plain,g1) vs p(fresh post-S re-G,g2)
    "fresh": ["N q 10", "N p 10", "F f1 9", "F f2 9", "F f3 9",
              "G q f1", "G p f2", "H p", "G p f2", "S",
              "H p", "G p f2", "V"],
    # led count: x(l1,g1) vs y(l2,g1), pseq x first
    "ledcount": ["N x 10", "N y 10", "F f1 9", "F f2 9", "F f3 9",
                 "G x f1", "L x f2", "G y f1", "L y f2", "L y f3",
                 "V"],
    # led vs build: weak(led,build5) vs strong(unled,build90)
    "ledbuild": ["N w 5", "N s 90", "F f1 9", "F f2 9",
                 "G w f1", "L w f2", "G s f1", "V"],
    # U barn order: shift-BARN late-pseq vs H early-pseq
    "ubarn": ["N h 10", "N m 10", "N z 10", "F f1 9", "F f2 1", "F f3 1",
              "G m f1", "G z f1", "G h f2", "H h", "S", "U"],
    # mover order: y(first-enrolled,l0,g1) vs x(late,l1,g1) co-movers
    "mover": ["N y 10", "N x 10", "F f1 9", "F f2 9", "F f3 9",
              "G y f2", "G x f2", "L x f1", "L x f2", "S", "V"],
    # S1 order: a(g2,build90) vs b(g1,build10) co-movers at S1
    "s1ord": ["N a 90", "N b 10", "F f1 9", "F f2 9",
              "G a f1", "H a", "G a f1", "G b f1", "S", "V"],
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
