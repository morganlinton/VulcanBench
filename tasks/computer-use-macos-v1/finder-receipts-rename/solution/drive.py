"""finder-receipts-rename gold: Terminal renames and moves the ten receipts
(the gold knows what each says; an agent has to open each one). Renaming in
Terminal leaves the same files as renaming in Finder.

Layout facts (1280x800 points): Terminal in the Dock (goldlib); Terminal's
first access to ~/Documents raises a privacy prompt whose Allow button is at
ALLOW. Cheat variants: VB_UK=1 dates the London folio 2026-04-03;
VB_NO_PERSONAL=1 leaves the Visa receipts in Receipts; VB_DEL_POLICY=1
also deletes the expense policy.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

ALLOW = (698, 290)  # folder privacy prompt
PERSONAL = "" if os.environ.get("VB_NO_PERSONAL") == "1" else "Personal/"
HILTON = "2026-04-03" if os.environ.get("VB_UK") == "1" else "2026-03-04"
MOVES = [
    ("Scan.pdf", "2026-03-02 Office Depot.pdf"),
    ("Scan 1.pdf", f"{PERSONAL}2026-02-27 Whole Foods Market.pdf"),
    ("Scan 2.pdf", "2026-02-18 Delta Air Lines.pdf"),
    ("Scan 3.pdf", f"{HILTON} Hilton Garden Inn.pdf"),
    ("Scan 4.pdf", "2026-03-05 Uber.pdf"),
    ("Scan 5.pdf", "2026-02-21 Shell.pdf"),
    ("Scan 6.pdf", f"{PERSONAL}2026-03-06 CVS Pharmacy.pdf"),
    ("Scan 7.pdf", "2026-02-24 Apple Store.pdf"),
    ("Scan 8.pdf", "2026-03-03 Blue Bottle Coffee.pdf"),
    ("Scan 9.pdf", f"{PERSONAL}2026-03-06 Staples.pdf"),
]

G.start()
G.open_terminal()
G.run("cd ~/Documents/Receipts && ls", wait=3)
G.allow_folder("documents-prompt")
G.run("ls", wait=2)
for src, dst in MOVES:
    G.run(f'mv -n "{src}" "{dst}"', wait=0.8)
if os.environ.get("VB_DEL_POLICY") == "1":
    G.run('rm "Expense policy.pdf"', wait=1)
G.run("ls -R", wait=2)
G.shot("final")
time.sleep(1)
print("gold done")
