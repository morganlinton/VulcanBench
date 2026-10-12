"""The em-dash check polices written text (prose files) and exempts code."""

import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "check_no_emdash", ROOT / "scripts/check_no_emdash.py"
)
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)

EM = chr(0x2014)


def test_code_files_are_exempt(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    for name in ("a.py", "b.ts", "c.yml", "d.sh", "Makefile"):
        Path(name).write_text(f"x = 'a {EM} b'\n")
        assert check.main(["check", name]) == 0


def test_prose_files_fail(tmp_path, monkeypatch, capsys):
    monkeypatch.chdir(tmp_path)
    for name in ("a.md", "b.mdx", "c.mdc", "d.txt"):
        Path(name).write_text(f"Prose {EM} here\n")
        assert check.main(["check", name]) == 1
    assert "written text" in capsys.readouterr().out


def test_rule_files_and_third_party_trees_skipped():
    assert not check._should_scan("CLAUDE.md")
    assert not check._should_scan("tasks/some-task/README.md")
    assert check._should_scan("docs/DECISIONS.md")
