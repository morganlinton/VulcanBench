#!/usr/bin/env python3
"""Reject raw unwrap()/expect() calls in production Rust code."""

from __future__ import annotations

import re
import sys
from pathlib import Path


RAW_PANIC = re.compile(r"\.(unwrap|expect)\s*\(")
LIMIT = 0
CFG_TEST = re.compile(r"^\s*#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")
TEST_ATTR = re.compile(r"^\s*#\s*\[\s*(tokio::)?test\b")
MOD_OPEN = re.compile(r"\bmod\s+\w+\s*\{")
TEST_HELPER_MOD_OPEN = re.compile(r"\bmod\s+(test_helpers|tests)\s*\{")
FN_OPEN = re.compile(r"\bfn\s+\w+\s*(?:<[^>]*>)?\s*\([^)]*\)[^{;]*\{")


def brace_delta(line: str) -> int:
    stripped = line.split("//", 1)[0]
    return stripped.count("{") - stripped.count("}")


def rust_files(root: Path) -> list[Path]:
    # Path-component exclusions. Any file whose relative path contains
    # one of these directory names is dropped. `test_support` is the
    # in-crate test-scaffolding convention used by vyre-libs and others.
    ignored_parts = {
        "target",
        ".git",
        "tests",
        "benches",
        "examples",
        "fuzz",
        "xtask",
        "test_support",
    }
    # Crate-level exclusions: directories whose entire src/ is bench
    # / harness / scaffolding rather than production. The `benches`
    # directory match above only catches the conventional cargo
    # `benches/` subdirectory; standalone benchmark / test-harness
    # crates ship unwraps intentionally because they are measurement
    # / scaffolding code, not the dispatch path.
    ignored_crates = {"vyre-bench", "vyre-test-harness"}
    # File-name exclusions. `test_helpers.rs` is the in-crate
    # test-fixture convention used by vyre-runtime / vyre-foundation.
    ignored_filenames = {"build.rs", "tests.rs", "test_helpers.rs"}
    files: list[Path] = []
    for path in root.rglob("*.rs"):
        rel_parts = path.relative_to(root).parts
        if ignored_parts.intersection(rel_parts):
            continue
        if any(part in ignored_crates for part in rel_parts):
            continue
        if path.name in ignored_filenames or path.name.endswith("_tests.rs"):
            continue
        files.append(path)
    return sorted(files)


def production_violations(path: Path) -> list[tuple[int, str]]:
    violations: list[tuple[int, str]] = []
    cfg_test_pending = False
    test_attr_pending = False
    test_depths: list[int] = []
    depth = 0

    for line_no, line in enumerate(path.read_text(errors="replace").splitlines(), start=1):
        stripped = line.strip()

        if CFG_TEST.match(line):
            cfg_test_pending = True
        elif TEST_ATTR.match(line):
            test_attr_pending = True

        enters_test_scope = False
        if (cfg_test_pending and (MOD_OPEN.search(line) or FN_OPEN.search(line))) or TEST_HELPER_MOD_OPEN.search(line):
            enters_test_scope = True
            cfg_test_pending = False
        if test_attr_pending and FN_OPEN.search(line):
            enters_test_scope = True
            test_attr_pending = False

        if not stripped.startswith("//") and not test_depths and RAW_PANIC.search(line):
            violations.append((line_no, line.rstrip()))

        delta = brace_delta(line)
        if enters_test_scope:
            test_depths.append(depth + delta)
        depth += delta

        while test_depths and depth < test_depths[-1]:
            test_depths.pop()

        if stripped and not stripped.startswith("#") and not enters_test_scope:
            if cfg_test_pending and "mod " not in line:
                cfg_test_pending = False
            if test_attr_pending and "fn " not in line:
                test_attr_pending = False

    return violations


def main() -> int:
    root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path.cwd()
    all_violations: list[tuple[Path, int, str]] = []
    for path in rust_files(root):
        for line_no, line in production_violations(path):
            all_violations.append((path, line_no, line))

    if len(all_violations) > LIMIT:
        for path, line_no, line in all_violations:
            print(f"{path.relative_to(root)}:{line_no}: raw panic call in production code", file=sys.stderr)
            print(f"  {line}", file=sys.stderr)
        print(
            f"Fix: replace {len(all_violations)} production unwrap()/expect() calls with structured error handling (ratchet limit is {LIMIT}).",
            file=sys.stderr,
        )
        return 1

    print(f"unwrap() check: {len(all_violations)} production unwrap()/expect() calls (under limit of {LIMIT}).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
