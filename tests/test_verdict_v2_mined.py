"""Mined Verdict v2 families, against small synthetic caches (no network)."""

from __future__ import annotations

import gzip
import json
import random
from collections import Counter
from pathlib import Path
from typing import Any

from harness.verdict.v2.families import mined
from harness.verdict.v2.items import answer_label
from harness.verdict.v2.registry import BuildContext, builder_for


def _ctx(root: Path, per_family: int = 250, **options: Any) -> BuildContext:
    return BuildContext(
        seed=7, repo=root, per_family=per_family, options={"mined_dir": str(root), **options}
    )


def _write_jsonl(path: Path, records: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(r) + "\n" for r in records))


def _write_tree(root: Path, repo: str, sha: str, files: dict[str, int]) -> None:
    path = mined.tree_path(root, repo, sha)
    path.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(path, "wt", encoding="utf-8") as fh:
        json.dump({"repo": repo, "sha": sha, "truncated": False, "files": list(files.items())}, fh)


# --- paths ------------------------------------------------------------------


def test_source_and_test_paths():
    assert mined.source_lang("src/pkg/core.py") == "python"
    assert mined.source_lang("tests/test_core.py") is None
    assert mined.source_lang("pkg/core_test.go") is None
    assert mined.source_lang("src/app.spec.ts") is None
    assert mined.source_lang("types/index.d.ts") is None
    assert mined.source_lang("packages/resolution/test-resolution.ts") is None
    assert mined.source_lang("src/latest.ts") == "typescript"
    assert mined.source_lang("vendor/x/y.go") is None
    assert mined.source_lang("crates/a/src/lib.rs") == "rust"
    assert mined.is_public_module("src/werkzeug/datastructures/accept.py")
    assert mined.is_public_module("pkg/__init__.py")
    assert not mined.is_public_module("httpx/_client.py")
    assert not mined.is_public_module("pkg/_internal/util.py")
    assert not mined.is_public_module("hypothesis/src/hypothesis/internal/utils.py")
    assert not mined.is_public_module("setup.py")
    assert not mined.is_public_module("maint_tools/sort_whats_new.py")


def test_names_file_matches_path_name_and_stem_only_as_words():
    assert mined.names_file("crash in src/pkg/router.py", "src/pkg/router.py")
    assert mined.names_file("see router.py line 3", "src/pkg/router.py")
    assert mined.names_file("The Router module breaks", "src/pkg/router.py")
    assert not mined.names_file("routers are broken", "src/pkg/router.py")
    assert not mined.names_file("rapid failure", "pkg/api.go")


# --- fix-file ---------------------------------------------------------------


def _fix_file_cache(root: Path, n_repos: int = 3, per_repo: int = 4) -> None:
    records = []
    for r in range(n_repos):
        repo = f"org/repo{r}"
        files = {f"pkg/sub/mod{i}.py": 100 + 37 * i for i in range(25)}
        files.update({f"pkg/other/thing{i}.py": 50 + i for i in range(10)})
        files["pkg/sub/web.ts"] = 999
        files["README.md"] = 10
        for k in range(per_repo):
            sha = f"sha{r}{k}"
            _write_tree(root, repo, sha, files)
            records.append(
                {
                    "status": "ok",
                    "repo": repo,
                    "number": 100 + k,
                    "merged_at": "2026-07-01T00:00:00Z",
                    "base_sha": sha,
                    "fixed_file": f"pkg/sub/mod{k + 3}.py",
                    "issue_number": 10 + k,
                    "issue_title": "Parsing fails on empty headers",
                    "issue_body": "Steps: call parse with an empty value.\n<!-- template -->",
                }
            )
    # One issue names the fixed file's stem verbatim: it must be excluded.
    records.append({**records[0], "number": 999, "issue_body": "the mod3 module crashes"})
    # One predates the cutoff.
    records.append({**records[1], "number": 998, "merged_at": "2026-05-30T00:00:00Z"})
    _write_jsonl(root / mined.FIX_FILE_CACHE, records)


def test_fix_file_builds_options_from_same_family_and_excludes_verbatim(tmp_path):
    _fix_file_cache(tmp_path)
    items = mined.build_fix_file(_ctx(tmp_path))
    assert len(items) == 12
    prs = {i.source["pr"] for i in items}
    assert 999 not in prs and 998 not in prs
    for item in items:
        options = item.question["options"]
        assert mined.FIX_FILE_MIN_OPTIONS <= len(options) <= mined.FIX_FILE_MAX_OPTIONS
        descriptions = item.question["descriptions"]
        fixed = descriptions[item.answer]
        assert fixed.startswith("pkg/sub/mod")
        assert all(d.endswith(".py") for d in descriptions.values())
        assert "pkg/sub/web.ts" not in descriptions.values()
        assert "<!--" not in item.state
        assert set(item.shortcuts) == {"token_overlap", "largest_file", "shortest_path"}
        assert all(v in options for v in item.shortcuts.values())
        assert item.reference == "merged-fix"
    # Deterministic given the cache and seed.
    again = mined.build_fix_file(_ctx(tmp_path))
    assert [i.to_json() for i in items] == [i.to_json() for i in again]


def test_fix_file_largest_file_shortcut_is_mostly_beaten(tmp_path):
    _fix_file_cache(tmp_path, n_repos=10, per_repo=6)
    items = mined.build_fix_file(_ctx(tmp_path))
    hits = sum(i.shortcuts["largest_file"] == i.answer for i in items)
    assert hits / len(items) < 0.25


def test_listed_paths_narrow_to_subtree_when_too_long():
    files = {f"big/dir{i}/{'x' * 80}{j}.py": 1 for i in range(20) for j in range(60)}
    files["small/pkg/fixed.py"] = 1
    files.update({f"small/pkg/f{i}.py": 1 for i in range(20)})
    listed, prefix = mined.listed_paths(files, "small/pkg/fixed.py")
    assert prefix == "small/"
    assert "small/pkg/fixed.py" in listed
    assert all(p.startswith("small/") for p in listed)


# --- comments and functions -------------------------------------------------


def test_strip_pair_python_removes_differing_and_sensitive_comments():
    before = (
        "def load(path):\n"
        '    """Load a file."""\n'
        "    # read the data\n"
        "    return open(path).read()  # CVE-2026-1 was here\n"
    )
    after = (
        "def load(path):\n"
        '    """Load a file, rejecting traversal."""\n'
        "    # read the data\n"
        "    # prevent path traversal\n"
        "    if '..' in path:\n"
        "        raise ValueError(path)\n"
        "    return open(path).read()\n"
    )
    stripped = mined.strip_pair(before, after, "python")
    assert stripped is not None
    b, a = stripped
    assert "# read the data" in b and "# read the data" in a
    assert "traversal" not in a and "CVE" not in b
    assert "Load a file" not in a and "Load a file" not in b
    assert "raise ValueError(path)" in a


def test_strip_pair_clike_handles_strings_and_block_comments():
    before = 'func Get(p string) string {\n\t// shared note\n\turl := "http://x//y"\n\treturn url + p\n}\n'
    after = (
        "func Get(p string) string {\n\t// shared note\n\t/* reject untrusted input */\n"
        '\tif strings.Contains(p, "..") {\n\t\treturn ""\n\t}\n'
        '\turl := "http://x//y"\n\treturn url + p\n}\n'
    )
    stripped = mined.strip_pair(before, after, "go")
    assert stripped is not None
    b, a = stripped
    assert '"http://x//y"' in b and '"http://x//y"' in a
    assert "// shared note" in a
    assert "reject untrusted" not in a


def test_rust_lifetimes_do_not_start_char_literals():
    code = "fn f<'a>(x: &'a str) -> char {\n    // hi\n    let c = '{';\n    x.len();\n    c\n}\n"
    masked, spans = mined.scan_clike(code, "rust")
    assert len(masked) == len(code)
    assert [s.text for s in spans] == ["// hi"]
    funcs = mined.clike_functions(code, "rust")
    assert [f.name for f in funcs] == ["f"]
    assert funcs[0].end == 5


def test_changed_function_picks_the_touched_function():
    before = (
        "package x\n\nfunc A() int {\n\treturn 1\n}\n\n"
        "func (s *Srv) Handle(p string) error {\n\tuse(p)\n\treturn nil\n}\n"
    )
    after = before.replace("\tuse(p)\n", '\tif p == "" {\n\t\treturn errBad\n\t}\n\tuse(p)\n')
    found = mined.changed_function(before, after, "go")
    assert found is not None
    f, g = found
    assert f.name == "Srv.Handle" and "errBad" in g.text and "errBad" not in f.text

    py_before = (
        "class C:\n    def a(self):\n        return 1\n\n    def b(self, x):\n        return x\n"
    )
    py_after = py_before.replace(
        "        return x\n", "        if x < 0:\n            x = 0\n        return x\n"
    )
    found = mined.changed_function(py_before, py_after, "python")
    assert found is not None and found[0].name == "C.b"

    js_before = "export function parse(s) {\n  return s.split(',');\n}\nconst other = (a) => {\n  return a;\n};\n"
    js_after = js_before.replace("  return a;\n", "  if (!a) return null;\n  return a;\n")
    found = mined.changed_function(js_before, js_after, "javascript")
    assert found is not None and found[0].name == "other"


# --- CWE collapsing ---------------------------------------------------------


def test_collapse_cwes():
    assert mined.collapse_cwes(["CWE-89"]) == "injection"
    assert mined.collapse_cwes(["CWE-22"]) == "path-traversal"
    assert mined.collapse_cwes(["CWE-20", "CWE-79"]) == "xss"
    assert mined.collapse_cwes(["CWE-1333"]) == "resource-exhaustion"
    assert mined.collapse_cwes(["CWE-1321"]) == "deserialization"
    assert mined.collapse_cwes(["CWE-601"]) == "ssrf"
    assert mined.collapse_cwes(["CWE-287", "CWE-347"]) is None  # families disagree
    assert mined.collapse_cwes(["CWE-20"]) is None
    assert mined.collapse_cwes([]) is None
    assert set(mined.CWE_FAMILIES) == set(mined.WEAKNESS_CLASSES)
    assert len(mined.WEAKNESS_CLASSES) == 10


def test_keyword_weakness_rules():
    assert mined.keyword_weakness("cursor.execute(sql_query)") == "injection"
    assert mined.keyword_weakness("os.path.join(root, filename)") == "path-traversal"


# --- advisories: vuln-pair and weakness-class --------------------------------

_CWES = ["CWE-22", "CWE-89", "CWE-79", "CWE-400", "CWE-918"]


def _advisory_cache(root: Path, n: int = 60) -> None:
    records = []
    for i in range(n):
        body = "\n".join(f"    step{j} = compute(step{j - 1}, {i})" for j in range(1, 4))
        before = f"def handler_{i}(value):\n    step0 = value\n{body}\n    return step3\n"
        kind = i % 4
        if kind == 0:
            # The fix removes a dangerous branch: shorter, no check words either side.
            before_fn = before.replace(
                "    return step3\n", "    if value:\n        run(value)\n    return step3\n"
            )
            after_fn = before
        elif kind == 1:
            # The fix adds a check: longer, more check words.
            before_fn = before
            after_fn = before.replace(
                "    step0 = value\n",
                "    # CVE-2026-9 guard\n    check_limit(value)\n    step0 = value\n",
            )
        elif kind == 2:
            # The fix adds a conversion: longer, no check words.
            before_fn = before
            after_fn = before.replace(
                "    step0 = value\n", "    step0 = str(value)\n    step0 = step0.strip()\n"
            )
        else:
            # The fix replaces two lines with one validated call: shorter, more check words.
            before_fn = before.replace(
                "    step0 = value\n",
                "    step0 = value\n    step0 = step0 or 0\n    step0 = step0 + 0\n",
            )
            after_fn = before.replace("    step0 = value\n", "    step0 = validate(value)\n")
        module_b = f"import os\n\n\ndef other():\n    return 1\n\n\n{before_fn}"
        module_a = f"import os\n\n\ndef other():\n    return 1\n\n\n{after_fn}"
        records.append(
            {
                "status": "ok",
                "id": f"GHSA-{i:04d}",
                "published_at": "2026-07-01T00:00:00Z",
                "repo": f"org/lib{i % 25}",
                "fix_commit": f"c{i}",
                "commit_date": "2026-06-20T00:00:00Z",
                "cwes": [_CWES[i % len(_CWES)]],
                "files": [{"path": "lib/handlers.py", "before": module_b, "after": module_a}],
            }
        )
    # Published after the cutoff but fixed before it: dropped by default.
    records.append(
        {
            **records[0],
            "id": "GHSA-early",
            "fix_commit": "early",
            "commit_date": "2026-05-01T00:00:00Z",
        }
    )
    records.append({"status": "skip:no-fix-files", "id": "GHSA-skip", "repo": "x/y"})
    _write_jsonl(root / mined.ADVISORY_CACHE, records)


def test_vuln_pair_positions_balanced_and_comments_stripped(tmp_path):
    _advisory_cache(tmp_path)
    items = mined.build_vuln_pair(_ctx(tmp_path))
    assert len(items) >= 30
    answers = Counter(i.answer for i in items)
    assert abs(answers["A"] - answers["B"]) <= 1
    for item in items:
        assert "CVE" not in item.state
        assert item.question["options"] == ["A", "B"]
        assert set(item.shortcuts) == {"longer", "shorter", "more_checks", "fewer_checks"}
    for name in ("longer", "shorter", "more_checks", "fewer_checks"):
        acc = sum(i.shortcuts[name] == i.answer for i in items) / len(items)
        assert acc <= 0.62, (name, acc)


def test_fix_commit_date_filter(tmp_path):
    _advisory_cache(tmp_path)
    strict = mined.advisory_pairs(tmp_path, fix_after_cutoff=True)
    loose = mined.advisory_pairs(tmp_path)
    assert all(p.record.get("fix_commit") != "early" for p in strict)
    assert any(p.record.get("fix_commit") == "early" for p in loose)


def test_weakness_class_labels_from_cwe_and_caps_classes(tmp_path):
    _advisory_cache(tmp_path)
    items = mined.build_weakness_class(_ctx(tmp_path, per_family=40))
    assert items
    counts = Counter(i.answer for i in items)
    assert set(counts) <= set(mined.WEAKNESS_CLASSES)
    assert max(counts.values()) <= 0.2 * 40 + 1
    for item in items:
        assert set(item.question["options"]) == set(mined.WEAKNESS_CLASSES)
        assert "CWE" not in item.state and "CVE" not in item.state
        assert item.reference == "advisory"


def test_missing_cache_returns_no_items(tmp_path):
    ctx = _ctx(tmp_path / "absent")
    for build in mined.BUILDERS.values():
        assert build(ctx) == []


def test_registry_finds_all_four_builders():
    for family in ("fix-file", "vuln-pair", "weakness-class", "semver-impact"):
        assert builder_for(family) is mined.BUILDERS[family]


# --- semver-impact ------------------------------------------------------------


def _level(before: str, after: str, path: str = "pkg/core.py") -> str:
    record = {
        "files": [{"path": path, "status": "modified", "patch": "@@"}],
        "contents": {path: {"before": before, "after": after}},
    }
    labelled = mined.label_semver(record)
    assert labelled is not None
    return labelled[0]


def test_api_differ_levels():
    base = "def f(a, b=1):\n    return a\n\nclass C:\n    def m(self, x):\n        return x\n"
    assert _level(base, base.replace("return a", "return a + 1")) == "patch"
    assert _level(base, base.replace("def f(a, b=1)", "def f(a, b=1, c=None)")) == "minor"
    assert _level(base, base.replace("def f(a, b=1)", "def f(a, b=1, *, c=None)")) == "minor"
    assert _level(base, base.replace("def f(a, b=1)", "def f(a, b=1, *, c)")) == "major"
    assert _level(base, base.replace("def f(a, b=1)", "def f(a, c, b=1)")) == "major"
    assert _level(base, base.replace("def f(a, b=1)", "def f(a, *, b=1)")) == "major"
    assert _level(base, base.replace("def f(a, b=1)", "def f(a, b)")) == "major"
    assert _level(base, base.replace("def f(a, b=1)", "def f(a)")) == "major"
    assert _level(base, base + "\ndef g():\n    pass\n") == "minor"
    assert _level(base, base + "\ndef _g():\n    pass\n") == "patch"
    assert _level(base, base.replace("def m(self, x)", "def m(self, y)")) == "major"
    assert (
        _level(base, base.replace("    def m(self, x):\n        return x\n", "    pass\n"))
        == "major"
    )
    assert _level(base, base.replace("def f(", "async def f(")) == "major"
    assert _level(base, base + "\nLIMIT = 3\n") == "minor"
    # Private modules have no public API by convention.
    assert _level(base, "", path="pkg/_core.py") == "patch"


def test_api_differ_moved_and_reexported_name_is_not_removed():
    before = "class HeaderSet:\n    def add(self, x):\n        pass\n\nclass Other:\n    pass\n"
    after = "from .set import HeaderSet\n\nclass Other:\n    pass\n"
    assert _level(before, after) == "patch"
    assert _level(before, "class Other:\n    pass\n") == "major"


def test_api_differ_inheritance_lazy_names_and_typevars():
    before = (
        "class Base:\n    def help(self):\n        pass\n\n"
        "class Opt(Base):\n    def help(self):\n        pass\n\n"
        "class Old:\n    pass\n"
    )
    # The override moves to the base class: still callable on Opt.
    after_inherit = before.replace(
        "class Opt(Base):\n    def help(self):\n        pass\n", "class Opt(Base):\n    pass\n"
    )
    assert _level(before, after_inherit) == "patch"
    # A removed class served by a module __getattr__ is deprecated, not removed.
    after_lazy = before.replace(
        "class Old:\n    pass\n",
        "def __getattr__(name):\n    if name == 'Old':\n        return 1\n",
    )
    assert _level(before, after_lazy) == "patch"
    assert (
        _level(before, before + "\nT = TypeVar('T')\nlogger = logging.getLogger(__name__)\n")
        == "patch"
    )


def test_api_differ_new_module_and_init_imports():
    record = {
        "files": [
            {"path": "pkg/new.py", "status": "added", "patch": "@@"},
            {"path": "pkg/__init__.py", "status": "modified", "patch": "@@"},
        ],
        "contents": {
            "pkg/new.py": {"before": None, "after": "def fresh():\n    pass\n"},
            "pkg/__init__.py": {
                "before": "from .core import a, b\n",
                "after": "from .core import a\n",
            },
        },
    }
    labelled = mined.label_semver(record)
    assert labelled is not None and labelled[0] == "major"


def _semver_cache(root: Path) -> None:
    variants = {
        "patch": ("def f(a):\n    return a\n", "def f(a):\n    return a + 1\n"),
        "minor": ("def f(a):\n    return a\n", "def f(a, b=None):\n    return a\n"),
        "major": ("def f(a):\n    return a\n\ndef g():\n    pass\n", "def f(a):\n    return a\n"),
    }
    records = []
    for i in range(90):
        level = ["patch", "patch", "patch", "minor", "major"][i % 5]
        before, after = variants[level]
        records.append(
            {
                "status": "ok",
                "repo": f"org/py{i % 22}",
                "number": i,
                "merged_at": "2026-08-01T00:00:00Z",
                "files": [
                    {
                        "path": "pkg/core.py",
                        "status": "modified",
                        "patch": f"@@ -1 +1 @@\n-{i}\n+{i}x",
                    }
                ],
                "contents": {"pkg/core.py": {"before": before, "after": after}},
            }
        )
    _write_jsonl(root / mined.SEMVER_CACHE, records)


def test_semver_items_are_level_balanced_and_tool_referenced(tmp_path):
    _semver_cache(tmp_path)
    items = mined.build_semver_impact(_ctx(tmp_path))
    assert items
    counts = Counter(answer_label(i.question, i.answer) for i in items)
    assert set(counts) == {"patch", "minor", "major"}
    assert max(counts.values()) / len(items) <= 0.45
    for item in items:
        assert item.question["levels"] == ["patch", "minor", "major"]
        assert item.reference == "tool"
        assert set(item.shortcuts) == {"size_band", "remove_words", "private_paths"}


# --- balancing --------------------------------------------------------------


def test_cap_shortcuts_brings_every_baseline_under_the_cap():
    rng = random.Random(1)
    cands = [
        mined.Candidate(
            key=f"k{i}",
            unit=f"u{i % 5}",
            answer="x",
            shortcuts={"s": "x" if i % 10 < 8 else "y"},
        )
        for i in range(100)
    ]
    kept = mined.cap_shortcuts(cands, 0.55, rng)
    acc = sum(c.shortcuts["s"] == c.answer for c in kept) / len(kept)
    assert acc <= 0.55
    assert len(kept) >= 40


def test_spread_units_round_robins():
    rng = random.Random(2)
    cands = [
        mined.Candidate(key=f"k{i}", unit="big" if i < 50 else f"u{i}", answer="a", shortcuts={})
        for i in range(60)
    ]
    chosen = mined.spread_units(cands, 20, rng)
    assert len(chosen) == 20
    # Every small unit is taken before the big one fills the rest.
    assert sum(c.unit != "big" for c in chosen) == 10
