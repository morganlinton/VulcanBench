"""Software-pillar families mined from the run archive: patch-pair and failing-test.

Every answer comes from the hidden-test verifier (per-test exit codes recorded
in each run's ``summary.json``). Nothing is labelled by a model. The archive
keeps no hidden-test output, which is why the planned ``ci-failure`` family
was replaced by the generated ``incident-root-cause`` (DECISIONS, 2026-09-24).

Sources are every suite in the archive whose runs carry per-test verifier
results (v1, v2, v3, the python-1 pool, Frontier v4, v4-new and their screens),
not only Frontier v4. A run is resolved to the task definition it was graded
against by its recorded ``task_hash``; when the task has since been edited, a
task directory with the same id and the same fail-to-pass test names is used
instead, and anything else is skipped. ``source_unit`` is the task id.

Filters applied before any family sees a patch:

- unfinished runs, runs without per-test results, empty patches;
- runs the integrity audit marked contaminated, and anything under a
  ``*contaminated*`` or ``discarded*`` archive directory;
- likely gold recitations: at least 90% of a substantive gold patch's added
  lines (gold with 25 or more) reproduced verbatim;
- duplicates: identical patch text (``index`` lines and trailing whitespace
  ignored) on the same task keeps its first run; if two copies disagree on
  any test result the patch is dropped as flaky.

Item files embed task issues and agent patches: keep them private.
"""

from __future__ import annotations

import hashlib
import json
import os
import random
import re
from collections import Counter, defaultdict
from collections.abc import Iterable, Iterator, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from harness.tasks import load_task, task_hash
from harness.verdict.v2.items import MAX_STATE_CHARS, Item, choice_question, make_item
from harness.verdict.v2.registry import BuildContext, Builder

# Directories inside a run that never hold other runs; pruning them keeps the
# archive walk fast (workspaces can hold whole dependency trees).
_PRUNE_DIRS = frozenset(
    {
        "workspace",
        "regrade_workspace",
        "node_modules",
        ".git",
        "target",
        "grok-session",
        "zcode-session",
        "devin-session",
        "muse-session-logs",
    }
)
_EXCLUDED_ARCHIVE_PARTS = ("contaminated", "discarded")

RECITATION_MIN_GOLD_LINES = 25
RECITATION_SHARE = 0.9

# patch-pair construction
PAIR_MAX_SIZE_GAP = 0.2  # lines changed within 20% of the larger patch
PAIR_MAX_FILE_GAP = 1
PAIR_MAX_PER_TASK = 15
PAIR_MAX_PATCH_USES = 3
PAIR_BALANCE_SLACK = 2  # how far each size shortcut may drift from 50/50 while picking

# failing-test construction
FAILING_MAX_PER_TASK = 15
FAILING_MAX_PER_TEST = 3  # per task and failing test, so a task's usual culprit cannot dominate
# Any number of failing targets: the item shows one of them plus the targets that
# passed, so exactly one listed test fails. Capping at two left 70 items.
FAILING_MAX_FAILED = 10_000
FAILING_PRIOR_MAX_SHARE = 0.25  # items the leave-one-out task prior may answer correctly

_TEST_KEYWORDS = re.compile(r"\b(test|tests|assert\w*|expect\w*|should|raises|mock)\b", re.I)
_TOKEN = re.compile(r"[A-Z]?[a-z]+|[A-Z]+(?![a-z])|\d+")


# ---------------------------------------------------------------------------
# Archive loading


@dataclass(frozen=True)
class TaskInfo:
    task_id: str
    tasks_dir: str  # the suite directory under tasks/ (v1, v3, coding-intelligence-index-v4, ...)
    root: Path
    issue: str


@dataclass(frozen=True)
class Patch:
    task_id: str
    group: str  # task id plus graded task definition: only patches sharing it are compared
    suite: str
    tasks_dir: str
    run_dir: Path
    run_id: str
    model: str
    effort: str | None
    text: str
    sha: str
    targets: tuple[tuple[str, bool], ...]
    p2p: tuple[tuple[str, bool], ...]
    p2p_ok: bool
    lines: int
    files: int

    @property
    def all_pass(self) -> bool:
        return self.p2p_ok and all(ok for _, ok in self.targets)

    @property
    def failing(self) -> list[str]:
        return [name for name, ok in self.targets if not ok]

    @property
    def passing(self) -> list[str]:
        return [name for name, ok in self.targets if ok]

    @property
    def outcome(self) -> str:
        if self.all_pass:
            return "all_pass"
        if not self.p2p_ok:
            return "regression"
        return "partial_fix" if self.passing else "no_progress"

    def source(self) -> dict[str, Any]:
        return {
            "run_id": self.run_id,
            "model": self.model,
            "effort": self.effort,
            "suite": self.suite,
            "tasks_dir": self.tasks_dir,
            "patch_sha256": self.sha,
        }


@dataclass
class Archive:
    tasks: dict[str, TaskInfo] = field(default_factory=dict)  # group -> task
    patches: list[Patch] = field(default_factory=list)
    skipped: Counter[str] = field(default_factory=Counter)

    def by_group(self) -> dict[str, list[Patch]]:
        groups: dict[str, list[Patch]] = defaultdict(list)
        for p in self.patches:
            groups[p.group].append(p)
        return dict(groups)


def normalize_patch(text: str) -> str:
    return "\n".join(
        line.rstrip() for line in text.splitlines() if not line.startswith("index ")
    ).strip()


def patch_sha(text: str) -> str:
    return hashlib.sha256(normalize_patch(text).encode()).hexdigest()


def patch_size(text: str) -> tuple[int, int]:
    """(lines added plus removed, files touched) of a unified diff."""
    changed = 0
    files: set[str] = set()
    for line in text.splitlines():
        if line.startswith("+++ "):
            files.add(line[4:].strip())
        elif line.startswith("--- "):
            continue
        elif line.startswith(("+", "-")):
            changed += 1
    files.discard("/dev/null")
    if not files:
        files = set(re.findall(r"^diff --git a/(\S+)", text, re.M))
    return changed, len(files)


def added_lines(text: str) -> list[str]:
    """Substantive added lines: stripped, without blanks and lone punctuation."""
    out = []
    for line in text.splitlines():
        if line.startswith("+") and not line.startswith("+++"):
            body = line[1:].strip()
            if len(body) > 3:
                out.append(body)
    return out


def recites_gold(patch: str, gold_added: Sequence[str]) -> bool:
    if len(gold_added) < RECITATION_MIN_GOLD_LINES:
        return False
    have = Counter(added_lines(patch))
    hit = 0
    for line in gold_added:
        if have[line] > 0:
            have[line] -= 1
            hit += 1
    return hit / len(gold_added) >= RECITATION_SHARE


def iter_summaries(run_roots: Iterable[Path]) -> Iterator[tuple[Path, Path]]:
    """(archive root, summary path) for every run under the roots."""
    for root in run_roots:
        for dirpath, dirnames, filenames in os.walk(root):
            if "summary.json" in filenames:
                dirnames.clear()  # a run directory never nests another run
                yield Path(root), Path(dirpath) / "summary.json"
                continue
            dirnames[:] = sorted(d for d in dirnames if d not in _PRUNE_DIRS)


class _TaskResolver:
    """Find the task directory a run was graded against."""

    def __init__(self, tasks_roots: Sequence[Path]) -> None:
        self.tasks_roots = tuple(tasks_roots)
        self._candidates: dict[str, list[Path]] = {}
        self._hash: dict[Path, str | None] = {}
        self._targets: dict[Path, frozenset[str]] = {}
        self._gold: dict[Path, list[str]] = {}

    def candidates(self, task_id: str) -> list[Path]:
        if task_id not in self._candidates:
            found = []
            for root in self.tasks_roots:
                if (root / task_id / "metadata.json").is_file():
                    found.append(root / task_id)
                if root.is_dir():
                    for sub in sorted(p for p in root.iterdir() if p.is_dir()):
                        if (sub / task_id / "metadata.json").is_file():
                            found.append(sub / task_id)
            self._candidates[task_id] = found
        return self._candidates[task_id]

    def _task_hash(self, task_dir: Path) -> str | None:
        if task_dir not in self._hash:
            try:
                self._hash[task_dir] = task_hash(load_task(task_dir.name, task_dir.parent))
            except (OSError, ValueError):
                self._hash[task_dir] = None
        return self._hash[task_dir]

    def _target_names(self, task_dir: Path) -> frozenset[str]:
        if task_dir not in self._targets:
            try:
                meta = json.loads((task_dir / "metadata.json").read_text())
                spec = (meta.get("tests") or {}).get("fail_to_pass") or []
                self._targets[task_dir] = frozenset(str(t.get("name")) for t in spec)
            except (OSError, ValueError, AttributeError):
                self._targets[task_dir] = frozenset()
        return self._targets[task_dir]

    def gold_added(self, task_dir: Path) -> list[str]:
        if task_dir not in self._gold:
            gold = task_dir / "gold_patch.diff"
            self._gold[task_dir] = (
                added_lines(gold.read_text(errors="replace")) if gold.is_file() else []
            )
        return self._gold[task_dir]

    def resolve(
        self, task_id: str, recorded_hash: str | None, targets: Iterable[str]
    ) -> tuple[Path, bool] | None:
        """(task dir, exact hash match) or None when no directory fits the run."""
        cands = self.candidates(task_id)
        if recorded_hash:
            for cand in cands:
                if self._task_hash(cand) == recorded_hash:
                    return cand, True
        names = frozenset(targets)
        for cand in cands:
            if self._target_names(cand) == names and (cand / "issue.md").is_file():
                return cand, False
        return None


def _bool_map(raw: Any) -> tuple[tuple[str, bool], ...]:
    if not isinstance(raw, dict):
        return ()
    return tuple((str(k), bool(v)) for k, v in raw.items())


def _read_run(
    root: Path, summary_path: Path, resolver: _TaskResolver, skipped: Counter[str]
) -> tuple[Patch, TaskInfo] | None:
    """One run as a labelled patch plus its task, or None (with the reason counted)."""
    run_dir = summary_path.parent
    where = "/".join((root.name, *run_dir.relative_to(root).parts)).lower()
    reason = None
    summary: Any = None
    if any(part in where for part in _EXCLUDED_ARCHIVE_PARTS):
        reason = "excluded archive dir"
    else:
        try:
            summary = json.loads(summary_path.read_text())
        except (OSError, ValueError):
            reason = "unreadable summary"
    if reason is None and (not isinstance(summary, dict) or not summary.get("task_id")):
        reason = "not a run summary"
    if reason is not None:
        skipped[reason] += 1
        return None
    verifier = summary.get("verifier") or {}
    targets = _bool_map(verifier.get("fail_to_pass"))
    patch_path = run_dir / "final.patch"
    text = patch_path.read_text(errors="replace") if patch_path.is_file() else ""
    resolved = None
    if not summary.get("finished") or not targets:
        reason = "unfinished or no per-test results"
    elif (summary.get("integrity_audit") or {}).get("contaminated"):
        reason = "contaminated"
    elif not text.strip():
        reason = "empty patch"
    else:
        resolved = resolver.resolve(
            str(summary["task_id"]), summary.get("task_hash"), (n for n, _ in targets)
        )
        if resolved is None:
            reason = "task definition not found"
        elif recites_gold(text, resolver.gold_added(resolved[0])):
            reason = "gold recitation"
    if reason is not None or resolved is None:
        skipped[reason or "unresolved"] += 1
        return None
    task_id = str(summary["task_id"])
    task_dir, exact = resolved
    group = f"{task_id}:{summary.get('task_hash') if exact else 'names:' + str(task_dir)}"
    task = TaskInfo(
        task_id=task_id,
        tasks_dir=task_dir.parent.name,
        root=task_dir,
        issue=(task_dir / "issue.md").read_text(errors="replace").strip(),
    )
    lines, files = patch_size(text)
    patch = Patch(
        task_id=task_id,
        group=group,
        suite=str(summary.get("suite") or ""),
        tasks_dir=task_dir.parent.name,
        run_dir=run_dir,
        run_id=str(summary.get("run_id") or run_dir.name),
        model=str(summary.get("model") or ""),
        effort=(summary.get("effort") or {}).get("requested"),
        text=text,
        sha=patch_sha(text),
        targets=targets,
        p2p=_bool_map(verifier.get("pass_to_pass")),
        p2p_ok=verifier.get("pass_to_pass_ok") is not False,
        lines=lines,
        files=files,
    )
    return patch, task


def load_archive(run_roots: Sequence[Path], tasks_roots: Sequence[Path]) -> Archive:
    """Every clean, distinct, labelled patch in the archive, with its task."""
    archive = Archive()
    resolver = _TaskResolver(tasks_roots)
    first: dict[tuple[str, str], Patch] = {}  # (task id, patch sha) -> first run of that patch
    flaky: set[tuple[str, str]] = set()
    for root, summary_path in iter_summaries(run_roots):
        read = _read_run(root, summary_path, resolver, archive.skipped)
        if read is None:
            continue
        patch, task = read
        key = (patch.task_id, patch.sha)
        prior = first.get(key)
        if prior is not None:
            archive.skipped["duplicate patch"] += 1
            if (prior.targets, prior.p2p_ok) != (patch.targets, patch.p2p_ok):
                flaky.add(key)
            continue
        first[key] = patch
        archive.tasks.setdefault(patch.group, task)
    archive.patches = [p for key, p in first.items() if key not in flaky]
    archive.skipped["flaky duplicate (dropped)"] += len(flaky)
    return archive


_ARCHIVE_CACHE: dict[tuple[tuple[Path, ...], tuple[Path, ...]], Archive] = {}


def archive_for(ctx: BuildContext) -> Archive:
    key = (tuple(ctx.run_roots), tuple(ctx.tasks_roots))
    if key not in _ARCHIVE_CACHE:
        _ARCHIVE_CACHE[key] = load_archive(ctx.run_roots, ctx.tasks_roots)
    return _ARCHIVE_CACHE[key]


# ---------------------------------------------------------------------------
# Shared helpers


def _order_key(seed: int, family: str, key: str) -> str:
    return hashlib.sha256(f"{seed}:{family}:{key}".encode()).hexdigest()


def _fenced(patch: str) -> str:
    return f"```diff\n{patch.rstrip()}\n```"


def tokens(text: str) -> set[str]:
    return {t.lower() for t in _TOKEN.findall(text) if len(t) >= 3}


def _pick(scores: Sequence[float], labels: Sequence[str]) -> str:
    """Highest score; ties go to the earliest label (labels are in shown order)."""
    best = max(range(len(labels)), key=lambda i: (scores[i], -i))
    return labels[best]


def _balanced_positions(rng: random.Random, n: int) -> list[bool]:
    firsts = [True] * (n // 2) + [False] * (n - n // 2)
    rng.shuffle(firsts)
    return firsts


# ---------------------------------------------------------------------------
# patch-pair

PAIR_INSTRUCTIONS = (
    "Exactly one of these two patches makes every hidden target test pass without "
    "breaking an existing test. Which one?"
)


def keyword_count(patch: str) -> int:
    return sum(
        len(_TEST_KEYWORDS.findall(line[1:]))
        for line in patch.splitlines()
        if line.startswith("+") and not line.startswith("+++")
    )


def _sign(a: float, b: float) -> int:
    return (a > b) - (a < b)


def size_matched(a: Patch, b: Patch) -> bool:
    lo, hi = sorted((a.lines, b.lines))
    return (
        lo > 0
        and hi - lo <= PAIR_MAX_SIZE_GAP * hi
        and abs(a.files - b.files) <= (PAIR_MAX_FILE_GAP)
    )


def pair_shortcuts(first: Patch, second: Patch) -> dict[str, str]:
    """What each surface baseline answers; ties go to A."""

    def larger(x: float, y: float) -> str:
        return "B" if y > x else "A"

    return {
        "larger-patch": larger(first.lines, second.lines),
        "smaller-patch": larger(-first.lines, -second.lines),
        "more-files": larger(first.files, second.files),
        "more-test-keywords": larger(keyword_count(first.text), keyword_count(second.text)),
    }


def build_patch_pair(ctx: BuildContext) -> list[Item]:
    archive = archive_for(ctx)
    family = "patch-pair"
    candidates: list[tuple[str, Patch, Patch]] = []
    for group, patches in archive.by_group().items():
        issue = archive.tasks[group].issue
        good = [p for p in patches if p.all_pass]
        bad = [p for p in patches if not p.all_pass]
        for g in good:
            for b in bad:
                if not size_matched(g, b):
                    continue
                if len(issue) + len(g.text) + len(b.text) + 200 > MAX_STATE_CHARS:
                    continue
                candidates.append((_order_key(ctx.seed, family, g.sha + b.sha), g, b))
    candidates.sort(key=lambda c: c[0])

    per_task: Counter[str] = Counter()
    uses: Counter[str] = Counter()
    drift = [0, 0, 0]  # larger, more files, more keywords: +1 when the passing patch has more
    chosen: list[tuple[Patch, Patch]] = []
    for _, g, b in candidates:
        if len(chosen) >= ctx.per_family:
            break
        if per_task[g.task_id] >= PAIR_MAX_PER_TASK:
            continue
        if uses[g.sha] >= PAIR_MAX_PATCH_USES or uses[b.sha] >= PAIR_MAX_PATCH_USES:
            continue
        signs = [
            _sign(g.lines, b.lines),
            _sign(g.files, b.files),
            _sign(keyword_count(g.text), keyword_count(b.text)),
        ]
        if any(
            abs(d + s) > PAIR_BALANCE_SLACK and abs(d + s) > abs(d)
            for d, s in zip(drift, signs, strict=True)
        ):
            continue
        drift = [d + s for d, s in zip(drift, signs, strict=True)]
        per_task[g.task_id] += 1
        uses[g.sha] += 1
        uses[b.sha] += 1
        chosen.append((g, b))

    if len(chosen) % 2:
        chosen.pop()  # keep the correct position exactly balanced
    rng = random.Random(f"{ctx.seed}:{family}:positions")
    items = []
    for (g, b), good_first in zip(chosen, _balanced_positions(rng, len(chosen)), strict=True):
        first, second = (g, b) if good_first else (b, g)
        issue = archive.tasks[g.group].issue
        state = (
            f"## Issue\n\n{issue}\n\n## Patch A\n\n{_fenced(first.text)}\n\n"
            f"## Patch B\n\n{_fenced(second.text)}\n"
        )
        items.append(
            make_item(
                family=family,
                key=f"{g.task_id}:{first.sha}:{second.sha}",
                source_unit=g.task_id,
                state=state,
                question=choice_question(PAIR_INSTRUCTIONS, {"A": "Patch A", "B": "Patch B"}),
                answer="A" if good_first else "B",
                reference="verifier",
                shortcuts=pair_shortcuts(first, second),
                source={
                    "passing": g.source(),
                    "failing": {**b.source(), "outcome": b.outcome},
                    "lines": [first.lines, second.lines],
                    "files": [first.files, second.files],
                },
            )
        )
    return items


# ---------------------------------------------------------------------------
# failing-test

FAILING_ONE_INSTRUCTIONS = (
    "With this patch applied, every listed hidden target test passes except exactly one. "
    "Which test fails?"
)
FAILING_SUBSET_INSTRUCTIONS = (
    "With this patch applied, exactly one of the listed hidden target tests fails and the "
    "other listed tests pass. Which test fails?"
)


def failing_shortcuts(
    options: Sequence[str], patch: str, issue: str, prior: Counter[str]
) -> dict[str, str]:
    patch_tokens, issue_tokens = tokens(patch), tokens(issue)
    return {
        "patch-overlap": _pick([len(tokens(o) & patch_tokens) for o in options], options),
        "issue-overlap": _pick([len(tokens(o) & issue_tokens) for o in options], options),
        "first-listed": options[0],
        "last-listed": options[-1],
        "longest-name": _pick([len(o) for o in options], options),
        # Leave-one-out: the target that fails most often in the task's other patches.
        "task-prior": _pick([prior[o] for o in options], options),
    }


@dataclass(frozen=True)
class _FailingCandidate:
    order: str
    patch: Patch
    options: tuple[str, ...]  # shown order
    answer: str
    shortcuts: dict[str, str]

    @property
    def prior_hit(self) -> bool:
        return self.shortcuts["task-prior"] == self.answer


def _failing_candidates(ctx: BuildContext, archive: Archive) -> list[_FailingCandidate]:
    family = "failing-test"
    max_failed = int((ctx.options or {}).get("failing_max_failed", FAILING_MAX_FAILED))
    out = []
    for group, patches in archive.by_group().items():
        issue = archive.tasks[group].issue
        for p in patches:
            failing, names = p.failing, [n for n, _ in p.targets]
            if not p.passing or not failing or len(failing) > max_failed:
                continue
            prior: Counter[str] = Counter()
            for other in patches:
                if other.sha != p.sha:
                    prior.update(other.failing)
            # With two failing targets, show the one other patches fail less often
            # (ties by hash) and drop the rest, so the task's usual culprit is not
            # always the answer.
            keep = min(failing, key=lambda n: (prior[n], _order_key(ctx.seed, family, p.sha + n)))
            options = sorted(n for n in names if n == keep or n not in failing)
            random.Random(f"{ctx.seed}:{family}:{p.sha}").shuffle(options)
            out.append(
                _FailingCandidate(
                    order=_order_key(ctx.seed, family, p.sha),
                    patch=p,
                    options=tuple(options),
                    answer=keep,
                    shortcuts=failing_shortcuts(options, p.text, issue, prior),
                )
            )
    return sorted(out, key=lambda c: c.order)


def build_failing_test(ctx: BuildContext) -> list[Item]:
    """Which listed hidden target fails.

    Selection runs in two passes: first every candidate whose answer is not the
    task's most frequent failer among the options, then candidates where it is,
    only while they stay under ``FAILING_PRIOR_MAX_SHARE`` of the family. Without
    the cap the leave-one-out task prior answers about 70% of items, because
    most failing patches miss the same hard target.
    """
    archive = archive_for(ctx)
    family = "failing-test"
    max_prior = float((ctx.options or {}).get("failing_prior_share", FAILING_PRIOR_MAX_SHARE))
    candidates = _failing_candidates(ctx, archive)
    per_task: Counter[str] = Counter()
    per_test: Counter[tuple[str, str]] = Counter()
    chosen: list[_FailingCandidate] = []
    for prior_pass in (False, True):
        for cand in candidates:
            p = cand.patch
            if len(chosen) >= ctx.per_family or cand.prior_hit != prior_pass:
                continue
            hits = sum(c.prior_hit for c in chosen)
            if prior_pass and hits + 1 > max_prior * (len(chosen) + 1):
                break
            if per_task[p.task_id] >= FAILING_MAX_PER_TASK:
                continue
            if per_test[(p.task_id, cand.answer)] >= FAILING_MAX_PER_TEST:
                continue
            chosen.append(cand)
            per_task[p.task_id] += 1
            per_test[(p.task_id, cand.answer)] += 1

    items: list[Item] = []
    for cand in sorted(chosen, key=lambda c: c.order):
        p = cand.patch
        issue = archive.tasks[p.group].issue
        listing = "\n".join(f"- {o}" for o in cand.options)
        state = (
            f"## Issue\n\n{issue}\n\n## Candidate patch\n\n{_fenced(p.text)}\n\n"
            f"## Hidden target tests\n\n{listing}\n"
        )
        if len(state) > MAX_STATE_CHARS:
            continue
        subset = len(cand.options) < len(p.targets)
        items.append(
            make_item(
                family=family,
                key=f"{p.task_id}:{p.sha}",
                source_unit=p.task_id,
                state=state,
                question=choice_question(
                    FAILING_SUBSET_INSTRUCTIONS if subset else FAILING_ONE_INSTRUCTIONS,
                    dict.fromkeys(cand.options),
                ),
                answer=cand.answer,
                reference="verifier",
                shortcuts=cand.shortcuts,
                source={**p.source(), "failing_targets": len(p.failing), "subset": subset},
            )
        )
    return items


BUILDERS: dict[str, Builder] = {
    "patch-pair": build_patch_pair,
    "failing-test": build_failing_test,
}
