"""``suite prs list``: merged pull requests of one repository, scored as task candidates.

One ``gh pr list --state merged`` call. Each pull request gets a suitability
score from 0 to 100 with the reasons behind it and flags for what makes it a
poor task: a task needs a behaviour change that tests can pin down, of a size
a model can make in one session, described well enough to write an issue
from. The score orders the list; the operator decides.
"""

from __future__ import annotations

import re
from pathlib import PurePosixPath
from typing import Any

from harness.suite_builder.gh import gh_json, is_test_path, require_login

PR_FIELDS = (
    "number,title,url,mergedAt,mergeCommit,additions,deletions,changedFiles,files,"
    "labels,closingIssuesReferences,body,author"
)
_REPO_RE = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
LANGUAGES = {
    ".py": "python",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".js": "javascript",
    ".jsx": "javascript",
    ".mjs": "javascript",
    ".go": "go",
    ".rs": "rust",
    ".java": "java",
    ".kt": "kotlin",
    ".c": "c",
    ".h": "c",
    ".cc": "cpp",
    ".cpp": "cpp",
    ".hpp": "cpp",
    ".rb": "ruby",
    ".php": "php",
    ".cs": "csharp",
    ".swift": "swift",
}
_DEP_FILES = {
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "poetry.lock",
    "uv.lock",
    "Cargo.lock",
    "go.sum",
    "Gemfile.lock",
    "composer.lock",
}
_DEP_TITLE = re.compile(r"\b(bump|deps?|dependabot|renovate|upgrade .* from)\b", re.I)


def languages_of(paths: list[str]) -> list[str]:
    found = {
        LANGUAGES[PurePosixPath(p).suffix] for p in paths if PurePosixPath(p).suffix in LANGUAGES
    }
    return sorted(found)


def score_pr(
    pr: dict[str, Any], *, min_loc: int = 10, max_loc: int = 400, max_files: int = 20
) -> dict[str, Any]:
    """Turn one ``gh pr list`` row into a candidate with a suitability score."""
    files = [f for f in pr.get("files") or [] if isinstance(f, dict) and f.get("path")]
    paths = [str(f["path"]) for f in files]
    test_files = [p for p in paths if is_test_path(p)]
    src_files = [p for p in paths if not is_test_path(p)]
    src_loc = sum(
        int(f.get("additions") or 0) + int(f.get("deletions") or 0)
        for f in files
        if not is_test_path(str(f["path"]))
    )
    labels = [str(lb.get("name")) for lb in pr.get("labels") or [] if isinstance(lb, dict)]
    linked = [
        int(i["number"])
        for i in pr.get("closingIssuesReferences") or []
        if isinstance(i, dict) and isinstance(i.get("number"), int)
    ]
    body = str(pr.get("body") or "").strip()
    title = str(pr.get("title") or "")
    n_files = int(pr.get("changedFiles") or len(paths))

    reasons: list[str] = []
    flags: list[str] = []
    score = 0
    if test_files:
        score += 30
        reasons.append(f"changes {len(test_files)} test file(s)")
    else:
        flags.append("no_tests")
    if src_loc < min_loc:
        flags.append("too_small")
    elif src_loc > max_loc:
        flags.append("too_large")
    else:
        score += 25
        reasons.append(f"{src_loc} source lines changed")
    if n_files > max_files:
        flags.append("many_files")
    else:
        score += 15
    dependency = (
        (
            any(PurePosixPath(p).name in _DEP_FILES for p in paths)
            and not any(PurePosixPath(p).suffix in LANGUAGES for p in src_files)
        )
        or bool(_DEP_TITLE.search(title))
        or any("depend" in lb.lower() for lb in labels)
    )
    if dependency:
        flags.append("dependency_bump")
    elif src_files:
        score += 15
        reasons.append("a code change, not a dependency bump")
    if linked or len(body) >= 80:
        score += 15
        reasons.append("linked issue" if linked else "described in the pull request")
    else:
        flags.append("no_description")
    merge = pr.get("mergeCommit") if isinstance(pr.get("mergeCommit"), dict) else {}
    return {
        "number": int(pr.get("number") or 0),
        "title": title,
        "url": pr.get("url"),
        "merged_at": pr.get("mergedAt"),
        "merge_commit": (merge or {}).get("oid"),
        "author": (pr.get("author") or {}).get("login")
        if isinstance(pr.get("author"), dict)
        else None,
        "additions": int(pr.get("additions") or 0),
        "deletions": int(pr.get("deletions") or 0),
        "changed_files": n_files,
        "src_loc": src_loc,
        "test_files": test_files,
        "src_files": src_files,
        "languages": languages_of(src_files or paths),
        "linked_issues": linked,
        "labels": labels,
        "suitability": {"score": min(score, 100), "reasons": reasons, "flags": flags},
    }


def list_prs(
    repo: str,
    *,
    since: str | None = None,
    limit: int = 50,
    min_loc: int = 10,
    max_loc: int = 400,
    max_files: int = 20,
) -> list[dict[str, Any]]:
    """Merged pull requests of ``repo``, best candidates first."""
    if not _REPO_RE.match(repo):
        raise ValueError(f"--repo must be owner/name, got {repo!r}")
    if since is not None and not _DATE_RE.match(since):
        raise ValueError(f"--since must be YYYY-MM-DD, got {since!r}")
    if not 1 <= limit <= 200:
        raise ValueError("--limit must be 1 to 200")
    require_login()
    args = [
        "pr",
        "list",
        "--repo",
        repo,
        "--state",
        "merged",
        "--limit",
        str(limit),
        "--json",
        PR_FIELDS,
    ]
    if since:
        args += ["--search", f"merged:>={since}"]
    rows = gh_json(args)
    if not isinstance(rows, list):
        return []
    scored = [
        score_pr(r, min_loc=min_loc, max_loc=max_loc, max_files=max_files)
        for r in rows
        if isinstance(r, dict)
    ]
    scored.sort(key=lambda c: (-c["suitability"]["score"], -c["number"]))
    return scored
