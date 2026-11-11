#!/usr/bin/env python3
"""Build the one-model Codex catalog the ComputerUse v1 codex reference runs with.

    python3 scripts/computer-use/codex_catalog.py --model gpt-6.1-sol --out PATH [--source FILE]

Codex decides part of its tool surface from the model's catalog entry, not
from config.toml: ``apply_patch_tool_type`` adds the apply_patch file editor
and ``multi_agent_version`` adds the subagent tools, whatever the
``[features]`` table says (checked against Codex 0.159.0 with a local stand-in
for the Responses API, 2026-11-10; docs/DECISIONS.md). The codex reference
must act only through the task's MCP computer tools, as the claude-code
reference does with its own tools disallowed, so the gate gives Codex a local
catalog (``model_catalog_json`` in scripts/computer-use/codex-gate.toml) that
holds the pinned model's served entry with those two keys cleared. Everything
else in the entry (instructions, tool mode, reasoning levels) stays as OpenAI
serves it.

The entry is read from ``codex debug models`` on this host (the CLI fetches
the catalog for the logged-in account; no model call, no quota), or from
--source (a file with the same {"models": [...]} shape, used by the tests).
Nothing from the catalog is committed to the repository: run_gate.sh writes
the result next to the job's status file and into the job's task copy.

Prints one JSON line: model, sha256 of the written file, the cleared keys and
their served values, tool_mode and shell_type. Exits 2 when the model is not in
the catalog.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

# Catalog keys that add a tool the computer-use row must not have.
CLEARED = ("apply_patch_tool_type", "multi_agent_version")


def load_catalog(source: str | None) -> dict:
    if source:
        data: dict = json.loads(Path(source).read_text())
        return data
    codex = os.environ.get("CODEX_BIN", "codex")
    proc = subprocess.run(
        [codex, "debug", "models"], capture_output=True, text=True, timeout=120, check=False
    )
    if proc.returncode != 0:
        sys.exit(f"codex_catalog: `{codex} debug models` failed: {proc.stderr.strip()[:300]}")
    data = json.loads(proc.stdout)
    return data


def lock_entry(catalog: dict, model: str) -> tuple[dict, dict]:
    models = catalog.get("models") if isinstance(catalog, dict) else None
    entry = next((m for m in models or [] if m.get("slug") == model), None)
    if entry is None:
        print(f"codex_catalog: {model} is not in the catalog", file=sys.stderr)
        sys.exit(2)
    locked = dict(entry)
    served = {k: entry.get(k) for k in CLEARED}
    for k in CLEARED:
        locked[k] = None
    return locked, served


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--model", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument(
        "--source", help="read the catalog from this file instead of `codex debug models`"
    )
    args = ap.parse_args(argv)
    locked, served = lock_entry(load_catalog(args.source), args.model)
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps({"models": [locked]}, indent=1, sort_keys=True) + "\n"
    out.write_text(text)
    print(
        json.dumps(
            {
                "model": args.model,
                "sha256": hashlib.sha256(text.encode()).hexdigest(),
                "cleared": served,
                "tool_mode": locked.get("tool_mode"),
                "shell_type": locked.get("shell_type"),
                "source": args.source or "codex debug models",
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
