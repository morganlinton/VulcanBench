#!/usr/bin/env python3
"""Find reserve calls that pass remaining capacity instead of additional len."""

from pathlib import Path
import sys


def reserve_block_uses_capacity(lines: list[str], start: int) -> tuple[bool, int]:
    """Return whether a try_reserve call block references .capacity()."""
    line = lines[start]
    call_pos = line.find(".try_reserve")
    block = [line[call_pos:]]
    end = start
    while end + 1 < len(lines) and ";" not in block[-1]:
        end += 1
        block.append(lines[end])
        if end - start > 8:
            break
    return ".capacity()" in "\n".join(block), end


def iter_rust_files(root: Path):
    for path in root.rglob("*.rs"):
        if "target" in path.parts or ".git" in path.parts:
            continue
        yield path


def main() -> int:
    root = Path(sys.argv[1])
    findings: list[str] = []
    for path in iter_rust_files(root):
        lines = path.read_text(errors="ignore").splitlines()
        index = 0
        while index < len(lines):
            line = lines[index]
            if ".try_reserve" not in line:
                index += 1
                continue
            uses_capacity, end = reserve_block_uses_capacity(lines, index)
            if uses_capacity:
                findings.append(f"{path.relative_to(root)}:{index + 1}: {line.strip()}")
            index = max(end + 1, index + 1)
    for finding in findings:
        print(finding)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
