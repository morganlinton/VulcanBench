import json
from collections import Counter

from harness.tasks import load_task, task_hash
from harness.verdict.v2.families import archive as A
from harness.verdict.v2.items import MAX_STATE_CHARS
from harness.verdict.v2.registry import BuildContext, builder_for

TARGETS = ["alpha_case", "beta_case", "gamma_case", "delta_case"]


def _patch(tag, lines, files=1):
    """A unified diff with ``lines`` added lines spread over ``files`` files."""
    out = []
    per = max(1, lines // files)
    for f in range(files):
        out.append(f"diff --git a/src/m{f}.py b/src/m{f}.py\n--- a/src/m{f}.py\n+++ b/src/m{f}.py")
        out.append(f"@@ -1 +1,{per} @@")
        out.extend(f"+value_{tag}_{f}_{i} = {i}" for i in range(per))
    return "\n".join(out) + "\n"


def _task(root, suite, task_id, targets=TARGETS, gold="--- a/x\n+++ b/x\n+y = 1\n"):
    task = root / suite / task_id
    (task / "repo" / "src").mkdir(parents=True)
    (task / "repo" / "src" / "m0.py").write_text("x = 1\n")
    (task / "tests").mkdir()
    (task / "tests" / "test_hidden.py").write_text(
        "from m0 import x\n\n\n"
        + "".join(f"def test_{n}():\n    assert x == len({n!r})\n\n\n" for n in targets)
    )
    (task / "issue.md").write_text(f"Issue for {task_id}: alpha and beta behave wrongly.")
    (task / "gold_patch.diff").write_text(gold)
    meta = {
        "id": task_id,
        "tests": {
            "fail_to_pass": [
                {"name": n, "cmd": f"python -m pytest test_hidden.py::test_{n} -q"} for n in targets
            ],
            "pass_to_pass": [{"name": "guard", "cmd": "true"}],
        },
    }
    (task / "metadata.json").write_text(json.dumps(meta))
    return task


class Runs:
    def __init__(self, root):
        self.root = root
        self.n = 0

    def add(
        self,
        task_dir,
        patch,
        results,
        *,
        p2p_ok=True,
        finished=True,
        contaminated=False,
        exact_hash=True,
        sub="",
    ):
        self.n += 1
        run = self.root / sub / f"run-{self.n:04d}" if sub else self.root / f"run-{self.n:04d}"
        run.mkdir(parents=True)
        summary = {
            "suite": "demo-suite",
            "task_id": task_dir.name,
            "run_id": run.name,
            "model": "demo:model",
            "finished": finished,
            "effort": {"requested": "low"},
            "task_hash": task_hash(load_task(task_dir.name, task_dir.parent))
            if exact_hash
            else "stale",
            "integrity_audit": {"contaminated": contaminated},
            "verifier": {
                "fail_to_pass": dict(results),
                "pass_to_pass": {"guard": p2p_ok},
                "pass_to_pass_ok": p2p_ok,
            },
        }
        (run / "summary.json").write_text(json.dumps(summary))
        (run / "final.patch").write_text(patch)
        return run


def _ctx(tmp_path, per_family=250, options=None):
    A._ARCHIVE_CACHE.clear()
    return BuildContext(
        seed=7,
        repo=tmp_path,
        run_roots=tuple(sorted(p for p in tmp_path.glob("runs*") if p.is_dir())),
        tasks_roots=(tmp_path / "tasks",),
        per_family=per_family,
        options=options,
    )


def _all(names, failing=()):
    return {n: n not in failing for n in names}


def test_builders_are_registered():
    for family in ("patch-pair", "failing-test"):
        assert builder_for(family) is A.BUILDERS[family]


def test_load_archive_filters_and_dedupes(tmp_path):
    tasks = tmp_path / "tasks"
    gold_lines = "".join(f"+gold_line_number_{i} = {i}\n" for i in range(30))
    task = _task(tasks, "v9", "demo-task", gold=f"--- a/g\n+++ b/g\n{gold_lines}")
    runs = Runs(tmp_path / "runs-demo")
    runs.add(task, _patch("good", 10), _all(TARGETS))
    runs.add(task, _patch("good", 10), _all(TARGETS))  # duplicate
    runs.add(task, _patch("flaky", 10), _all(TARGETS))
    runs.add(task, _patch("flaky", 10), _all(TARGETS, ["beta_case"]))  # disagrees: dropped
    runs.add(task, _patch("unfinished", 10), _all(TARGETS), finished=False)
    runs.add(task, _patch("leak", 10), _all(TARGETS), contaminated=True)
    runs.add(task, "", _all(TARGETS))
    runs.add(task, f"--- a/g\n+++ b/g\n{gold_lines}", _all(TARGETS))  # recites the gold
    runs.add(task, _patch("stale-ok", 10), _all(TARGETS), exact_hash=False)  # name fallback
    runs.add(task, _patch("renamed", 10), {"other": True}, exact_hash=False)  # unresolvable
    Runs(tmp_path / "runs-contaminated").add(task, _patch("quarantined", 10), _all(TARGETS))

    archive = A.load_archive(_ctx(tmp_path).run_roots, (tasks,))
    kept = sorted(p.run_id for p in archive.patches)
    assert kept == ["run-0001", "run-0009"]
    assert archive.skipped["duplicate patch"] == 2
    assert archive.skipped["flaky duplicate (dropped)"] == 1
    assert archive.skipped["unfinished or no per-test results"] == 1
    assert archive.skipped["contaminated"] == 1
    assert archive.skipped["empty patch"] == 1
    assert archive.skipped["gold recitation"] == 1
    assert archive.skipped["task definition not found"] == 1
    assert archive.skipped["excluded archive dir"] == 1
    # Exact-hash and name-fallback runs are graded against different definitions.
    assert len({p.group for p in archive.patches}) == 2
    assert {p.tasks_dir for p in archive.patches} == {"v9"}


def test_patch_size_counts_lines_and_files():
    assert A.patch_size(_patch("x", 12, files=3)) == (12, 3)
    assert A.patch_sha("index abc\n+a  \n") == A.patch_sha("+a\n")


def _pair_tree(tmp_path, n_tasks=6):
    tasks = tmp_path / "tasks"
    runs = Runs(tmp_path / "runs-pairs")
    for t in range(n_tasks):
        task = _task(tasks, "v9", f"task-{t}")
        for i in range(5):
            runs.add(task, _patch(f"pass{t}-{i}", 40 + 2 * i), _all(TARGETS))
            runs.add(task, _patch(f"part{t}-{i}", 41 + 2 * i), _all(TARGETS, ["beta_case"]))
        runs.add(task, _patch(f"reg{t}", 44), _all(TARGETS), p2p_ok=False)
        runs.add(task, _patch(f"huge{t}", 400), _all(TARGETS, ["alpha_case"]))  # never matched
        runs.add(task, _patch(f"wide{t}", 42, files=6), _all(TARGETS, ["alpha_case"]))
    return tmp_path


def test_patch_pair_is_size_matched_balanced_and_capped(tmp_path):
    ctx = _ctx(_pair_tree(tmp_path))
    items = A.build_patch_pair(ctx)
    assert items and len(items) % 2 == 0
    answers = Counter(i.answer for i in items)
    assert answers["A"] == answers["B"]
    per_task = Counter(i.source_unit for i in items)
    assert max(per_task.values()) <= A.PAIR_MAX_PER_TASK
    uses: Counter[str] = Counter()
    for item in items:
        lo, hi = sorted(item.source["lines"])
        assert hi - lo <= A.PAIR_MAX_SIZE_GAP * hi
        f_lo, f_hi = sorted(item.source["files"])
        assert f_hi - f_lo <= A.PAIR_MAX_FILE_GAP
        assert item.reference == "verifier"
        assert item.question["options"] == ["A", "B"]
        assert set(item.shortcuts) == {
            "larger-patch",
            "smaller-patch",
            "more-files",
            "more-test-keywords",
        }
        assert len(item.state) <= MAX_STATE_CHARS
        assert item.source["failing"]["outcome"] in {"partial_fix", "regression"}
        uses[item.source["passing"]["patch_sha256"]] += 1
        uses[item.source["failing"]["patch_sha256"]] += 1
    assert max(uses.values()) <= A.PAIR_MAX_PATCH_USES
    # Size shortcuts are held near 50/50 by the balancing pass.
    for name in ("larger-patch", "smaller-patch"):
        hits = sum(i.shortcuts[name] == i.answer for i in items)
        assert abs(hits - len(items) / 2) <= A.PAIR_BALANCE_SLACK + 1


def test_patch_pair_respects_per_family_and_is_deterministic(tmp_path):
    _pair_tree(tmp_path)
    first = A.build_patch_pair(_ctx(tmp_path, per_family=10))
    again = A.build_patch_pair(_ctx(tmp_path, per_family=10))
    assert len(first) == 10
    assert [i.item_id for i in first] == [i.item_id for i in again]
    assert [i.answer for i in first] == [i.answer for i in again]


def test_failing_test_labels_single_and_two_failure_patches(tmp_path):
    tasks = tmp_path / "tasks"
    runs = Runs(tmp_path / "runs-ft")
    task = _task(tasks, "v9", "ft-task")
    one = runs.add(task, _patch("one", 20), _all(TARGETS, ["gamma_case"]))
    two = runs.add(task, _patch("two", 20), _all(TARGETS, ["alpha_case", "delta_case"]))
    runs.add(task, _patch("three", 20), _all(TARGETS, ["alpha_case", "beta_case", "gamma_case"]))
    runs.add(task, _patch("none", 20), _all(TARGETS, TARGETS))  # nothing passes: unusable
    runs.add(task, _patch("allpass", 20), _all(TARGETS))

    capped = {"failing_prior_share": 1.0, "failing_max_failed": 2}
    items = A.build_failing_test(_ctx(tmp_path, options=capped))
    by_run = {i.source["run_id"]: i for i in items}
    assert set(by_run) == {one.name, two.name}
    default = A.build_failing_test(_ctx(tmp_path, options={"failing_prior_share": 1.0}))
    assert len(default) == 3  # three failing targets is usable by default

    single = by_run[one.name]
    assert single.answer == "gamma_case"
    assert sorted(single.question["options"]) == sorted(TARGETS)
    assert single.question["instructions"] == A.FAILING_ONE_INSTRUCTIONS
    for name in TARGETS:
        assert f"### `{name}`" in single.state
        assert f"def test_{name}():" in single.state
    assert "from m0 import x" in single.state  # the import the tests use

    pair = by_run[two.name]
    opts = pair.question["options"]
    assert pair.answer in {"alpha_case", "delta_case"}
    assert len({"alpha_case", "delta_case"} & set(opts)) == 1  # exactly one failing option
    assert {"beta_case", "gamma_case"} <= set(opts)
    assert pair.question["instructions"] == A.FAILING_SUBSET_INSTRUCTIONS
    for item in items:
        assert item.reference == "verifier"
        assert set(item.shortcuts) == {
            "patch-overlap",
            "issue-overlap",
            "first-listed",
            "last-listed",
            "longest-name",
            "task-prior",
            "source-patch-overlap",
            "source-issue-overlap",
            "longest-source",
        }
        assert all(g in item.question["options"] for g in item.shortcuts.values())


def test_failing_test_caps_the_task_prior(tmp_path):
    tasks = tmp_path / "tasks"
    runs = Runs(tmp_path / "runs-prior")
    for t in range(4):
        task = _task(tasks, "v9", f"prior-{t}")
        # Most failing patches miss the same hard target; a few miss another one.
        for i in range(8):
            runs.add(task, _patch(f"hard{t}-{i}", 20 + i), _all(TARGETS, ["delta_case"]))
        for i, name in enumerate(["alpha_case", "beta_case", "gamma_case"]):
            runs.add(task, _patch(f"easy{t}-{i}", 30 + i), _all(TARGETS, [name]))

    capped = A.build_failing_test(_ctx(tmp_path))
    hits = sum(i.shortcuts["task-prior"] == i.answer for i in capped)
    assert capped and hits <= A.FAILING_PRIOR_MAX_SHARE * len(capped)
    per_test = Counter((i.source_unit, i.answer) for i in capped)
    assert max(per_test.values()) <= A.FAILING_MAX_PER_TEST

    uncapped = A.build_failing_test(_ctx(tmp_path, options={"failing_prior_share": 1.0}))
    assert len(uncapped) > len(capped)


# --- failing-test: hidden test sources ----------------------------------------


def test_parse_target_cmd_forms():
    pt = A.parse_target_cmd(
        "PYTHONPATH=. python -m pytest -c /dev/null oss_tests.py::test_width_rewrap[a-1] -q"
    )
    assert pt == A.TargetSpec("pytest", "oss_tests.py", "test_width_rewrap")
    node = A.parse_target_cmd(
        "node --experimental-strip-types --test "
        "--test-name-pattern='^calls cache\\(\\) once$' tests/merge.test.ts"
    )
    assert node == A.TargetSpec("node", "tests/merge.test.ts", "calls cache() once")
    assert A.parse_target_cmd("go test -run '^TestSplit$' ./...") == A.TargetSpec(
        "go", None, "TestSplit"
    )
    cargo = A.parse_target_cmd("cargo test -p core --test test_borrow test_two_mut")
    assert cargo == A.TargetSpec("cargo", "test_borrow.rs", "test_two_mut")
    # Loose selectors are refused rather than guessed.
    assert A.parse_target_cmd("node --test --test-name-pattern='vb loose title' a.test.cjs") is None
    assert A.parse_target_cmd("python -m pytest test_x.py -k 'union' -q") is None
    assert A.parse_target_cmd("true alpha") is None


CONFTEST = """
import json
import pytest
from pathlib import Path

FIXTURES = json.loads((Path(__file__).parent / "fixtures.json").read_text())
UNUSED = 1


@pytest.fixture(autouse=True)
def quarantine():
    yield


@pytest.fixture
def engine():
    return "engine"


def run_cli(lines):
    return lines


def assert_family(family):
    for case in FIXTURES[family]:
        assert run_cli(case["input"]) == case["expected"]
"""

OSS_TESTS = """
from conftest import assert_family


def test_overlong():
    assert_family("f2p_overlong")


def test_uses_fixture(engine):
    assert engine
"""


def test_python_source_follows_helpers_fixtures_and_case_inputs(tmp_path):
    tests = tmp_path / "tests"
    tests.mkdir()
    (tests / "conftest.py").write_text(CONFTEST)
    (tests / "oss_tests.py").write_text(OSS_TESTS)
    cases = [{"input": [f"W {i}", "D"], "expected": [f"SECRET-{i}"]} for i in range(400)]
    (tests / "fixtures.json").write_text(json.dumps({"f2p_overlong": cases, "other": []}))

    src = A.extract_test_source(tests, "python -m pytest oss_tests.py::test_overlong -q")
    assert src is not None
    assert src.code.startswith("def test_overlong():")
    helpers = "\n".join(code for _, _, code in src.helpers)
    assert "def assert_family" in helpers and "def run_cli" in helpers
    assert "FIXTURES = json.loads" in helpers and "import json" in helpers
    assert "quarantine" not in helpers and "UNUSED" not in helpers  # not reached
    assert all(rel == "conftest.py" for rel, _, _ in src.helpers)
    # Case inputs are shown, expected outputs never, and the blob is cut to budget.
    assert '["W 0", "D"]' in src.cases
    assert "SECRET" not in src.cases
    assert "of 400 cases shown" in src.cases
    assert len(src.cases) <= A.CASES_MAX_CHARS + 400

    fixture = A.extract_test_source(tests, "python -m pytest oss_tests.py::test_uses_fixture -q")
    assert fixture is not None
    assert any("def engine" in code for _, _, code in fixture.helpers)
    assert fixture.cases == ""
    assert A.extract_test_source(tests, "python -m pytest oss_tests.py::test_absent -q") is None
    assert A.extract_test_source(tests, "python -m pytest missing.py::test_overlong -q") is None


JS_TESTS = """const test = require('node:test')
const assert = require('node:assert')
const truncate = require('./functions/truncate')

const check = (cases) => {
  for (const [v, want] of cases) {
    assert.strictEqual(truncate(v), want, `truncate(${v}) is not {${want}}`)
  }
}

const unused = 3

test('vb truncate levels', () => {
  check([['1.2.3-foo', '1.2.3'], ['a)b', '}']])
})

test('vb other', () => {
  assert.ok(true)
})
"""

GO_TESTS = """package money

import "testing"

func cents(n int) []int {
	return []int{n, n}
}

func TestSplit(t *testing.T) {
	if got := cents(2); len(got) != 2 {
		t.Fatalf("got %v, want {}", got)
	}
}
"""

RUST_TESTS = """use core::get_pair_mut;
use utils::SplitVec;

fn helper<'a>(v: &'a mut SplitVec<i32>) -> char {
    let _ = v;
    '}'
}

#[test]
fn test_two_mut() {
    let mut v: SplitVec<i32> = SplitVec::new();
    let (a, _b) = get_pair_mut(&mut v, 0, 2);
    assert_eq!(helper(&mut v), '}');
    drop(a);
}
"""


def test_brace_sources_for_node_go_and_rust(tmp_path):
    tests = tmp_path / "tests"
    (tests / "money").mkdir(parents=True)
    (tests / "core" / "tests").mkdir(parents=True)
    (tests / "vb_truncate.test.js").write_text(JS_TESTS)
    (tests / "money" / "split_test.go").write_text(GO_TESTS)
    (tests / "core" / "tests" / "test_borrow.rs").write_text(RUST_TESTS)

    js = A.extract_test_source(
        tests, "node --test --test-name-pattern='^vb truncate levels$' vb_truncate.test.js"
    )
    assert js is not None
    assert js.code.startswith("test('vb truncate levels'") and js.code.endswith("})")
    assert "vb other" not in js.code
    helpers = "\n".join(code for _, _, code in js.helpers)
    assert "const check = (cases) =>" in helpers and helpers.rstrip().endswith("}")
    assert "require('./functions/truncate')" in helpers  # reached through check
    assert "unused" not in helpers
    assert A.render_test_sources(["levels"], {"levels": js}).count("```javascript") == 2

    go = A.extract_test_source(tests, "go test -run '^TestSplit$' ./...")
    assert go is not None and go.code.startswith("func TestSplit(") and go.code.endswith("}")
    assert [c.split("(")[0] for _, _, c in go.helpers] == ["func cents"]

    rust = A.extract_test_source(tests, "cargo test -p core --test test_borrow test_two_mut")
    assert rust is not None
    assert rust.code.startswith("#[test]\nfn test_two_mut()") and rust.code.endswith("}")
    rust_helpers = "\n".join(c for _, _, c in rust.helpers)
    assert "use core::get_pair_mut;" in rust_helpers and "fn helper<'a>" in rust_helpers

    assert A.extract_test_source(tests, "go test -run '^TestAbsent$' ./...") is None


def test_failing_test_drops_items_whose_sources_are_unknown(tmp_path):
    tasks = tmp_path / "tasks"
    runs = Runs(tmp_path / "runs-src")
    known = _task(tasks, "v9", "known-task")
    unknown = _task(tasks, "v9", "unknown-task")
    # One listed target's test function is missing from the hidden file.
    hidden = unknown / "tests" / "test_hidden.py"
    hidden.write_text(hidden.read_text().replace("def test_beta_case", "def test_renamed"))
    for task in (known, unknown):
        runs.add(task, _patch(f"{task.name}-g", 20), _all(TARGETS, ["gamma_case"]))
    items = A.build_failing_test(_ctx(tmp_path, options={"failing_prior_share": 1.0}))
    assert [i.source_unit for i in items] == ["known-task"]
    bare = A.build_failing_test(
        _ctx(tmp_path, options={"failing_prior_share": 1.0, "failing_sources": False})
    )
    assert sorted(i.source_unit for i in bare) == ["known-task", "unknown-task"]
    assert all("- alpha_case" in i.state for i in bare)


def test_failing_test_name_fallback_needs_identical_copies(tmp_path):
    tasks = tmp_path / "tasks"
    runs = Runs(tmp_path / "runs-fallback")
    first = _task(tasks, "v8", "copied-task")
    second = _task(tasks, "v9", "copied-task")
    runs.add(first, _patch("stale", 20), _all(TARGETS, ["gamma_case"]), exact_hash=False)
    opts = {"failing_prior_share": 1.0}
    assert len(A.build_failing_test(_ctx(tmp_path, options=opts))) == 1
    # A second copy whose test differs: the graded version is unknown, so drop.
    hidden = second / "tests" / "test_hidden.py"
    hidden.write_text(hidden.read_text().replace("len('gamma_case')", "len('changed')"))
    assert A.build_failing_test(_ctx(tmp_path, options=opts)) == []
