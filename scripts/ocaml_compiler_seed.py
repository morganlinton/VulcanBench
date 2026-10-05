#!/usr/bin/env python3
"""Package and install only generated files from a public OCaml baseline build."""

from __future__ import annotations

import argparse
import hashlib
import json
import tarfile
from pathlib import Path
from typing import Any


def inventory(source: Path, output: Path) -> None:
    """Record original paths before configure or make creates any artifacts."""
    entries: dict[str, Any] = {}
    for path in sorted(source.rglob("*")):
        if path.is_dir():
            continue
        name = path.relative_to(source).as_posix()
        entries[name] = (
            {"symlink": str(path.readlink())}
            if path.is_symlink()
            else {"sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(entries, indent=2) + "\n")


def package(source: Path, seed: Path) -> None:
    """Never package original source paths or private tests in the build seed."""
    original = json.loads((seed / "source-inventory.json").read_text())
    artifacts = []
    with tarfile.open(seed / "artifacts.tar.gz", "w:gz") as archive:
        for path in sorted(source.rglob("*")):
            if path.is_dir():
                continue
            name = path.relative_to(source).as_posix()
            if name in original:
                continue
            if name.startswith(".git/") or name.startswith("hidden/"):
                raise ValueError(f"unexpected private or Git content: {name}")
            # Generated documentation is irrelevant to compiler builds.
            if name.startswith("api_docgen/build/"):
                continue
            archive.add(path, arcname=name, recursive=False)
            artifacts.append(name)
    (seed / "artifact-inventory.json").write_text(json.dumps(artifacts, indent=2) + "\n")


def install(seed: Path, workspace: Path) -> None:
    """Seed a fresh workspace once, without overwriting sources or later edits."""
    marker = workspace / ".vulcanbench-compiler-seeded"
    if marker.exists():
        return
    original = json.loads((seed / "source-inventory.json").read_text())
    expected = set(json.loads((seed / "artifact-inventory.json").read_text()))
    with tarfile.open(seed / "artifacts.tar.gz") as archive:
        members = archive.getmembers()
        if {member.name for member in members} != expected:
            raise ValueError("artifact archive differs from its inventory")
        for member in members:
            path = Path(member.name)
            if path.is_absolute() or ".." in path.parts or member.name in original:
                raise ValueError(f"unsafe or original-source artifact: {member.name}")
            if (workspace / path).exists() or (workspace / path).is_symlink():
                raise ValueError(f"refusing to overwrite existing artifact: {member.name}")
        archive.extractall(workspace, filter="data")
    marker.write_text("Public pre-fix compiler artifacts installed once.\n")
    # The harness initializes Git before setup. Keep public build artifacts out
    # of the captured model patch without ignoring any original source path.
    exclude = workspace / ".git/info/exclude"
    if exclude.parent.is_dir():
        patterns = ["/.vulcanbench-compiler-seeded"]
        for name in sorted(expected):
            escaped = name
            for character in ("\\", "*", "?", "["):
                escaped = escaped.replace(character, "\\" + character)
            patterns.append("/" + escaped)
        patterns.extend(["*.cmi", "*.cmo", "*.cmx", "*.cmxa", "*.cmt", "*.cmti", "*.cmxs"])
        with exclude.open("a") as output:
            output.write("\n# Public OCaml baseline build artifacts\n")
            output.write("\n".join(patterns) + "\n")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("inventory", "package", "install"):
        command = commands.add_parser(name)
        command.add_argument("first", type=Path)
        command.add_argument("second", type=Path)
    args = parser.parse_args()
    {"inventory": inventory, "package": package, "install": install}[args.command](
        args.first, args.second
    )


if __name__ == "__main__":
    main()
