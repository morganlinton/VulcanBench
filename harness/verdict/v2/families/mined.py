"""Mined software families: fix-file, vuln-pair, weakness-class, semver-impact.

Every answer here comes from a merged fix, a published advisory or a
deterministic tool run over real code, never from a model. Mining and
building are split: ``scripts/verdict-v2/mine_sources.py`` does all network
work (read-only ``gh`` calls) and writes raw records to the private cache
``verdict-v2-items/mined/``; the builders below read only that cache, so a
build is offline and deterministic given the cache and ``ctx.seed``. A
missing cache yields no items, never a crash.

Sources must be merged or published on or after ``CUTOFF``. For the two
advisory families the fix commit must also date from after it by default
(option ``fix_after_cutoff``), because many advisories published after the
cutoff describe fixes committed before it.

- ``fix-file``: a merged bug-fix PR that changes exactly one non-test source
  file and closes an issue. State is the issue plus the repository's source
  paths at the base commit; options are the fixed file and hard distractors
  from the same directory. Issues that name the file path, file name or stem
  verbatim are excluded.
- ``vuln-pair``: the function a security fix changed, before and after,
  with comments that differ between versions or mention the fix removed.
- ``weakness-class``: the vulnerable function and its fix diff, labelled by
  the advisory CWE collapsed to ten families.
- ``semver-impact``: a merged Python PR's package diff, labelled patch,
  minor or major by the public-API differ in this module (reference
  ``tool``), because neither griffe nor cargo-semver-checks is installed.
"""

from __future__ import annotations

import ast
import difflib
import gzip
import io
import json
import math
import random
import re
import textwrap
import tokenize
from collections import Counter, defaultdict
from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath
from typing import Any

from harness.verdict.v2.items import (
    MAX_STATE_CHARS,
    Item,
    choice_question,
    make_item,
    score_question,
)
from harness.verdict.v2.registry import BuildContext

CUTOFF = "2026-06-01"
MINED_DIR = Path("verdict-v2-items") / "mined"
FIX_FILE_CACHE = "fix_file.jsonl"
ADVISORY_CACHE = "advisories.jsonl"
SEMVER_CACHE = "semver.jsonl"
TREES_DIR = "trees"

# ---------------------------------------------------------------------------
# Paths and languages
# ---------------------------------------------------------------------------

LANG_BY_EXT = {
    ".py": "python",
    ".go": "go",
    ".rs": "rust",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".mts": "typescript",
    ".cts": "typescript",
    ".js": "javascript",
    ".jsx": "javascript",
    ".mjs": "javascript",
    ".cjs": "javascript",
}
LANG_NAMES = {
    "python": "Python",
    "go": "Go",
    "rust": "Rust",
    "typescript": "TypeScript",
    "javascript": "JavaScript",
}
NON_SOURCE_DIRS = frozenset(
    {
        "test",
        "tests",
        "testing",
        "__tests__",
        "__test__",
        "spec",
        "specs",
        "testdata",
        "test-data",
        "test_data",
        "fixtures",
        "__fixtures__",
        "__mocks__",
        "e2e",
        "examples",
        "example",
        "docs",
        "doc",
        "benchmarks",
        "benchmark",
        "benches",
        "bench",
        "scripts",
        "vendor",
        "node_modules",
        "third_party",
        "third-party",
        "dist",
        "build",
        "target",
        ".github",
        "site-packages",
        "playground",
        "demo",
        "demos",
        "samples",
        "sample",
    }
)
_TEST_NAME = re.compile(
    r"(^tests?[-_.]|^conftest\.py$|_test\.py$|_test\.go$|_tests?\.rs$|^tests?\.rs$"
    r"|[-_.](test|spec|bench)\.[cm]?[jt]sx?$)"
)
_GENERATED_NAME = re.compile(
    r"(\.d\.[cm]?ts$|\.min\.js$|\.pb\.go$|_generated\.go$|\.gen\.go$|\.bundle\.js$|_pb2\.py$)"
)


def is_test_path(path: str) -> bool:
    """True for tests, fixtures, docs, examples, vendored and build output."""
    parts = path.lower().split("/")
    if any(part in NON_SOURCE_DIRS for part in parts[:-1]):
        return True
    return bool(_TEST_NAME.search(parts[-1]))


def source_lang(path: str) -> str | None:
    """The language of a non-test, hand-written source file, else None."""
    lang = LANG_BY_EXT.get(PurePosixPath(path).suffix.lower())
    if lang is None or is_test_path(path) or _GENERATED_NAME.search(path.lower()):
        return None
    return lang


def lang_family(lang: str) -> str:
    return "js" if lang in {"typescript", "javascript"} else lang


NON_PACKAGE_ROOTS = frozenset(
    {"benchmarking", "maint_tools", "tools", "asv_bench", "ci", "dev", "devtools", "utils_dev"}
)


def python_module_parts(path: str) -> list[str] | None:
    """Module path components of a package file (``src/pkg/mod.py`` gives pkg, mod)."""
    if source_lang(path) != "python":
        return None
    parts = path[: -len(".py")].split("/")
    if len(parts) > 2 and parts[0] in {"libs", "packages"}:
        parts = parts[2:]
    while len(parts) > 1 and parts[0] in {"src", "lib", "python"}:
        parts = parts[1:]
    if len(parts) < 2 or not all(part.isidentifier() for part in parts):
        return None
    if parts[0] in NON_PACKAGE_ROOTS:
        return None
    return parts


def is_public_module(path: str) -> bool:
    """Package file whose module path has no underscore-prefixed or ``internal`` component."""
    parts = python_module_parts(path)
    if parts is None:
        return False
    return not any(
        (part.startswith("_") and part != "__init__") or part == "internal" for part in parts
    )


# ---------------------------------------------------------------------------
# Cache access
# ---------------------------------------------------------------------------


def mined_dir(ctx: BuildContext) -> Path:
    custom = (ctx.options or {}).get("mined_dir")
    return Path(custom) if custom else ctx.repo / MINED_DIR


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    """Records from a JSONL cache; a torn last line from an interrupted run is skipped."""
    if not path.exists():
        return []
    records = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            records.append(value)
    return records


def tree_path(root: Path, repo: str, sha: str) -> Path:
    return root / TREES_DIR / f"{repo.replace('/', '__')}__{sha}.json.gz"


def load_tree(root: Path, repo: str, sha: str) -> dict[str, int] | None:
    """Source paths and blob sizes at a commit, as the miner stored them."""
    path = tree_path(root, repo, sha)
    if not path.exists():
        return None
    try:
        with gzip.open(path, "rt", encoding="utf-8") as fh:
            data = json.load(fh)
    except (OSError, json.JSONDecodeError):
        return None
    if data.get("truncated"):
        return None
    return {str(p): int(s) for p, s in data.get("files", [])}


def _on_or_after_cutoff(date: str | None) -> bool:
    return bool(date) and str(date)[:10] >= CUTOFF


# ---------------------------------------------------------------------------
# Balancing helpers
# ---------------------------------------------------------------------------


TIE = "tie"


@dataclass
class Candidate:
    """A would-be item before balancing: its answer, unit and shortcut guesses."""

    key: str
    unit: str
    answer: str
    shortcuts: dict[str, str]
    payload: dict[str, Any] = field(default_factory=dict)


def cap_shortcuts(
    candidates: list[Candidate],
    cap: float,
    rng: random.Random,
    *,
    constants: Sequence[str] = (),
    ignore: Sequence[str] = (),
) -> list[Candidate]:
    """Drop items until every shortcut (and every constant answer) is right at most ``cap``.

    Greedy: while some baseline is over the cap, remove an item that the most
    over-cap baselines get right. ``constants`` adds "always answer X"
    baselines, which balances the answer distribution at the same time.
    """
    pool = list(candidates)
    rng.shuffle(pool)
    names = sorted({n for c in pool for n in c.shortcuts} - set(ignore))

    def right(c: Candidate) -> dict[str, float]:
        # A shortcut that ties answers by position, which is balanced later: half right.
        hits = {n: 1.0 for n in names if c.shortcuts.get(n) == c.answer}
        hits.update({n: 0.5 for n in names if c.shortcuts.get(n) == TIE})
        hits.update({f"const:{k}": 1.0 for k in constants if c.answer == k})
        return hits

    hits = [right(c) for c in pool]
    counts: defaultdict[str, float] = defaultdict(float)
    for hs in hits:
        for name, weight in hs.items():
            counts[name] += weight
    alive = [True] * len(pool)
    n = len(pool)
    while n:
        over = {name for name, k in counts.items() if k / n > cap + 1e-9}
        if not over:
            break
        best, best_score = -1, 0.0
        for index, ok in enumerate(alive):
            if not ok:
                continue
            score = sum(w for name, w in hits[index].items() if name in over)
            if score > best_score:
                best, best_score = index, score
        if best < 0:
            break
        alive[best] = False
        for name, weight in hits[best].items():
            counts[name] -= weight
        n -= 1
    kept = [c for c, ok in zip(pool, alive, strict=True) if ok]
    kept.sort(key=lambda c: c.key)
    return kept


def spread_units(candidates: list[Candidate], limit: int, rng: random.Random) -> list[Candidate]:
    """Up to ``limit`` candidates, taken round-robin over source units."""
    by_unit: dict[str, list[Candidate]] = defaultdict(list)
    for c in sorted(candidates, key=lambda c: c.key):
        by_unit[c.unit].append(c)
    for group in by_unit.values():
        rng.shuffle(group)
    units = sorted(by_unit)
    rng.shuffle(units)
    chosen: list[Candidate] = []
    depth = 0
    while len(chosen) < limit and any(len(by_unit[u]) > depth for u in units):
        for unit in units:
            if len(by_unit[unit]) > depth and len(chosen) < limit:
                chosen.append(by_unit[unit][depth])
        depth += 1
    chosen.sort(key=lambda c: c.key)
    return chosen


def argmax_label(scores: dict[str, float], order: Sequence[str]) -> str:
    """Label with the highest score; ties go to the first in ``order``."""
    best = max(scores[label] for label in order)
    return next(label for label in order if scores[label] == best)


# ---------------------------------------------------------------------------
# Tokens
# ---------------------------------------------------------------------------

_CAMEL = re.compile(r"(?<=[a-z0-9])(?=[A-Z])")
_PATH_STOP = frozenset(
    {"src", "lib", "pkg", "internal", "index", "mod", "main", "init", "the", "and", "for"}
)


def word_tokens(text: str) -> set[str]:
    """Lower-case word pieces, split on case changes, of three or more characters."""
    out = set()
    for word in re.findall(r"[A-Za-z][A-Za-z0-9]*", text):
        for piece in _CAMEL.sub(" ", word).split():
            if len(piece) >= 3:
                out.add(piece.lower())
    return out


def path_tokens(path: str) -> set[str]:
    stem = re.sub(r"\.[A-Za-z0-9]+$", "", path)
    return word_tokens(stem.replace("_", " ").replace("-", " ")) - _PATH_STOP


# ---------------------------------------------------------------------------
# fix-file
# ---------------------------------------------------------------------------

FIX_FILE_MIN_OPTIONS = 10
FIX_FILE_MAX_OPTIONS = 20
FIX_FILE_POOL = 40
PATHS_BUDGET = 60_000
ISSUE_BODY_BUDGET = 6_000
FIX_FILE_PER_REPO = 15


def clean_issue_body(body: str) -> str:
    text = re.sub(r"<!--.*?-->", "", body or "", flags=re.S)
    text = re.sub(r"\n{3,}", "\n\n", text).strip()
    return text


def names_file(text: str, path: str) -> bool:
    """True when the text names the path, file name or stem verbatim."""
    lowered = text.lower()
    name = PurePosixPath(path).name.lower()
    stem = PurePosixPath(path).stem.lower()
    if path.lower() in lowered or name in lowered:
        return True
    return bool(re.search(rf"(?<![A-Za-z0-9]){re.escape(stem)}(?![A-Za-z0-9])", lowered))


def path_distance(a: str, b: str) -> int:
    da, db = a.split("/")[:-1], b.split("/")[:-1]
    common = 0
    for x, y in zip(da, db, strict=False):
        if x != y:
            break
        common += 1
    return len(da) + len(db) - 2 * common


def listed_paths(files: dict[str, int], fixed: str) -> tuple[list[str], str]:
    """Source paths for the state, narrowed to the fixed file's subtree if too long."""
    paths = sorted(files)
    prefix = ""
    parts = fixed.split("/")[:-1]
    depth = 0
    while sum(len(p) + 1 for p in paths) > PATHS_BUDGET and depth < len(parts):
        depth += 1
        prefix = "/".join(parts[:depth]) + "/"
        paths = [p for p in sorted(files) if p.startswith(prefix)]
    return paths, prefix


def fix_file_candidate(
    record: dict[str, Any], files: dict[str, int], rng: random.Random
) -> Candidate | None:
    """One fix-file candidate from a mined PR and its base tree, or None."""
    fixed = str(record["fixed_file"])
    lang = source_lang(fixed)
    if lang is None or fixed not in files:
        return None
    title = str(record.get("issue_title") or "")
    body = clean_issue_body(str(record.get("issue_body") or ""))
    if names_file(f"{title}\n{body}", fixed):
        return None
    family = lang_family(lang)
    source = {p: s for p, s in files.items() if source_lang(p) is not None}
    listed, prefix = listed_paths(source, fixed)
    if fixed not in listed:
        return None
    same_family = [p for p in listed if p != fixed and lang_family(source_lang(p) or "") == family]
    ties = {p: rng.random() for p in same_family}
    ranked = sorted(same_family, key=lambda p: (path_distance(p, fixed), ties[p]))
    pool = ranked[:FIX_FILE_POOL]
    n_options = rng.randint(FIX_FILE_MIN_OPTIONS, FIX_FILE_MAX_OPTIONS)
    need = min(n_options - 1, len(pool))
    if need < FIX_FILE_MIN_OPTIONS - 1:
        return None
    issue_tokens = word_tokens(f"{title}\n{body}")

    def overlap(p: str) -> float:
        return float(len(path_tokens(p) & issue_tokens))

    def size(p: str) -> float:
        return float(source.get(p, 0))

    def shortness(p: str) -> float:
        return -float(len(p))

    metrics: dict[str, Callable[[str], float]] = {
        "token_overlap": overlap,
        "largest_file": size,
        "shortest_path": shortness,
    }
    chosen: list[str] = []
    for metric in metrics.values():
        # With probability 1/n the answer is allowed to win this metric; otherwise
        # at least one distractor beats it, so every shortcut lands near chance.
        if rng.random() < 1 / (need + 1):
            continue
        if any(metric(c) > metric(fixed) for c in chosen):
            continue
        # The nearest file that beats the answer, looking past the pool if needed.
        beaters = [p for p in ranked if p not in chosen and metric(p) > metric(fixed)]
        if beaters:
            chosen.append(beaters[0])
    for p in pool:
        if len(chosen) >= need:
            break
        if p not in chosen:
            chosen.append(p)
    distractors = chosen[:need]
    mapping, answer = label_map(rng, fixed, distractors)
    order = list(mapping)
    shortcuts = {
        name: argmax_label({lab: fn(mapping[lab]) for lab in order}, order)
        for name, fn in metrics.items()
    }
    return Candidate(
        key=f"{record['repo']}#{record['number']}",
        unit=str(record["repo"]),
        answer=answer,
        shortcuts=shortcuts,
        payload={
            "record": record,
            "mapping": mapping,
            "listed": listed,
            "prefix": prefix,
            "lang": lang,
            "body": body,
            "title": title,
        },
    )


def label_map(
    rng: random.Random, correct: str, distractors: Sequence[str]
) -> tuple[dict[str, str], str]:
    """Letter labels in random order mapped to texts; returns (map, correct label)."""
    texts = [correct, *distractors]
    if len(set(texts)) != len(texts):
        raise ValueError("options must be distinct")
    rng.shuffle(texts)
    mapping = {chr(ord("A") + i): text for i, text in enumerate(texts)}
    return mapping, next(label for label, text in mapping.items() if text == correct)


def build_fix_file(ctx: BuildContext) -> list[Item]:
    root = mined_dir(ctx)
    records = read_jsonl(root / FIX_FILE_CACHE)
    records = [r for r in records if r.get("status") == "ok"]
    records.sort(key=lambda r: (str(r.get("repo")), int(r.get("number", 0))))
    candidates: list[Candidate] = []
    seen: set[str] = set()
    for record in records:
        if not _on_or_after_cutoff(record.get("merged_at")):
            continue
        key = f"{record['repo']}#{record['number']}"
        if key in seen:
            continue
        seen.add(key)
        files = load_tree(root, str(record["repo"]), str(record["base_sha"]))
        if files is None:
            continue
        rng = random.Random(f"{ctx.seed}:fix-file:{key}")
        cand = fix_file_candidate(record, files, rng)
        if cand is None:
            continue
        candidates.append(cand)
    pick_rng = random.Random(f"{ctx.seed}:fix-file:pick")
    per_repo = max(FIX_FILE_PER_REPO, math.ceil(ctx.per_family / 20))
    capped = _cap_per_unit(candidates, per_repo, pick_rng)
    chosen = spread_units(capped, ctx.per_family, pick_rng)
    return [_fix_file_item(c) for c in chosen]


def _cap_per_unit(candidates: list[Candidate], cap: int, rng: random.Random) -> list[Candidate]:
    by_unit: dict[str, list[Candidate]] = defaultdict(list)
    for c in candidates:
        by_unit[c.unit].append(c)
    out: list[Candidate] = []
    for unit in sorted(by_unit):
        group = sorted(by_unit[unit], key=lambda c: c.key)
        rng.shuffle(group)
        out.extend(group[:cap])
    return out


def _fix_file_item(c: Candidate) -> Item:
    p = c.payload
    record = p["record"]
    lang = LANG_NAMES[p["lang"]]
    body = p["body"]
    if len(body) > ISSUE_BODY_BUDGET:
        body = body[:ISSUE_BODY_BUDGET] + "\n[issue text truncated]"
    scope = f" under {p['prefix']}" if p["prefix"] else ""
    state = (
        f"Repository: {record['repo']} (mainly {lang})\n\n"
        f"Issue #{record['issue_number']}: {p['title']}\n\n{body}\n\n"
        f"Source files in the repository at the fix's base commit{scope} "
        f"({len(p['listed'])} paths, tests excluded):\n" + "\n".join(p["listed"])
    )
    if len(state) > MAX_STATE_CHARS:
        state = state[:MAX_STATE_CHARS]
    mapping: dict[str, str] = p["mapping"]
    question = choice_question(
        "A maintainer fixed this issue with a merged pull request that changed exactly one "
        "non-test source file. Which file did the fix change?",
        {label: text for label, text in mapping.items()},
    )
    return make_item(
        family="fix-file",
        key=c.key,
        source_unit=c.unit,
        state=state,
        question=question,
        answer=c.answer,
        reference="merged-fix",
        shortcuts=c.shortcuts,
        source={
            "repo": record["repo"],
            "pr": record["number"],
            "issue": record["issue_number"],
            "merged_at": record["merged_at"],
            "base_sha": record["base_sha"],
        },
    )


# ---------------------------------------------------------------------------
# Comments: scanning, stripping
# ---------------------------------------------------------------------------

SENSITIVE = re.compile(
    r"(?i)(cve-\d|ghsa-|secur|vulnerab|exploit|attack|malicious|advisory|\bcwe\b|\bfix"
    r"|\bpatch|harden|bypass|inject|travers|\bxss\b|\bssrf\b|\bcsrf\b|redos|denial|\bdos\b"
    r"|overflow|untrusted|sanitiz|escap|prevent|mitigat|unsafe input|attacker)"
)


@dataclass(frozen=True)
class Span:
    start: int
    end: int
    text: str


def _norm_comment(text: str) -> str:
    return re.sub(r"\s+", " ", re.sub(r"^[\s#/*!\"']+|[\s*/\"']+$", "", text)).strip().lower()


def scan_clike(code: str, lang: str) -> tuple[str, list[Span]]:  # noqa: PLR0912, PLR0915
    """Mask strings and comments (same length, newlines kept) and list comment spans."""
    out = list(code)
    spans: list[Span] = []
    i, n = 0, len(code)

    def mask(a: int, b: int) -> None:
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        ch = code[i]
        nxt = code[i + 1] if i + 1 < n else ""
        if ch == "/" and nxt == "/":
            end = code.find("\n", i)
            end = n if end < 0 else end
            spans.append(Span(i, end, code[i:end]))
            mask(i, end)
            i = end
        elif ch == "/" and nxt == "*":
            depth, j = 1, i + 2
            while j < n and depth:
                if code.startswith("*/", j):
                    depth -= 1
                    j += 2
                elif lang == "rust" and code.startswith("/*", j):
                    depth += 1
                    j += 2
                else:
                    j += 1
            spans.append(Span(i, j, code[i:j]))
            mask(i, j)
            i = j
        elif (
            lang == "rust"
            and ch == "r"
            and re.match(r'r#*"', code[i:])
            and (i == 0 or not (code[i - 1].isalnum() or code[i - 1] == "_"))
        ):
            m = re.match(r'r(#*)"', code[i:])
            assert m is not None
            closer = '"' + m.group(1)
            end = code.find(closer, i + len(m.group(0)))
            end = n if end < 0 else end + len(closer)
            mask(i + len(m.group(0)), max(i + len(m.group(0)), end - len(closer)))
            i = end
        elif ch == '"' or (ch == "'" and lang in {"typescript", "javascript"}):
            j = i + 1
            while j < n and code[j] != ch and code[j] != "\n":
                j += 2 if code[j] == "\\" else 1
            mask(i + 1, min(j, n))
            i = j + 1
        elif ch == "'" and lang in {"go", "rust"}:
            m = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\x[0-9a-fA-F]{2}|\\.|[^\\'\n])'", code[i:])
            if m:
                mask(i + 1, i + len(m.group(0)) - 1)
                i += len(m.group(0))
            else:
                i += 1  # a Rust lifetime
        elif ch == "`":
            j = i + 1
            while j < n and code[j] != "`":
                j += 2 if (code[j] == "\\" and lang != "go") else 1
            mask(i + 1, min(j, n))
            i = j + 1
        else:
            i += 1
    return "".join(out), spans


def python_comment_spans(code: str) -> list[Span] | None:
    """Comments and docstrings of a (dedented) Python snippet, or None if it will not parse."""
    try:
        tree = ast.parse(code)
        tokens = list(tokenize.generate_tokens(io.StringIO(code).readline))
    except (SyntaxError, tokenize.TokenError, IndentationError, ValueError):
        return None
    starts = [0]
    for line in code.splitlines(keepends=True):
        starts.append(starts[-1] + len(line))

    def offset(row: int, col: int) -> int:
        return starts[row - 1] + col

    spans = [
        Span(offset(*t.start), offset(*t.end), t.string)
        for t in tokens
        if t.type == tokenize.COMMENT
    ]
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef | ast.Module):
            body = node.body
            if (
                body
                and isinstance(body[0], ast.Expr)
                and isinstance(body[0].value, ast.Constant)
                and isinstance(body[0].value.value, str)
            ):
                doc = body[0]
                assert doc.end_lineno is not None and doc.end_col_offset is not None
                a = offset(doc.lineno, doc.col_offset)
                b = offset(doc.end_lineno, doc.end_col_offset)
                spans.append(Span(a, b, code[a:b]))
    return sorted(spans, key=lambda s: s.start)


def comment_spans(code: str, lang: str) -> list[Span] | None:
    if lang == "python":
        return python_comment_spans(code)
    return scan_clike(code, lang)[1]


def remove_spans(code: str, spans: Iterable[Span]) -> str:
    """Cut the spans; lines left blank by a cut are dropped, other blank lines kept."""
    cut = sorted(spans, key=lambda s: s.start)
    if not cut:
        return code
    pieces, pos = [], 0
    touched: set[int] = set()
    for s in cut:
        if s.start < pos:
            continue
        pieces.append(code[pos : s.start])
        pos = s.end
        touched.add(len("".join(pieces)))
    pieces.append(code[pos:])
    merged = "".join(pieces)
    # Recompute which output lines held a cut.
    lines_out, cursor = [], 0
    marks = sorted(touched)
    for line in merged.split("\n"):
        end = cursor + len(line)
        had_cut = any(cursor <= m <= end for m in marks)
        if not (had_cut and not line.strip()):
            lines_out.append(line.rstrip() if had_cut else line)
        cursor = end + 1
    return "\n".join(lines_out)


def strip_pair(before: str, after: str, lang: str) -> tuple[str, str] | None:
    """Remove comments that differ between versions or mention the fix."""
    sb, sa = comment_spans(before, lang), comment_spans(after, lang)
    if sb is None or sa is None:
        return None
    nb = {_norm_comment(s.text) for s in sb}
    na = {_norm_comment(s.text) for s in sa}

    def drop(spans: list[Span], other: set[str]) -> list[Span]:
        return [s for s in spans if _norm_comment(s.text) not in other or SENSITIVE.search(s.text)]

    return remove_spans(before, drop(sb, na)), remove_spans(after, drop(sa, nb))


# ---------------------------------------------------------------------------
# Function extraction
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Func:
    name: str
    start: int  # 0-based first line
    end: int  # 0-based last line, inclusive
    text: str


def python_functions(code: str) -> list[Func] | None:
    try:
        tree = ast.parse(code)
    except (SyntaxError, ValueError):
        return None
    lines = code.splitlines()
    out: list[Func] = []

    def visit(node: ast.AST, prefix: str) -> None:
        for child in ast.iter_child_nodes(node):
            if isinstance(child, ast.FunctionDef | ast.AsyncFunctionDef):
                start = min([child.lineno, *(d.lineno for d in child.decorator_list)]) - 1
                end = (child.end_lineno or child.lineno) - 1
                name = f"{prefix}{child.name}"
                out.append(Func(name, start, end, "\n".join(lines[start : end + 1])))
                visit(child, f"{name}.")
            elif isinstance(child, ast.ClassDef):
                visit(child, f"{prefix}{child.name}.")
            elif not isinstance(child, ast.expr):
                visit(child, prefix)

    visit(tree, "")
    return out


_JS_KEYWORDS = frozenset(
    {
        "if",
        "for",
        "while",
        "switch",
        "catch",
        "return",
        "function",
        "else",
        "do",
        "try",
        "new",
        "typeof",
        "await",
        "yield",
        "with",
        "super",
        "import",
        "export",
        "throw",
        "delete",
        "void",
        "case",
        "constructor_",
    }
)
_HEADERS: dict[str, list[re.Pattern[str]]] = {
    "go": [
        re.compile(r"^[ \t]*func\s*(?:\(\s*\w*\s*\*?\s*([A-Za-z_]\w*)[^)]*\)\s*)?([A-Za-z_]\w*)")
    ],
    "rust": [
        re.compile(
            r"^[ \t]*(?:pub(?:\s*\([^)]*\))?\s+)?(?:default\s+)?(?:const\s+)?(?:async\s+)?"
            r"(?:unsafe\s+)?(?:extern\s+\"[^\"]*\"\s+)?fn\s+([A-Za-z_]\w*)"
        )
    ],
    "js": [
        re.compile(
            r"^[ \t]*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s*\*?\s*([A-Za-z_$][\w$]*)"
        ),
        re.compile(
            r"^[ \t]*(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=]+)?=\s*"
            r"(?:async\s+)?(?:function\b|\([^)]*\)\s*(?::[^=]+)?=>|[A-Za-z_$][\w$]*\s*=>)"
        ),
        re.compile(r"^[ \t]*([A-Za-z_$][\w$]*)\s*:\s*(?:async\s+)?(?:function\b|\([^)]*\)\s*=>)"),
        re.compile(
            r"^[ \t]*(?:(?:public|private|protected|static|async|override|readonly|get|set"
            r"|abstract)\s+)*\*?\s*(#?[A-Za-z_$][\w$]*)\s*(?:<[^>()]*>)?\s*\([^;]*$"
        ),
    ],
}


def clike_functions(code: str, lang: str) -> list[Func]:  # noqa: PLR0912
    masked, _ = scan_clike(code, lang)
    lines = masked.split("\n")
    raw_lines = code.split("\n")
    starts = [0]
    for line in lines:
        starts.append(starts[-1] + len(line) + 1)
    patterns = _HEADERS[lang_family(lang)]
    out: list[Func] = []
    for index, line in enumerate(lines):
        match = None
        for pattern in patterns:
            match = pattern.match(line)
            if match:
                break
        if not match:
            continue
        name = match.group(match.lastindex or 1) or ""
        if lang == "go" and match.lastindex == 2 and match.group(1):
            name = f"{match.group(1)}.{match.group(2)}"
        if not name or name in _JS_KEYWORDS:
            continue
        pos = starts[index] + match.end()
        depth = 0
        brace = -1
        while pos < len(masked):
            ch = masked[pos]
            if ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
            elif ch == ";" and depth <= 0:
                break
            elif ch == "{" and depth <= 0:
                brace = pos
                break
            pos += 1
        if brace < 0:
            continue
        level, pos = 0, brace
        close = -1
        while pos < len(masked):
            if masked[pos] == "{":
                level += 1
            elif masked[pos] == "}":
                level -= 1
                if level == 0:
                    close = pos
                    break
            pos += 1
        if close < 0:
            continue
        end_line = masked.count("\n", 0, close)
        out.append(Func(name, index, end_line, "\n".join(raw_lines[index : end_line + 1])))
    return out


def functions_of(code: str, lang: str) -> list[Func] | None:
    if lang == "python":
        return python_functions(code)
    return clike_functions(code, lang)


def _keyed(funcs: list[Func]) -> dict[str, Func]:
    seen: Counter[str] = Counter()
    out = {}
    for f in funcs:
        out[f"{f.name}#{seen[f.name]}"] = f
        seen[f.name] += 1
    return out


def changed_function(before: str, after: str, lang: str) -> tuple[Func, Func] | None:
    """The function the change touched most, as (before, after), or None."""
    fb, fa = functions_of(before, lang), functions_of(after, lang)
    if not fb or not fa:
        return None
    bl, al = before.split("\n"), after.split("\n")
    touched: Counter[int] = Counter()
    matcher = difflib.SequenceMatcher(None, bl, al, autojunk=False)
    for tag, i1, i2, _j1, _j2 in matcher.get_opcodes():
        if tag == "equal":
            continue
        if i1 == i2:
            touched[max(i1 - 1, 0)] += 1
            touched[i1] += 1
        for k in range(i1, i2):
            touched[k] += 1
    after_by_key = _keyed(fa)
    best: tuple[int, int, str] | None = None
    for key, f in _keyed(fb).items():
        g = after_by_key.get(key)
        if g is None or g.text == f.text:
            continue
        hits = sum(v for line, v in touched.items() if f.start <= line <= f.end)
        if hits == 0:
            continue
        rank = (hits, -(f.end - f.start), key)
        if best is None or rank[:2] > best[:2]:
            best = rank
    if best is None:
        return None
    key = best[2]
    f, g = _keyed(fb)[key], after_by_key[key]
    lines_f, lines_g = f.text.split("\n"), g.text.split("\n")
    if difflib.SequenceMatcher(None, lines_f, lines_g, autojunk=False).ratio() < 0.3:
        return None
    return f, g


# ---------------------------------------------------------------------------
# Advisories: CWE families
# ---------------------------------------------------------------------------

WEAKNESS_CLASSES: dict[str, str] = {
    "injection": "Injection: untrusted input reaches a command, query, template, header, XML "
    "parser or interpreter (SQL, OS command, code, CRLF, XXE, template)",
    "path-traversal": "Path traversal or link following: a file path escapes the intended "
    "directory",
    "xss": "Cross-site scripting: untrusted input is rendered as HTML or script",
    "memory-safety": "Memory safety: out-of-bounds access, use after free, integer overflow, "
    "null dereference, type confusion",
    "resource-exhaustion": "Resource exhaustion or denial of service: unbounded CPU, memory, "
    "recursion, regex backtracking or a crash",
    "access-control": "Authentication, authorization, session or CSRF flaw",
    "crypto": "Cryptography or randomness: weak algorithms, missing signature or certificate "
    "checks, predictable values, timing leaks",
    "deserialization": "Unsafe deserialization or object pollution: untrusted data builds or "
    "mutates objects (prototype pollution, mass assignment)",
    "ssrf": "Server-side request forgery or open redirect: untrusted URLs are fetched or "
    "redirected to",
    "info-exposure": "Information exposure: secrets or sensitive data leak through output, "
    "logs, errors or storage",
}


def _ids(*spec: int | tuple[int, int]) -> set[int]:
    out: set[int] = set()
    for item in spec:
        if isinstance(item, tuple):
            out.update(range(item[0], item[1] + 1))
        else:
            out.add(item)
    return out


CWE_FAMILIES: dict[str, set[int]] = {
    "injection": _ids(
        74,
        150,
        75,
        76,
        77,
        78,
        88,
        89,
        90,
        91,
        93,
        94,
        95,
        96,
        97,
        113,
        117,
        470,
        564,
        611,
        643,
        652,
        827,
        917,
        943,
        1236,
        1336,
    ),
    "path-traversal": _ids((22, 36), 41, 59, 61, 62, 64, 65, 66, 67, 73),
    "xss": _ids(79, 80, 81, 82, 83, 84, 85, 86, 87),
    "memory-safety": _ids(
        (119, 127),
        129,
        130,
        131,
        170,
        190,
        191,
        193,
        415,
        416,
        457,
        466,
        467,
        476,
        587,
        680,
        681,
        786,
        787,
        788,
        805,
        806,
        (822, 825),
        843,
        908,
        909,
    ),
    "resource-exhaustion": _ids(
        404,
        248,
        369,
        400,
        405,
        407,
        409,
        606,
        617,
        674,
        770,
        776,
        789,
        834,
        835,
        920,
        1050,
        1325,
        1333,
    ),
    "access-control": _ids(
        250,
        (266, 290),
        294,
        306,
        307,
        346,
        352,
        384,
        425,
        613,
        639,
        732,
        798,
        862,
        863,
        942,
        1220,
        1385,
    ),
    "crypto": _ids(
        208,
        311,
        (295, 299),
        310,
        (322, 331),
        (334, 338),
        340,
        345,
        347,
        354,
        385,
        757,
        759,
        760,
        916,
        1240,
        1241,
    ),
    "deserialization": _ids(502, 913, 915, 1321),
    "ssrf": _ids(441, 601, 918),
    "info-exposure": _ids(
        (200, 203),
        488,
        524,
        668,
        (209, 215),
        256,
        260,
        (312, 316),
        319,
        359,
        497,
        522,
        526,
        532,
        538,
        540,
        548,
        552,
        1230,
        1258,
    ),
}
_CWE_TO_FAMILY = {cwe: fam for fam, ids in CWE_FAMILIES.items() for cwe in ids}


def collapse_cwes(cwes: Iterable[str | int]) -> str | None:
    """The one weakness family the CWEs map to; None if none map or they disagree."""
    families = set()
    for cwe in cwes:
        m = re.search(r"(\d+)", str(cwe))
        if m and int(m.group(1)) in _CWE_TO_FAMILY:
            families.add(_CWE_TO_FAMILY[int(m.group(1))])
    return families.pop() if len(families) == 1 else None


WEAKNESS_KEYWORDS: dict[str, str] = {
    "injection": r"sql|query|exec|shell|command|eval|template|subprocess|spawn|header|crlf|\\r"
    r"|ldap|xpath|xml",
    "path-traversal": r"path|\.\./|dir|filepath|filename|symlink|realpath|abspath|join",
    "xss": r"html|innerhtml|script|dompurify|href|markup|dangerously|render",
    "memory-safety": r"unsafe|ptr|alloc|buffer|offset|bounds|usize|transmute|slice|index",
    "resource-exhaustion": r"limit|max|timeout|depth|recurs|regex|loop|count|size",
    "access-control": r"auth|permission|role|session|login|admin|allow|deny|owner|csrf|origin",
    "crypto": r"crypto|random|rand|hash|cipher|sign|verify|hmac|tls|cert|nonce",
    "deserialization": r"pickle|yaml|unmarshal|deserializ|__proto__|prototype|constructor"
    r"|merge|assign",
    "ssrf": r"url|http|fetch|request|host|redirect|dns|location",
    "info-exposure": r"log|error|secret|password|leak|expose|debug|mask|redact|trace",
}
_WEAKNESS_RULES = {k: re.compile(v, re.I) for k, v in WEAKNESS_KEYWORDS.items()}


def keyword_weakness(text: str) -> str:
    scores = {k: float(len(rule.findall(text))) for k, rule in _WEAKNESS_RULES.items()}
    return argmax_label(scores, list(WEAKNESS_CLASSES))


# ---------------------------------------------------------------------------
# Advisories: function pairs
# ---------------------------------------------------------------------------

FUNCTION_BUDGET = 6_000
MIN_FUNCTION_LINES = 3
VALIDATION_WORDS = re.compile(r"check|valid|sanitiz|escape|limit|max|len", re.I)


@dataclass(frozen=True)
class FixPair:
    key: str
    unit: str
    lang: str
    path: str
    function: str
    before: str
    after: str
    record: dict[str, Any]


def advisory_pairs(root: Path, *, fix_after_cutoff: bool = False) -> list[FixPair]:
    """One stripped before/after function pair per usable advisory, deduplicated by fix.

    Advisories must be published on or after the cutoff; with ``fix_after_cutoff``
    the fix commit itself must also date from after it.
    """
    records = [r for r in read_jsonl(root / ADVISORY_CACHE) if r.get("status") == "ok"]
    records.sort(key=lambda r: str(r.get("id")))
    pairs: dict[str, FixPair] = {}
    for record in records:
        if not _on_or_after_cutoff(record.get("published_at") or record.get("merged_at")):
            continue
        if fix_after_cutoff and not _on_or_after_cutoff(
            record.get("commit_date") or record.get("merged_at")
        ):
            continue
        for entry in record.get("files", []):
            before, after = entry.get("before"), entry.get("after")
            path = str(entry.get("path", ""))
            lang = source_lang(path)
            if not before or not after or lang is None:
                continue
            found = changed_function(before, after, lang)
            if found is None:
                continue
            f, g = found
            b, a = textwrap.dedent(f.text), textwrap.dedent(g.text)
            stripped = strip_pair(b, a, lang)
            if stripped is None:
                continue
            b, a = stripped
            if _squash(b) == _squash(a):
                continue
            if max(len(b), len(a)) > FUNCTION_BUDGET:
                continue
            if min(b.count("\n"), a.count("\n")) + 1 < MIN_FUNCTION_LINES:
                continue
            key = f"{record['repo']}@{record.get('fix_commit')}:{path}:{f.name}"
            if key in pairs:
                prior = pairs[key].record
                merged = {
                    **prior,
                    "cwes": sorted({*prior.get("cwes", []), *record.get("cwes", [])}),
                }
                merged["ids"] = sorted({*prior.get("ids", [prior.get("id")]), record.get("id")})
                pairs[key] = FixPair(
                    key,
                    pairs[key].unit,
                    lang,
                    path,
                    f.name,
                    pairs[key].before,
                    pairs[key].after,
                    merged,
                )
                continue
            rec = {**record, "ids": [record.get("id")]}
            rec.pop("files", None)
            pairs[key] = FixPair(key, str(record["repo"]), lang, path, f.name, b, a, rec)
            break  # one function per advisory: the most-changed file comes first
    return [pairs[k] for k in sorted(pairs)]


def _squash(code: str) -> str:
    return re.sub(r"\s+", "", code)


def _fence(lang: str) -> str:
    return {"typescript": "ts", "javascript": "js"}.get(lang, lang)


def build_vuln_pair(ctx: BuildContext) -> list[Item]:
    pairs = advisory_pairs(
        mined_dir(ctx), fix_after_cutoff=bool((ctx.options or {}).get("fix_after_cutoff", True))
    )
    candidates = []
    for pair in pairs:
        vulnerable, fixed = pair.before, pair.after
        candidates.append(
            Candidate(
                key=pair.key,
                unit=pair.unit,
                answer="vulnerable",
                shortcuts={
                    "longer": _pick(len(vulnerable) > len(fixed), len(vulnerable) == len(fixed)),
                    "shorter": _pick(len(vulnerable) < len(fixed), len(vulnerable) == len(fixed)),
                    "more_checks": _pick(
                        _checks(vulnerable) > _checks(fixed), _checks(vulnerable) == _checks(fixed)
                    ),
                    "fewer_checks": _pick(
                        _checks(vulnerable) < _checks(fixed), _checks(vulnerable) == _checks(fixed)
                    ),
                },
                payload={"pair": pair},
            )
        )
    for c in candidates:
        c.payload["ties"] = tuple(sorted(k for k, v in c.shortcuts.items() if v == TIE))
    rng = random.Random(f"{ctx.seed}:vuln-pair")
    # Tied shortcuts are position-decided later, so they count as half right.
    balanced = cap_shortcuts(candidates, 0.55, rng)
    chosen = spread_units(balanced, ctx.per_family, rng)
    chosen = cap_shortcuts(chosen, 0.56, rng)
    return [_vuln_item(c, first) for c, first in assign_positions(chosen, rng)]


def assign_positions(chosen: list[Candidate], rng: random.Random) -> list[tuple[Candidate, bool]]:
    """Vulnerable-first flags, balanced overall and within each tie pattern.

    A tied shortcut answers position A, so alternating positions inside each
    group of items that tie on the same shortcuts keeps those shortcuts at
    chance as well as the answer positions.
    """
    groups: dict[tuple[str, ...], list[Candidate]] = defaultdict(list)
    for c in chosen:
        groups[tuple(c.payload.get("ties", ()))].append(c)
    toggle = rng.random() < 0.5
    out = []
    for key in sorted(groups):
        group = sorted(groups[key], key=lambda c: c.key)
        rng.shuffle(group)
        for c in group:
            out.append((c, toggle))
            toggle = not toggle
    out.sort(key=lambda pair: pair[0].key)
    return out


def _checks(code: str) -> int:
    return len(VALIDATION_WORDS.findall(code))


def _pick(vulnerable_wins: bool, tie: bool) -> str:
    return TIE if tie else ("vulnerable" if vulnerable_wins else "fixed")


def _vuln_item(c: Candidate, vulnerable_first: bool) -> Item:
    pair: FixPair = c.payload["pair"]
    a, b = (pair.before, pair.after) if vulnerable_first else (pair.after, pair.before)
    answer = "A" if vulnerable_first else "B"
    record = pair.record
    fence = _fence(pair.lang)
    state = (
        f"Language: {LANG_NAMES[pair.lang]}\nRepository: {record['repo']}\nFile: {pair.path}\n\n"
        "Below are two versions of the same function from this file. One is the code as it "
        "was before a security fix, the other is the same function after the fix.\n\n"
        f"Version A:\n```{fence}\n{a}\n```\n\nVersion B:\n```{fence}\n{b}\n```\n"
    )

    shortcuts = {}
    la, lb = len(a), len(b)
    ca, cb = _checks(a), _checks(b)
    shortcuts["longer"] = "A" if la >= lb else "B"
    shortcuts["shorter"] = "A" if la <= lb else "B"
    shortcuts["more_checks"] = "A" if ca >= cb else "B"
    shortcuts["fewer_checks"] = "A" if ca <= cb else "B"
    return make_item(
        family="vuln-pair",
        key=c.key,
        source_unit=c.unit,
        state=state,
        question=choice_question(
            "Which version is the vulnerable one, from before the security fix?",
            {"A": "Version A is vulnerable", "B": "Version B is vulnerable"},
        ),
        answer=answer,
        reference="merged-fix",
        shortcuts=shortcuts,
        source=_advisory_source(record, pair),
    )


def _advisory_source(record: dict[str, Any], pair: FixPair) -> dict[str, Any]:
    return {
        "repo": record["repo"],
        "advisories": record.get("ids", [record.get("id")]),
        "fix_commit": record.get("fix_commit"),
        "published_at": record.get("published_at"),
        "commit_date": record.get("commit_date"),
        "path": pair.path,
        "function": pair.function,
    }


def build_weakness_class(ctx: BuildContext) -> list[Item]:
    pairs = advisory_pairs(
        mined_dir(ctx), fix_after_cutoff=bool((ctx.options or {}).get("fix_after_cutoff", True))
    )
    include_diff = bool((ctx.options or {}).get("weakness_diff", True))
    candidates = []
    for pair in pairs:
        family = collapse_cwes(pair.record.get("cwes", []))
        if family is None:
            continue
        before = remove_spans(
            pair.before,
            [s for s in (comment_spans(pair.before, pair.lang) or []) if SENSITIVE.search(s.text)],
        )
        diff = "\n".join(
            difflib.unified_diff(
                pair.before.split("\n"), pair.after.split("\n"), "before", "after", lineterm="", n=2
            )
        )
        state_code = before + ("\n" + diff if include_diff else "")
        candidates.append(
            Candidate(
                key=pair.key,
                unit=pair.unit,
                answer=family,
                shortcuts={"keywords": keyword_weakness(state_code)},
                payload={"pair": pair, "before": before, "diff": diff if include_diff else ""},
            )
        )
    rng = random.Random(f"{ctx.seed}:weakness-class")
    opts = ctx.options or {}
    class_cap = float(opts.get("weakness_cap", 0.2))
    keyword_cap = float(opts.get("weakness_keyword_cap", 0.26))
    # Cap the largest classes first, then the keyword rules, then spread over repos.
    per_class = max(1, math.ceil(class_cap * min(ctx.per_family, len(candidates))))
    by_class: dict[str, list[Candidate]] = defaultdict(list)
    for c in candidates:
        by_class[c.answer].append(c)
    capped: list[Candidate] = []
    for cls in sorted(by_class):
        capped.extend(spread_units(by_class[cls], per_class, rng))
    capped = cap_shortcuts(capped, keyword_cap, rng)
    chosen = spread_units(capped, ctx.per_family, rng)
    return [_weakness_item(c, ctx.seed) for c in chosen]


def _weakness_item(c: Candidate, seed: int) -> Item:
    pair: FixPair = c.payload["pair"]
    record = pair.record
    fence = _fence(pair.lang)
    state = (
        f"Language: {LANG_NAMES[pair.lang]}\nRepository: {record['repo']}\nFile: {pair.path}\n\n"
        f"A vulnerable function, as it was before its security fix:\n```{fence}\n"
        f"{c.payload['before']}\n```\n"
    )
    if c.payload["diff"]:
        state += f"\nThe fix, as a diff of this function:\n```diff\n{c.payload['diff']}\n```\n"
    order = list(WEAKNESS_CLASSES)
    random.Random(f"{seed}:weakness-class:order:{c.key}").shuffle(order)
    question = choice_question(
        "Which weakness class best describes the vulnerability that this fix addresses?",
        {k: WEAKNESS_CLASSES[k] for k in order},
    )
    return make_item(
        family="weakness-class",
        key=c.key,
        source_unit=c.unit,
        state=state,
        question=question,
        answer=c.answer,
        reference="advisory",
        shortcuts=c.shortcuts,
        source={**_advisory_source(record, pair), "cwes": record.get("cwes", [])},
    )


# ---------------------------------------------------------------------------
# semver-impact: a deterministic public-API differ for Python
# ---------------------------------------------------------------------------

SEMVER_LEVELS = ("patch", "minor", "major")
_RANK = {level: i for i, level in enumerate(SEMVER_LEVELS)}
SEMVER_DIFF_BUDGET = 30_000
META = "~"
REEXPORT = "~import:"
BASES = "~bases:"
LAZY = "~lazy:"
# Assignments that are public by name but not API: type variables and loggers.
NOT_API_NAMES = ("TYPE_CHECKING",)
NOT_API_CALLS = frozenset({"TypeVar", "ParamSpec", "TypeVarTuple", "NewType", "getLogger"})


@dataclass(frozen=True)
class Param:
    name: str
    kind: str  # posonly, pos, vararg, kwonly, varkw
    has_default: bool


def _params(args: ast.arguments, drop_first: bool) -> tuple[Param, ...]:
    positional = [*args.posonlyargs, *args.args]
    defaults_from = len(positional) - len(args.defaults)
    out: list[Param] = []
    for i, a in enumerate(positional):
        kind = "posonly" if i < len(args.posonlyargs) else "pos"
        out.append(Param(a.arg, kind, i >= defaults_from))
    if args.vararg:
        out.append(Param(args.vararg.arg, "vararg", True))
    for a, d in zip(args.kwonlyargs, args.kw_defaults, strict=True):
        out.append(Param(a.arg, "kwonly", d is not None))
    if args.kwarg:
        out.append(Param(args.kwarg.arg, "varkw", True))
    if drop_first and out and out[0].kind in {"posonly", "pos"}:
        out = out[1:]
    return tuple(out)


def _decorator_names(node: ast.FunctionDef | ast.AsyncFunctionDef) -> list[str]:
    names = []
    for d in node.decorator_list:
        target = d.func if isinstance(d, ast.Call) else d
        if isinstance(target, ast.Name):
            names.append(target.id)
        elif isinstance(target, ast.Attribute):
            names.append(target.attr)
    return names


def _is_public(name: str) -> bool:
    return not name.startswith("_") or name in {"__init__", "__call__"}


def _function_api(
    node: ast.FunctionDef | ast.AsyncFunctionDef, in_class: bool
) -> tuple[Any, ...] | None:
    decorators = _decorator_names(node)
    if "overload" in decorators or "setter" in decorators or "deleter" in decorators:
        return None
    if "property" in decorators or "cached_property" in decorators:
        return ("property",)
    kind = "static" if "staticmethod" in decorators else "method" if in_class else "function"
    if "classmethod" in decorators:
        kind = "classmethod"
    drop_first = in_class and kind in {"method", "classmethod"}
    return (
        "function",
        kind,
        isinstance(node, ast.AsyncFunctionDef),
        _params(node.args, drop_first),
    )


def _flat_body(body: list[ast.stmt]) -> Iterable[ast.stmt]:
    """Statements, looking inside module- or class-level if/try blocks."""
    for stmt in body:
        if isinstance(stmt, ast.If):
            yield from _flat_body(stmt.body)
            yield from _flat_body(stmt.orelse)
        elif isinstance(stmt, ast.Try):
            yield from _flat_body(stmt.body)
            for handler in stmt.handlers:
                yield from _flat_body(handler.body)
            yield from _flat_body(stmt.orelse)
            yield from _flat_body(stmt.finalbody)
        else:
            yield stmt


def _assigned_names(stmt: ast.stmt) -> list[str]:
    if isinstance(stmt, ast.Assign):
        return [t.id for t in stmt.targets if isinstance(t, ast.Name)]
    if isinstance(stmt, ast.AnnAssign) and isinstance(stmt.target, ast.Name):
        return [stmt.target.id]
    return []


def module_api(  # noqa: PLR0912
    source: str, *, is_init: bool = False
) -> dict[str, tuple[Any, ...]] | None:
    """Public names of one module mapped to a comparable signature; None if unparseable."""
    try:
        tree = ast.parse(source)
    except (SyntaxError, ValueError):
        return None
    api: dict[str, tuple[Any, ...]] = {}
    for stmt in _flat_body(tree.body):
        if isinstance(stmt, ast.FunctionDef | ast.AsyncFunctionDef):
            if stmt.name == "__getattr__":
                # A module __getattr__ keeps deprecated names importable.
                for node in ast.walk(stmt):
                    if isinstance(node, ast.Constant) and isinstance(node.value, str):
                        api[f"{LAZY}{node.value}"] = ("lazy",)
            if stmt.name.startswith("_"):
                continue
            sig = _function_api(stmt, in_class=False)
            if sig is not None:
                api[stmt.name] = sig
        elif isinstance(stmt, ast.ClassDef):
            if stmt.name.startswith("_"):
                continue
            api[stmt.name] = ("class",)
            api[f"{BASES}{stmt.name}"] = ("bases", tuple(_base_names(stmt)))
            for inner in _flat_body(stmt.body):
                if isinstance(inner, ast.FunctionDef | ast.AsyncFunctionDef):
                    if not _is_public(inner.name):
                        continue
                    sig = _function_api(inner, in_class=True)
                    if sig is not None and f"{stmt.name}.{inner.name}" not in api:
                        api[f"{stmt.name}.{inner.name}"] = sig
                for name in _assigned_names(inner):
                    if not name.startswith("_"):
                        api[f"{stmt.name}.{name}"] = ("value",)
        elif isinstance(stmt, ast.ImportFrom | ast.Import):
            # Imports are API only in a package __init__.py. Elsewhere they are
            # recorded under "~import:" so a name moved to another module and
            # imported back is not counted as removed.
            for alias in stmt.names:
                name = alias.asname or alias.name.split(".")[0]
                if name == "*" or name.startswith("_"):
                    continue
                if is_init and isinstance(stmt, ast.ImportFrom):
                    api[name] = ("import",)
                else:
                    api[f"{REEXPORT}{name}"] = ("import",)
        elif not _not_api_value(stmt):
            for name in _assigned_names(stmt):
                if not name.startswith("_"):
                    api[name] = ("value",)
    if any(isinstance(stmt, ast.FunctionDef) and stmt.name == "__getattr__" for stmt in tree.body):
        # Lazy modules often list their names in a module-level table.
        for stmt in tree.body:
            if isinstance(stmt, ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef):
                continue
            for node in ast.walk(stmt):
                if isinstance(node, ast.Constant) and isinstance(node.value, str):
                    api[f"{LAZY}{node.value}"] = ("lazy",)
    for name in NOT_API_NAMES:
        api.pop(name, None)
    return api


def _base_names(node: ast.ClassDef) -> list[str]:
    names = []
    for base in node.bases:
        target = base.value if isinstance(base, ast.Subscript) else base
        if isinstance(target, ast.Name):
            names.append(target.id)
        elif isinstance(target, ast.Attribute):
            names.append(target.attr)
    return names


def _not_api_value(stmt: ast.stmt) -> bool:
    value = stmt.value if isinstance(stmt, ast.Assign | ast.AnnAssign) else None
    if not isinstance(value, ast.Call):
        return False
    func = value.func
    name = (
        func.id
        if isinstance(func, ast.Name)
        else func.attr
        if isinstance(func, ast.Attribute)
        else ""
    )
    return name in NOT_API_CALLS


def _inherited(name: str, api: dict[str, tuple[Any, ...]]) -> bool:
    """True when ``Class.member`` is provided by a base class defined in the same module."""
    if "." not in name:
        return False
    cls, member = name.split(".", 1)
    stack = list(api.get(f"{BASES}{cls}", ("bases", ()))[1])
    seen: set[str] = set()
    while stack:
        base = stack.pop()
        if base in seen:
            continue
        seen.add(base)
        if f"{base}.{member}" in api:
            return True
        stack.extend(api.get(f"{BASES}{base}", ("bases", ()))[1])
    return False


def _still_reachable(name: str, api: dict[str, tuple[Any, ...]]) -> bool:
    """A name missing from the definitions but still importable or inherited.

    An ``__init__`` that appears or disappears is treated as reachable: the
    constructor it replaces is inherited from somewhere this module cannot see.
    """
    owner = name.split(".", maxsplit=1)[0]
    if name.endswith(".__init__"):
        return True
    return (
        f"{REEXPORT}{owner}" in api
        or f"{LAZY}{owner}" in api
        or f"{LAZY}{name}" in api
        or _inherited(name, api)
    )


def signature_change(  # noqa: PLR0911, PLR0912
    old: tuple[Param, ...], new: tuple[Param, ...]
) -> str:
    """patch, minor (compatible extension) or major (a call that worked may now fail)."""
    level = "patch"
    old_pos = [p for p in old if p.kind in {"posonly", "pos"}]
    new_pos = [p for p in new if p.kind in {"posonly", "pos"}]
    for i, p in enumerate(old_pos):
        if i >= len(new_pos):
            return "major"
        q = new_pos[i]
        if p.kind == "pos" and (q.name != p.name or q.kind == "posonly"):
            return "major"
        if p.has_default and not q.has_default:
            return "major"
        if (not p.has_default and q.has_default) or (p.kind == "posonly" and q.kind == "pos"):
            level = "minor"
    for q in new_pos[len(old_pos) :]:
        if not q.has_default:
            return "major"
        level = "minor"
    old_kw = {p.name: p for p in old if p.kind == "kwonly"}
    new_kw = {p.name: p for p in new if p.kind == "kwonly"}
    new_pos_names = {q.name: q for q in new_pos if q.kind == "pos"}
    for name, p in old_kw.items():
        moved = new_kw.get(name) or new_pos_names.get(name)
        if moved is None or (p.has_default and not moved.has_default):
            return "major"
        if name not in new_kw:
            level = "minor"
    for name, q in new_kw.items():
        if name not in old_kw:
            if not q.has_default:
                return "major"
            level = "minor"
    for star in ("vararg", "varkw"):
        had = any(p.kind == star for p in old)
        has = any(p.kind == star for p in new)
        if had and not has:
            return "major"
        if has and not had:
            level = "minor"
    return level


def api_change(
    before: dict[str, tuple[Any, ...]] | None, after: dict[str, tuple[Any, ...]] | None
) -> tuple[str, list[str]]:
    """Level of one module's API change plus the reasons, for audit."""
    full_old, full_new = before or {}, after or {}
    level = "patch"
    reasons: list[str] = []
    old = {k: v for k, v in full_old.items() if not k.startswith(META)}
    new = {k: v for k, v in full_new.items() if not k.startswith(META)}

    def bump(to: str, why: str) -> None:
        nonlocal level
        reasons.append(why)
        if _RANK[to] > _RANK[level]:
            level = to

    for name in sorted(old):
        if name not in new:
            if not _still_reachable(name, full_new):
                bump("major", f"removed {name}")
            continue
        a, b = old[name], new[name]
        if a == b or "import" in {a[0], b[0]} or {a[0], b[0]} == {"value", "property"}:
            continue
        if a[0] != b[0] or (a[0] == "function" and (a[1] != b[1] or a[2] != b[2])):
            bump("major", f"changed kind of {name}")
        elif a[0] == "function":
            change = signature_change(a[3], b[3])
            if change != "patch":
                bump(change, f"{change} signature change of {name}")
    for name in sorted(set(new) - set(old)):
        if not _still_reachable(name, full_old):
            bump("minor", f"added {name}")
    return level, reasons


def label_semver(record: dict[str, Any]) -> tuple[str, list[str]] | None:
    """Label a mined PR from full file contents at both commits; None if unlabelable."""
    level = "patch"
    reasons: list[str] = []
    contents = record.get("contents", {})
    for entry in record.get("files", []):
        path = str(entry.get("path"))
        previous = str(entry.get("previous_path") or path)
        paths = {previous, path}
        for p in sorted(paths):
            if not is_public_module(p):
                continue
            got = contents.get(p)
            if got is None:
                return None
            is_init = p.endswith("__init__.py")
            before_src, after_src = got.get("before"), got.get("after")
            if len(paths) == 2:  # a rename: old path loses everything, new path gains
                before_src = before_src if p == previous else None
                after_src = after_src if p == path else None
            before = module_api(before_src, is_init=is_init) if before_src is not None else {}
            after = module_api(after_src, is_init=is_init) if after_src is not None else {}
            if before is None or after is None:
                return None
            lvl, why = api_change(before, after)
            reasons.extend(f"{p}: {w}" for w in why)
            if _RANK[lvl] > _RANK[level]:
                level = lvl
    return level, reasons


def semver_diff_text(record: dict[str, Any]) -> str | None:
    blocks = []
    for entry in sorted(record.get("files", []), key=lambda e: str(e.get("path"))):
        patch = entry.get("patch")
        if patch is None:
            return None
        path, status = str(entry["path"]), str(entry.get("status", "modified"))
        header = f"--- a/{entry.get('previous_path') or path}\n+++ b/{path}"
        if status == "added":
            header = f"--- /dev/null\n+++ b/{path}  (new file)"
        elif status == "removed":
            header = f"--- a/{path}\n+++ /dev/null  (deleted file)"
        elif status == "renamed":
            header += "  (renamed)"
        blocks.append(f"{header}\n{patch}")
    return "\n".join(blocks) if blocks else None


REMOVE_WORDS = re.compile(r"remov|deprecat|delet|\bdrop", re.I)
NEW_DEF = re.compile(r"^\+\s*(async\s+def|def|class)\s+[A-Za-z]", re.M)


def semver_shortcuts(diff: str, record: dict[str, Any], bands: tuple[int, int]) -> dict[str, str]:
    changed = sum(
        1 for line in diff.split("\n") if line[:1] in "+-" and not line.startswith(("+++", "---"))
    )
    size = "patch" if changed <= bands[0] else "minor" if changed <= bands[1] else "major"
    keywords = (
        "major" if REMOVE_WORDS.search(diff) else "minor" if NEW_DEF.search(diff) else "patch"
    )
    public = any(is_public_module(str(e.get("path"))) for e in record.get("files", []))
    return {
        "size_band": size,
        "remove_words": keywords,
        "private_paths": keywords if public else "patch",
    }


def _changed_lines(diff: str) -> int:
    return sum(1 for ln in diff.split("\n") if ln[:1] in "+-" and not ln.startswith(("+++", "---")))


def build_semver_impact(ctx: BuildContext) -> list[Item]:
    root = mined_dir(ctx)
    records = [r for r in read_jsonl(root / SEMVER_CACHE) if r.get("status") == "ok"]
    records.sort(key=lambda r: (str(r.get("repo")), int(r.get("number", 0))))
    prepared = []
    for record in records:
        if not _on_or_after_cutoff(record.get("merged_at")):
            continue
        diff = semver_diff_text(record)
        if diff is None or len(diff) > SEMVER_DIFF_BUDGET:
            continue
        labelled = label_semver(record)
        if labelled is None:
            continue
        prepared.append((record, diff, labelled))
    if not prepared:
        return []
    sizes = sorted(_changed_lines(d) for _, d, _ in prepared)
    bands = (sizes[len(sizes) // 3], sizes[2 * len(sizes) // 3])
    candidates = [
        Candidate(
            key=f"{r['repo']}#{r['number']}",
            unit=str(r["repo"]),
            answer=lab[0],
            shortcuts=semver_shortcuts(d, r, bands),
            payload={"record": r, "diff": d, "reasons": lab[1]},
        )
        for r, d, lab in prepared
    ]
    rng = random.Random(f"{ctx.seed}:semver-impact")
    opts = ctx.options or {}
    cap = float(opts.get("semver_cap", 0.42))
    ratio = float(opts.get("semver_level_ratio", 2.0))
    # Major bumps are rare in merged PRs: no level may outnumber the rarest by more
    # than ``ratio``, then no shortcut or constant answer may beat ``cap``.
    by_level: dict[str, list[Candidate]] = defaultdict(list)
    for c in candidates:
        by_level[c.answer].append(c)
    rarest = min(len(by_level[level]) for level in SEMVER_LEVELS)
    per_level = max(1, math.ceil(ratio * rarest))
    levelled: list[Candidate] = []
    for level in SEMVER_LEVELS:
        levelled.extend(spread_units(by_level[level], per_level, rng))
    balanced = cap_shortcuts(levelled, cap, rng, constants=SEMVER_LEVELS)
    chosen = spread_units(balanced, ctx.per_family, rng)
    chosen = cap_shortcuts(chosen, cap + 0.02, rng, constants=SEMVER_LEVELS)
    return [_semver_item(c) for c in chosen]


SEMVER_INSTRUCTIONS = (
    "Under semantic versioning, what release bump does this change require for the "
    "library's public Python API? Public API means module-level functions, classes and "
    "constants, the public methods and attributes of public classes, and names imported "
    "into a package __init__.py, where the name does not start with an underscore and no "
    "module path component starts with an underscore or is named internal (tests, type "
    "variables and loggers excluded). A name that "
    "stays importable (re-imported from its new module, served by a module __getattr__, "
    "or inherited from a base class in the same module) is not removed, and a class "
    "gaining or losing an explicit __init__ is not by itself a change. major: a public "
    "name is removed or its call signature changes incompatibly (a parameter removed, "
    "renamed, reordered, made required or made keyword-only). minor: public names are "
    "added or signatures are extended compatibly (for example a new optional parameter). "
    "patch: no public API change."
)


def _semver_item(c: Candidate) -> Item:
    record = c.payload["record"]
    state = (
        f"Library: {record['repo']} (Python)\n\n"
        "Changes merged in one pull request, as a unified diff of the package's non-test "
        f"Python files:\n\n{c.payload['diff']}\n"
    )
    return make_item(
        family="semver-impact",
        key=c.key,
        source_unit=c.unit,
        state=state,
        question=score_question(SEMVER_INSTRUCTIONS, SEMVER_LEVELS),
        answer=c.answer,
        reference="tool",
        shortcuts=c.shortcuts,
        source={
            "repo": record["repo"],
            "pr": record["number"],
            "merged_at": record["merged_at"],
            "before_sha": record.get("before_sha"),
            "after_sha": record.get("after_sha"),
            "tool": "verdict-v2 python api differ",
            "reasons": c.payload["reasons"][:20],
        },
    )


BUILDERS: dict[str, Callable[[BuildContext], list[Item]]] = {
    "fix-file": build_fix_file,
    "vuln-pair": build_vuln_pair,
    "weakness-class": build_weakness_class,
    "semver-impact": build_semver_impact,
}
