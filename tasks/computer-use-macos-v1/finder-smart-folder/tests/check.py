#!/usr/bin/env python3
"""finder-smart-folder checker: ~/Library/Saved Searches and the Finder
sidebar list copied from the stopped VM.

    check.py /var/lib/vbmac

A Finder smart folder is a .savedSearch property list. Its RawQuery is the
Spotlight query (Kind is PDF writes `_kMDItemGroupId = 11`; Name contains
"invoice" writes `kMDItemDisplayName = "*invoice*"cdw`), and its search
scope is in RawQueryDict.SearchScopes. The sidebar's favorites are bookmarks
in com.apple.LSSharedFileList.FavoriteItems.sfl*, whose path components
include the saved search's file name.

Sub-metrics (reward is 1 only when all are 1):
  artifact_present   the Saved Searches folder or the sidebar list was collected
  saved              "Client invoices.savedSearch" exists
  pdf_only           the query restricts the kind to PDF
  name_invoice       the query requires "invoice" in the name, ignoring case
  documents_scope    the search covers ~/Documents (and nothing wider)
  no_extra_criteria  the query tests nothing but kind and name
  in_sidebar         the smart folder is a Finder sidebar favorite
  ui_path            the MCP server logged pointer input
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, "/opt/vbdesk")
import cu_check as cu  # noqa: E402
import mac_state as M  # noqa: E402

METRICS = (
    "artifact_present",
    "saved",
    "pdf_only",
    "name_invoice",
    "documents_scope",
    "no_extra_criteria",
    "in_sidebar",
    "ui_path",
)
NAME = "Client invoices"
SFL = f"{M.HOME}/Library/Application Support/com.apple.sharedfilelist"
DOCS = {"/Users/admin/Documents", "/Users/admin/Documents/", "~/Documents"}
KIND_ATTRS = {"_kMDItemGroupId", "kMDItemContentType", "kMDItemContentTypeTree", "kMDItemKind"}
NAME_ATTRS = {"kMDItemDisplayName", "kMDItemFSName"}
PDF_TERMS = (
    re.compile(r"_kMDItemGroupId\s*==?\s*11\b"),
    re.compile(r'kMDItemContentType(Tree)?\s*==?\s*"com\.adobe\.pdf"'),
)
NAME_TERM = re.compile(r'kMDItem(DisplayName|FSName)\s*==?\s*"\*?invoice\*?"(c|cd|cdw|dc|cw|cwd)\b', re.IGNORECASE)


def main(argv: list[str]) -> int:
    root = Path(argv[1])
    export = root / "export"
    fs = export / "fs"
    m = {k: 0 for k in METRICS}
    extra: dict[str, object] = {"shutdown": M.manifest(export).get("shutdown", "none")}
    saved_dir = fs / M.SAVED_SEARCHES
    sfl_dir = fs / SFL
    if not (saved_dir.is_dir() or sfl_dir.is_dir()):
        cu.emit(m, METRICS, **extra)
        return 0
    m["artifact_present"] = 1
    searches = M.saved_searches(fs)
    extra["saved_searches"] = sorted(searches)
    s = searches.get(NAME)
    if s is not None:
        m["saved"] = 1
        raw = str(s.get("RawQuery") or (s.get("RawQueryDict") or {}).get("RawQuery") or "")
        extra["raw_query"] = raw
        m["pdf_only"] = int(any(p.search(raw) for p in PDF_TERMS))
        m["name_invoice"] = int(bool(NAME_TERM.search(raw)))
        scopes = list((s.get("RawQueryDict") or {}).get("SearchScopes") or [])
        extra["scopes"] = scopes
        m["documents_scope"] = int(bool(scopes) and all(str(x) in DOCS for x in scopes))
        attrs = set(re.findall(r"\b(_?kMDItem\w+)", raw))
        extra["attributes"] = sorted(attrs)
        m["no_extra_criteria"] = int(bool(attrs) and attrs <= KIND_ATTRS | NAME_ATTRS)
    favorites = sorted(sfl_dir.glob("com.apple.LSSharedFileList.FavoriteItems.sfl*")) if sfl_dir.is_dir() else []
    needle = f"{NAME}.savedSearch".encode()
    m["in_sidebar"] = int(any(needle in p.read_bytes() for p in favorites))
    events_log = M.input_events(root / "input.log")
    m["ui_path"] = int(bool({e.get("kind") for e in events_log} & {"click", "drag"}))
    extra["input_events"] = len(events_log)
    cu.emit(m, METRICS, **extra)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
