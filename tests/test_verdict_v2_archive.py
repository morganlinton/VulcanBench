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
    (task / "tests" / "test_hidden.py").write_text("def test_x():\n    pass\n")
    (task / "issue.md").write_text(f"Issue for {task_id}: alpha and beta behave wrongly.")
    (task / "gold_patch.diff").write_text(gold)
    meta = {
        "id": task_id,
        "tests": {
            "fail_to_pass": [{"name": n, "cmd": f"true {n}"} for n in targets],
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
    assert all(f"- {name}" in single.state for name in TARGETS)

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
