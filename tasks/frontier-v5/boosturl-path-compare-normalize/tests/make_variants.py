"""Split the pin's test files into one variant per fix: base lines plus that fix's added lines.

usage: make_variants.py <repo> <pin> <outdir> <commit> [<commit> ...]
"""
import os, subprocess, sys
repo, pin, out, commits = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
full = {subprocess.check_output(["git", "-C", repo, "rev-parse", c], text=True).strip(): c for c in commits}
for rel in ("test/unit/url.cpp", "test/unit/url_base.cpp"):
    blame = subprocess.check_output(["git", "-C", repo, "blame", "--porcelain", pin, "--", rel], text=True).splitlines()
    lines, i = [], 0
    while i < len(blame):
        sha = blame[i].split()[0]; i += 1
        while not blame[i].startswith("\t"): i += 1
        lines.append((full.get(sha), blame[i][1:])); i += 1
    # Test blocks of 359816e, 5ec0b82 and c12800e add checks inside the block
    # 8b75693 opened (its `check` helper); their variants keep that block's
    # scaffolding but not its own check(...) calls.
    scaffold_users = {"359816e", "5ec0b82", "c12800e"}
    def scaffold(t):
        return not t.strip().startswith("check(")
    for c in commits:
        keep = [t for owner, t in lines if owner in (None, c)
                or (owner == "8b75693" and c in scaffold_users and scaffold(t))]
        d = os.path.join(out, c, os.path.dirname(rel)); os.makedirs(d, exist_ok=True)
        open(os.path.join(out, c, rel), "w").write("\n".join(keep) + "\n")
    base = [t for owner, t in lines if owner is None]
    d = os.path.join(out, "base", os.path.dirname(rel)); os.makedirs(d, exist_ok=True)
    open(os.path.join(out, "base", rel), "w").write("\n".join(base) + "\n")
    print(rel, "lines", len(lines), {c: sum(1 for o, _ in lines if o == c) for c in commits})
