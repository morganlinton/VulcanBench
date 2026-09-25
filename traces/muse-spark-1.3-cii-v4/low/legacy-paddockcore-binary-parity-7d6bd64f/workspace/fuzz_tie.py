"""Forced-tie fuzz: all builds 10, each pony G'd exactly once (g=1 all).
V-order then exposes the pure tie-break."""
import random
import subprocess
import sys

def gen(rng):
    names = ["a", "b", "c", "d"][:rng.randint(2, 4)]
    flds = ["f1", "f2", "f3"][:rng.randint(2, 3)]
    cmds = []
    for p in names:
        cmds.append(f"N {p} 10")
    for f in flds:
        cmds.append(f"F {f} {rng.choice([1, 2, 3, 9])}")
    for p in names:
        cmds.append(f"G {p} {rng.choice(flds)}")
    for _ in range(rng.randint(1, 2)):
        cmds.append("S")
    cmds.append("V")
    for f in flds:
        cmds.append(f"R {f}")
    return cmds

def main():
    seed = int(sys.argv[1]) if len(sys.argv) > 1 else 2
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 40
    rng = random.Random(seed)
    for i in range(n):
        cmds = gen(rng)
        p = subprocess.run(["legacy/run"], input="\n".join(cmds) + "\n",
                           capture_output=True, text=True)
        print(f"### CMDS {i}")
        print("\n".join(cmds))
        print(f"### OUT {i}")
        print(p.stdout, end="")

if __name__ == "__main__":
    main()
