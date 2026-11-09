#!/usr/bin/env python3
"""World for tasks/computer-use-v1/files-invoice-entry (deterministic).

    .venv/bin/python scripts/computer-use/seeds/files_invoice_entry.py

Writes, under the task's environment/invoices/: the ledger Finance/Payables.ods
(fourteen rows, via ods.py), the supplier PDFs (pdfs/incoming, eight files;
pdfs/entered, two older ones already in the ledger, via pdf.py) and the
answer key invoices.json (also copied to tests/seed-invoices.json). Five
visibly different invoice layouts; one invoice is already in the ledger, one
arrives twice under two file names, one file is a credit note. Every byte is
a pure function of this script.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ods import write_ods
from pdf import Page, write_pdf

TASK = Path(__file__).resolve().parents[3] / "tasks" / "computer-use-v1" / "files-invoice-entry"
OUT = TASK / "environment" / "invoices"
CUSTOMER = [
    "Relay Desk Ltd",
    "Accounts Payable",
    "Unit 4, Quayside Works",
    "Leith, Edinburgh EH6 7DX",
]

HEADERS = ["Invoice", "Supplier", "Date", "Amount"]
LEDGER: list[list[object]] = [
    ["BF-1988", "Brightwater Foods", "2026-01-08", 1140.20],
    ["KO-INV-00291", "Kestrel Optics", "2026-01-12", 2160.00],
    ["TM/2026/097", "Tidewater Marine", "2026-01-14", 695.75],
    ["OD-4410", "Orchard Dental", "2026-01-16", 388.00],
    ["QP-5502", "Quill Publishing", "2026-01-21", 412.50],
    ["RC-20260123", "Redfern Cycles", "2026-01-23", 1530.00],
    ["VR-77012", "Vantage Retail", "2026-01-28", 264.90],
    ["SS-0904", "Sable Studio", "2026-01-30", 2975.00],
    ["BF-2012", "Brightwater Foods", "2026-02-04", 1312.40],
    ["KO-INV-00305", "Kestrel Optics", "2026-02-09", 1845.00],
    ["OD-4437", "Orchard Dental", "2026-02-11", 402.00],
    ["TM/2026/108", "Tidewater Marine", "2026-02-13", 720.00],
    ["RC-20260217", "Redfern Cycles", "2026-02-17", 1610.00],
    ["LP-2026-031", "Linden & Park", "2026-02-20", 1975.25],
]

# The eight files in incoming and the two in entered. kind: invoice (enter and
# move), duplicate (move only; its number is entered from the other file),
# entered (already in the ledger; move only), credit-note (leave alone).
INCOMING: list[dict] = [
    {
        "file": "brightwater-foods-BF-2041.pdf",
        "kind": "invoice",
        "layout": "A",
        "number": "BF-2041",
        "supplier": "Brightwater Foods",
        "date": "2026-02-23",
        "amount": 1284.60,
        "due": "25 March 2026",
        "items": [("Cold-chain produce, week 8", 1, 964.60), ("Delivery and handling", 4, 80.00)],
    },
    {
        "file": "KO-INV-00318.pdf",
        "kind": "invoice",
        "layout": "B",
        "number": "KO-INV-00318",
        "supplier": "Kestrel Optics",
        "date": "2026-02-24",
        "amount": 2310.00,
        "due": "10 March 2026",
        "net": 1925.00,
        "vat": 385.00,
        "items": [
            ("Prescription safety lenses, batch 7", 40, 38.50),
            ("Anti-scratch coating", 40, 9.625),
        ],
    },
    {
        "file": "tidewater-marine-statement-TM-2026-114.pdf",
        "kind": "invoice",
        "layout": "C",
        "number": "TM/2026/114",
        "supplier": "Tidewater Marine",
        "date": "2026-02-19",
        "amount": 760.50,
        "due": "21 March 2026",
        "items": [("Berth 12 mooring, February", 1, 640.50), ("Shore power", 1, 120.00)],
    },
    {
        "file": "quill-publishing-QP-5587.pdf",
        "kind": "invoice",
        "layout": "A",
        "number": "QP-5587",
        "supplier": "Quill Publishing",
        "date": "2026-02-25",
        "amount": 418.00,
        "due": "27 March 2026",
        "items": [("Style guide reprint, 20 copies", 20, 18.40), ("Courier", 1, 50.00)],
    },
    {
        "file": "KO-INV-00318 (resend).pdf",
        "kind": "duplicate",
        "layout": "B",
        "number": "KO-INV-00318",
        "supplier": "Kestrel Optics",
        "date": "2026-02-24",
        "amount": 2310.00,
        "due": "10 March 2026",
        "net": 1925.00,
        "vat": 385.00,
        "resend": "COPY: re-sent 27 February 2026 at the customer's request",
        "items": [
            ("Prescription safety lenses, batch 7", 40, 38.50),
            ("Anti-scratch coating", 40, 9.625),
        ],
    },
    {
        "file": "sable-studio-SS-0931.pdf",
        "kind": "invoice",
        "layout": "D",
        "number": "SS-0931",
        "supplier": "Sable Studio",
        "date": "2026-02-26",
        "amount": 3150.00,
        "due": "12 March 2026",
        "words": "three thousand one hundred and fifty pounds",
        "items": [("Brand refresh, phase two", 1, 2800.00), ("Icon set, 24 icons", 1, 350.00)],
    },
    {
        "file": "linden-park-LP-2026-031.pdf",
        "kind": "entered",
        "layout": "E",
        "number": "LP-2026-031",
        "supplier": "Linden & Park",
        "date": "2026-02-20",
        "amount": 1975.25,
        "due": "20 March 2026",
        "items": [("Office fit-out consultancy, 12 h", 12, 145.00), ("Site visit", 1, 235.25)],
    },
    {
        "file": "brightwater-foods-CN-0199.pdf",
        "kind": "credit-note",
        "layout": "A",
        "number": "CN-0199",
        "supplier": "Brightwater Foods",
        "date": "2026-02-26",
        "amount": -312.40,
        "against": "BF-2041",
        "items": [("Returned produce, 2 crates short", 1, 312.40)],
    },
]
ENTERED: list[dict] = [
    {
        "file": "brightwater-foods-BF-1988.pdf",
        "kind": "entered",
        "layout": "A",
        "number": "BF-1988",
        "supplier": "Brightwater Foods",
        "date": "2026-01-08",
        "amount": 1140.20,
        "due": "7 February 2026",
        "items": [("Cold-chain produce, week 2", 1, 900.20), ("Delivery and handling", 3, 80.00)],
    },
    {
        "file": "KO-INV-00291.pdf",
        "kind": "entered",
        "layout": "B",
        "number": "KO-INV-00291",
        "supplier": "Kestrel Optics",
        "date": "2026-01-12",
        "amount": 2160.00,
        "due": "26 January 2026",
        "net": 1800.00,
        "vat": 360.00,
        "items": [
            ("Prescription safety lenses, batch 6", 36, 40.00),
            ("Anti-scratch coating", 36, 10.00),
        ],
    },
]
ADDRESS = {
    "Brightwater Foods": [
        "Brightwater Foods",
        "Unit 9, Granton Harbour Estate",
        "Edinburgh EH5 1HF",
    ],
    "Kestrel Optics": ["Kestrel Optics Ltd", "14 Harbour Row, Leith", "Edinburgh EH6 6QT"],
    "Tidewater Marine": ["Tidewater Marine", "The Boatyard, Newhaven Place", "Edinburgh EH6 4LX"],
    "Quill Publishing": ["Quill Publishing", "27 Constitution Street", "Edinburgh EH6 7BS"],
    "Sable Studio": ["Sable Studio", "Studio 3, Coburg House", "Edinburgh EH6 6ET"],
    "Linden & Park": ["Linden & Park", "41 Bernard Street", "Edinburgh EH6 6SH"],
}
MONTHS = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
]


def long_date(iso: str) -> str:
    y, m, d = iso.split("-")
    return f"{int(d)} {MONTHS[int(m) - 1]} {y}"


def money(v: float) -> str:
    return f"{v:,.2f}"


def items_table(p: Page, y: float, inv: dict, with_vat: bool = False) -> float:
    rows = [[d, str(q), money(u), money(q * u)] for d, q, u in inv["items"]]
    return p.table(
        72,
        y,
        [
            ("Description", 251, "left"),
            ("Qty", 50, "right"),
            ("Unit", 75, "right"),
            ("Amount", 75, "right"),
        ],
        rows,
    )


def layout_a(inv: dict) -> Page:
    """Brightwater: bold name top left, document type top right, totals bottom right."""
    p = Page()
    credit = inv["kind"] == "credit-note"
    addr = ADDRESS[inv["supplier"]]
    p.text(72, 780, addr[0], size=20, bold=True)
    p.lines(72, 762, addr[1:], size=9, leading=12)
    p.text(523, 780, "CREDIT NOTE" if credit else "INVOICE", size=15, bold=True, align="right")
    p.rule(72, 738, 523, 738, width=1.2)
    p.lines(72, 715, ["Bill to:", *CUSTOMER], size=10)
    label = "Credit note No." if credit else "Invoice No."
    right = [f"{label}  {inv['number']}", f"Date  {long_date(inv['date'])}"]
    right.append(f"Against invoice  {inv['against']}" if credit else f"Payment due  {inv['due']}")
    p.lines(372, 715, right, size=10)
    y = items_table(p, 630, inv)
    total = abs(inv["amount"])
    p.text(400, y - 26, "Credit" if credit else "Total due", size=12, bold=True)
    p.text(523, y - 26, money(total), size=12, bold=True, align="right")
    p.rule(392, y - 34, 523, y - 34)
    if credit:
        p.text(
            72,
            y - 60,
            f"This credit note reduces the balance of invoice {inv['against']}. No payment is due on it.",
            size=9,
        )
    else:
        p.text(
            72,
            y - 60,
            "Bank transfer to the account on file. Please quote the invoice number.",
            size=9,
        )
    p.text(
        72,
        90,
        f"{addr[0]}  |  VAT GB 441 2210 07  |  accounts@{addr[0].split()[0].lower()}foods.example",
        size=8,
    )
    return p


def layout_b(inv: dict) -> Page:
    """Kestrel: tax invoice with a line-item table and a net, VAT, total block."""
    p = Page()
    addr = ADDRESS[inv["supplier"]]
    p.text(72, 780, addr[0], size=20, bold=True)
    p.text(72, 762, ", ".join(addr[1:]), size=9)
    p.text(523, 780, "TAX INVOICE", size=14, bold=True, align="right")
    if inv.get("resend"):
        p.text(523, 762, inv["resend"], size=9, bold=True, align="right")
    p.rule(72, 750, 523, 750)
    p.lines(72, 725, ["Invoice to:", *CUSTOMER], size=10)
    p.lines(
        340,
        725,
        [
            f"Invoice number:  {inv['number']}",
            f"Invoice date:  {long_date(inv['date'])}",
            "Payment terms:  14 days",
            f"Due date:  {inv['due']}",
        ],
        size=10,
    )
    y = items_table(p, 630, inv)
    p.text(400, y - 20, "Net total", size=10)
    p.text(523, y - 20, money(inv["net"]), size=10, align="right")
    p.text(400, y - 36, "VAT 20%", size=10)
    p.text(523, y - 36, money(inv["vat"]), size=10, align="right")
    p.rule(392, y - 44, 523, y - 44)
    p.text(400, y - 60, "Total due", size=11, bold=True)
    p.text(523, y - 60, money(inv["amount"]), size=11, bold=True, align="right")
    p.text(
        72,
        110,
        "Please quote the invoice number with your payment. Thank you for your business.",
        size=9,
    )
    p.text(
        72,
        96,
        "Kestrel Optics Ltd is registered in Scotland, SC 402 118. VAT GB 712 9930 41.",
        size=8,
    )
    return p


def layout_c(inv: dict) -> Page:
    """Tidewater: centred header, a 'Reference' label instead of an invoice number."""
    p = Page()
    addr = ADDRESS[inv["supplier"]]
    p.text(297, 785, addr[0], size=22, bold=True, align="center")
    p.text(297, 768, ", ".join(addr[1:]), size=9, align="center")
    p.rule(72, 755, 523, 755, width=0.8)
    p.text(297, 735, "Invoice", size=13, bold=True, align="center")
    p.box(72, 650, 451, 70, fill=0.95)
    p.lines(
        84,
        702,
        [
            f"Reference:  {inv['number']}",
            f"Issued:  {long_date(inv['date'])}",
            f"Payment due by:  {inv['due']}",
        ],
        size=10,
        leading=16,
    )
    p.lines(330, 702, ["To:", *CUSTOMER[:3]], size=10, leading=16)
    y = items_table(p, 620, inv)
    p.text(72, y - 30, "Amount payable", size=12, bold=True)
    p.text(523, y - 30, money(inv["amount"]), size=12, bold=True, align="right")
    p.text(
        72,
        y - 56,
        "Mooring charges are invoiced monthly in arrears. Queries to the harbour office.",
        size=9,
    )
    return p


def layout_d(inv: dict) -> Page:
    """Sable: sparse studio layout, amount in words."""
    p = Page()
    addr = ADDRESS[inv["supplier"]]
    p.text(72, 770, addr[0].upper(), size=26, bold=True)
    p.text(72, 752, ", ".join(addr[1:]), size=9)
    p.text(72, 700, f"Invoice {inv['number']}", size=14, bold=True)
    p.text(72, 682, long_date(inv["date"]), size=11)
    p.lines(72, 650, ["For", *CUSTOMER[:2]], size=10)
    y = 590
    for d, q, u in inv["items"]:
        p.text(72, y, d, size=11)
        p.text(523, y, money(q * u), size=11, align="right")
        y -= 20
    p.rule(72, y + 6, 523, y + 6)
    p.text(72, y - 18, "Balance due", size=13, bold=True)
    p.text(523, y - 18, money(inv["amount"]), size=13, bold=True, align="right")
    p.text(72, y - 38, f"({inv['words']})", size=9)
    p.text(72, y - 66, f"Payable by {inv['due']}. Thank you.", size=10)
    return p


def layout_e(inv: dict) -> Page:
    """Linden & Park: boxed header with the figures in a two-column grid."""
    p = Page()
    addr = ADDRESS[inv["supplier"]]
    p.box(72, 700, 451, 92, width=1.0)
    p.text(84, 768, addr[0], size=18, bold=True)
    p.lines(84, 748, addr[1:], size=9, leading=12)
    p.text(511, 768, "INVOICE", size=12, bold=True, align="right")
    grid = [
        ("Invoice number", inv["number"]),
        ("Date of invoice", long_date(inv["date"])),
        ("Terms", "30 days"),
        ("Due", inv["due"]),
    ]
    y = 672
    for k, v in grid:
        p.text(72, y, k, size=10, bold=True)
        p.text(200, y, v, size=10)
        y -= 17
    p.lines(340, 672, ["Client", *CUSTOMER[:3]], size=10, leading=17)
    y = items_table(p, 580, inv)
    p.text(400, y - 24, "Total", size=12, bold=True)
    p.text(523, y - 24, money(inv["amount"]), size=12, bold=True, align="right")
    p.text(72, 100, "Linden & Park LLP. Partnership number SO 307 441.", size=8)
    return p


LAYOUTS = {"A": layout_a, "B": layout_b, "C": layout_c, "D": layout_d, "E": layout_e}


def render(inv: dict, out_dir: Path) -> Path:
    page = LAYOUTS[inv["layout"]](inv)
    kind = "Credit note" if inv["kind"] == "credit-note" else "Invoice"
    return write_pdf(out_dir / inv["file"], [page], title=f"{kind} {inv['number']}")


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    amounts = [r[3] for r in LEDGER] + [i["amount"] for i in INCOMING if i["kind"] == "invoice"]
    assert len(set(amounts)) == len(amounts), "amounts must be distinct"
    assert LEDGER[-1][0] == "LP-2026-031"
    pdf_in, pdf_done = OUT / "pdfs" / "incoming", OUT / "pdfs" / "entered"
    for d in (pdf_in, pdf_done):
        d.mkdir(parents=True, exist_ok=True)
        for old in d.glob("*.pdf"):
            old.unlink()
    files = []
    for inv in INCOMING:
        files.append(
            {
                "folder": "incoming",
                **{
                    k: v
                    for k, v in inv.items()
                    if k in ("file", "kind", "number", "supplier", "date", "amount")
                },
                "sha256": sha(render(inv, pdf_in)),
            }
        )
    for inv in ENTERED:
        files.append(
            {
                "folder": "entered",
                **{
                    k: v
                    for k, v in inv.items()
                    if k in ("file", "kind", "number", "supplier", "date", "amount")
                },
                "sha256": sha(render(inv, pdf_done)),
            }
        )
    write_ods(
        OUT / "Payables.ods",
        [{"name": "Payables", "widths": ["co2", "co3", "co2", "co2"], "rows": [HEADERS, *LEDGER]}],
    )
    expected_new = [
        [i["number"], i["supplier"], i["date"], i["amount"]]
        for i in INCOMING
        if i["kind"] == "invoice"
    ]
    key = {
        "ledger_file": "Payables.ods",
        "sheet": "Payables",
        "headers": HEADERS,
        "ledger_rows": LEDGER,
        "expected_new_rows": expected_new,
        "files": files,
        "credit_notes": [f["file"] for f in files if f["kind"] == "credit-note"],
        "to_move": sorted(
            f["file"] for f in files if f["folder"] == "incoming" and f["kind"] != "credit-note"
        ),
        "already_entered": sorted(f["file"] for f in files if f["folder"] == "entered"),
    }
    text = json.dumps(key, indent=1, ensure_ascii=False) + "\n"
    (OUT / "invoices.json").write_text(text)
    (TASK / "tests").mkdir(parents=True, exist_ok=True)
    (TASK / "tests" / "seed-invoices.json").write_text(text)
    print(
        f"wrote {len(files)} PDFs, Payables.ods ({len(LEDGER)} rows), invoices.json; "
        f"{len(expected_new)} rows to enter, {len(key['to_move'])} files to move"
    )


if __name__ == "__main__":
    main()
