"""Quality and security analyzers for JavaScript, Rust, C and C++ against the real tools.

Each test skips when its tool is not installed (scripts/install_analyzers.sh sets
them up). The workspaces are git repositories with a base commit, like a real run,
so the added-lines scope of the security scanners is exercised too.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

from harness.evaluator import cfamily, diffscope
from harness.evaluator.langs import group_by_language
from harness.evaluator.quality import assess_quality, eslint_dir
from harness.evaluator.security import assess_security
from harness.task_metadata import CODE_SUFFIXES

needs_clang_tidy = pytest.mark.skipif(cfamily.clang_tidy_path() is None, reason="no clang-tidy")
needs_cppcheck = pytest.mark.skipif(shutil.which("cppcheck") is None, reason="no cppcheck")
needs_eslint = pytest.mark.skipif(
    not (eslint_dir() / "node_modules" / ".bin" / "eslint").exists()
    or shutil.which("node") is None,
    reason="pinned eslint not installed",
)
needs_cargo = pytest.mark.skipif(
    shutil.which("cargo") is None or shutil.which("cargo-clippy") is None,
    reason="no cargo clippy",
)


def _git(ws: Path, *args: str) -> None:
    subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.invalid", *args],
        cwd=ws,
        check=True,
        capture_output=True,
    )


def _repo(ws: Path, files: dict[str, str]) -> Path:
    for name, text in files.items():
        (ws / name).parent.mkdir(parents=True, exist_ok=True)
        (ws / name).write_text(text)
    _git(ws, "init", "-q")
    _git(ws, "add", "-A")
    _git(ws, "commit", "-q", "-m", "base")
    return ws


# --- language detection and line counting --------------------------------------


def test_headers_follow_the_change_language(tmp_path: Path) -> None:
    assert group_by_language(["a.c", "a.h"]) == {"c": ["a.c", "a.h"]}
    assert group_by_language(["a.cpp", "a.h"]) == {"cpp": ["a.cpp", "a.h"]}
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "x.cpp").write_text("int x;\n")
    assert group_by_language(["include/a.h"], tmp_path) == {"cpp": ["include/a.h"]}
    assert group_by_language(["include/a.h"]) == {"c": ["include/a.h"]}


def test_c_family_sources_count_as_code() -> None:
    for suffix in (".c", ".h", ".cpp", ".cc", ".hpp", ".mjs"):
        assert suffix in CODE_SUFFIXES


def test_added_lines_scope(tmp_path: Path) -> None:
    ws = _repo(tmp_path, {"a.c": "int a;\nint b;\n"})
    (ws / "a.c").write_text("int a;\nint b;\nint c;\n")
    (ws / "new.c").write_text("int d;\n")
    _git(ws, "add", "-A")
    scope = diffscope.added_lines(ws, ["a.c", "new.c"])
    assert scope == {"a.c": {3}, "new.c": {1}}
    assert diffscope.added_lines(tmp_path / "missing", ["a.c"]) is None


# --- C and C++ -----------------------------------------------------------------

_C_BASE = {
    "include/demo/buf.h": "#ifndef DEMO_BUF_H\n#define DEMO_BUF_H\nint buf_len(const char *s);\n#endif\n",
    "src/buf.c": '#include "demo/buf.h"\n\nint buf_len(const char *s) { return s ? 1 : 0; }\n',
}


@needs_clang_tidy
def test_c_quality_compiles_with_project_includes(tmp_path: Path) -> None:
    ws = _repo(tmp_path, _C_BASE)
    result = assess_quality(ws, ["src/buf.c"])
    details = result.details["languages"]["c"]
    assert details["compile_errors"] == 0
    assert result.score == 1.0
    assert details["tool_version"]


@needs_clang_tidy
def test_c_security_counts_only_added_memory_errors(tmp_path: Path) -> None:
    ws = _repo(tmp_path, _C_BASE)
    (ws / "src" / "buf.c").write_text(
        '#include <stdlib.h>\n#include "demo/buf.h"\n\n'
        "int buf_len(const char *s) { return s ? 1 : 0; }\n\n"
        "int use_after_free(void) {\n"
        "    int *p = malloc(sizeof *p);\n"
        "    if (!p) return 0;\n"
        "    free(p);\n"
        "    return *p;\n"
        "}\n"
    )
    _git(ws, "add", "-A")
    result = assess_security(ws, ["src/buf.c"])
    details = result.details["languages"]["c"]
    assert details["scope"] == "added lines"
    assert details["high"] >= 1
    assert result.score is not None and result.score < 1.0


@needs_clang_tidy
def test_c_security_ignores_findings_on_untouched_lines(tmp_path: Path) -> None:
    risky = (
        "#include <stdlib.h>\n\n"
        "int use_after_free(void) {\n"
        "    int *p = malloc(sizeof *p);\n"
        "    if (!p) return 0;\n"
        "    free(p);\n"
        "    return *p;\n"
        "}\n"
    )
    ws = _repo(tmp_path, {"src/risky.c": risky})
    (ws / "src" / "risky.c").write_text(risky + "\nint answer(void) { return 42; }\n")
    _git(ws, "add", "-A")
    result = assess_security(ws, ["src/risky.c"])
    assert result.details["languages"]["c"]["high"] == 0
    assert result.score == 1.0


@needs_clang_tidy
@needs_cppcheck
def test_cpp_header_change_parses_as_cpp(tmp_path: Path) -> None:
    ws = _repo(
        tmp_path,
        {
            "include/demo/box.h": "#pragma once\n#include <string>\nstd::string box();\n",
            "src/box.cpp": '#include "demo/box.h"\nstd::string box() { return "x"; }\n',
        },
    )
    (ws / "include" / "demo" / "box.h").write_text(
        "#pragma once\n#include <string>\nstd::string box();\nclass Tag { public: int n = 0; };\n"
    )
    _git(ws, "add", "-A")
    quality = assess_quality(ws, ["include/demo/box.h"])
    assert "cpp" in quality.details["languages"]
    assert quality.details["languages"]["cpp"]["compile_errors"] == 0
    security = assess_security(ws, ["include/demo/box.h"])
    assert security.details["languages"]["cpp"]["score"] == 1.0


# --- JavaScript ------------------------------------------------------------------


@needs_eslint
def test_js_quality_and_security_use_the_pinned_eslint(tmp_path: Path) -> None:
    ws = _repo(
        tmp_path,
        {
            "package.json": '{"type": "module"}\n',
            "src/run.js": "export function run(x) {\n  return x + 1;\n}\n",
        },
    )
    clean = assess_quality(ws, ["src/run.js"])
    assert clean.score == 1.0
    assert clean.details["languages"]["javascript"]["tool_version"] == "10.11.0"
    (ws / "src" / "run.js").write_text(
        "export function run(x) {\n  var unused = 1;\n  return eval(x);\n}\n"
    )
    _git(ws, "add", "-A")
    quality = assess_quality(ws, ["src/run.js"])
    assert quality.details["languages"]["javascript"]["issues"] >= 1
    security = assess_security(ws, ["src/run.js"])
    details = security.details["languages"]["javascript"]
    assert details["high"] >= 1
    assert (
        "no-eval" in details["rules"] or "security/detect-eval-with-expression" in details["rules"]
    )
    assert "npm_audit" not in details  # no declared dependencies, nothing to audit


# --- Rust --------------------------------------------------------------------------


@needs_cargo
def test_rust_quality_counts_real_clippy_and_fmt_findings(tmp_path: Path) -> None:
    ws = _repo(
        tmp_path,
        {
            "Cargo.toml": '[package]\nname = "demo"\nversion = "0.1.0"\nedition = "2021"\n',
            "src/lib.rs": "pub fn ok() -> bool {\n    true\n}\n",
        },
    )
    subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=ws, check=True)
    clean = assess_quality(ws, ["src/lib.rs"])
    assert clean.details["languages"]["rust"]["clippy_warnings"] == 0
    (ws / "src" / "lib.rs").write_text(
        "pub fn ok() -> bool {\n    true\n}\n\n"
        "pub fn empty(v: &Vec<i32>) -> bool { let x = v.len() == 0; x }\n"
    )
    result = assess_quality(ws, ["src/lib.rs"])
    details = result.details["languages"]["rust"]
    assert details["clippy_warnings"] >= 2
    assert details["unformatted"] >= 1
    assert result.score is not None and result.score < 1.0
    security = assess_security(ws, ["src/lib.rs"])
    assert security.details["languages"]["rust"]["audit"] == "skipped: no external dependencies"
