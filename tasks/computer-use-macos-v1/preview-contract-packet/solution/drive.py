"""preview-contract-packet gold: Terminal runs a short JavaScript for
Automation script with PDFKit (the framework Preview itself uses) that
copies draft pages 1, 2, 4 and 6, Exhibit A pages 1 and 2 and the signed
page into a new document saved in Documents. The originals are opened
read-only and never written.

Layout facts (1280x800 points): Terminal in the Dock (goldlib); Terminal's
first access to ~/Downloads and then ~/Documents each raise a privacy
prompt, Allow at ALLOW. Cheat variants: VB_IN_PLACE=1 also writes the
trimmed draft back over the original; VB_ORDER=sig-first puts the signed
page before Exhibit A; VB_KEEP_DUP=1 keeps the duplicated page.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import goldlib as G  # noqa: E402

ALLOW = (698, 290)  # folder privacy prompt
draft_pages = "[0,1,3,4,5]" if os.environ.get("VB_KEEP_DUP") == "1" else "[0,1,3,5]"
add_draft = f"{draft_pages}.forEach(function(i){{add(a,i)}});"
add_exhibit = "[0,1].forEach(function(i){add(x,i)});"
add_signed = "add(s,0);"
order = (
    add_draft + add_signed + add_exhibit
    if os.environ.get("VB_ORDER") == "sig-first"
    else add_draft + add_exhibit + add_signed
)
in_place = (
    'a.removePageAtIndex(6);a.removePageAtIndex(4);a.removePageAtIndex(2);a.writeToFile(D+"Acme MSA - draft 3.pdf");'
    if os.environ.get("VB_IN_PLACE") == "1"
    else ""
)
script = (
    'ObjC.import("PDFKit");var D="/Users/admin/Downloads/";'
    "function doc(n){return $.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(D+n))}"
    'var a=doc("Acme MSA - draft 3.pdf"),x=doc("Acme MSA - Exhibit A.pdf"),'
    's=doc("Acme MSA - signature page (signed).pdf");var o=$.PDFDocument.alloc.init;'
    "function add(d,i){o.insertPageAtIndex(d.pageAtIndex(i).copy,o.pageCount)}"
    + order
    + 'o.writeToFile("/Users/admin/Documents/Acme MSA - executed.pdf");'
    + in_place
    + "o.pageCount"
)

G.start()
G.open_terminal()
G.run(f"osascript -l JavaScript -e '{script}'", wait=4)
G.allow_prompt(*ALLOW, name="downloads-prompt")
time.sleep(2)
G.allow_prompt(*ALLOW, name="documents-prompt")
time.sleep(3)
G.run('ls -l ~/Documents ~/Downloads', wait=2)
G.shot("final")
print("gold done")
