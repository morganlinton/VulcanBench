"""Tests for the CLI-harness integrity audit (web + filesystem)."""

from __future__ import annotations

import json
from pathlib import Path

from harness.agent.run_audit import audit_filesystem, audit_run, audit_stream, upstream_refs

META = {
    "upstream": {
        "url": "https://github.com/PennyLaneAI/pennylane/pull/9459",
        "commit": "7bf1af3c952c11bd716f7859887eb17fe9a5f4de",
    }
}


def _stream(tmp_path: Path, lines: list[dict]) -> Path:
    p = tmp_path / "cli-agent-stream.jsonl"
    p.write_text("\n".join(json.dumps(x) for x in lines), encoding="utf-8")
    return p


def _fetch(url: str) -> dict:
    return {
        "type": "tool_call",
        "subtype": "started",
        "tool_call": {"webFetchToolCall": {"args": {"url": url}}},
    }


def _search(term: str) -> dict:
    return {
        "type": "tool_call",
        "subtype": "started",
        "tool_call": {"webSearchToolCall": {"args": {"searchTerm": term}}},
    }


def test_upstream_refs_parses_provenance() -> None:
    refs = upstream_refs(META)
    assert refs == {"repo": "pennylaneai/pennylane", "pr": "9459", "commit": "7bf1af3c952c"}


def test_no_stream_is_no_web(tmp_path: Path) -> None:
    audit = audit_stream(tmp_path / "missing.jsonl", META)
    assert audit["verdict"] == "no_web"
    assert audit["contaminated"] is False


def test_rejected_calls_are_blocked_not_used(tmp_path: Path) -> None:
    # A denied call appears only as a completed event carrying a rejection: it
    # obtained nothing, so it must not count as access, but it is recorded --
    # an agent reaching for the web on a decontaminated task is worth seeing.
    rejected = {
        "type": "tool_call",
        "subtype": "completed",
        "tool_call": {"webSearchToolCall": {"result": {"rejected": {"reason": "User Rejected"}}}},
    }
    audit = audit_stream(_stream(tmp_path, [rejected]), META)
    assert audit["verdict"] == "web_blocked"
    assert audit["web_blocked"] == 1
    assert audit["web_searches"] == 0
    assert audit["contaminated"] is False


def test_blocked_upstream_fetch_is_not_contamination(tmp_path: Path) -> None:
    rejected = {
        "type": "tool_call",
        "subtype": "completed",
        "tool_call": {
            "webFetchToolCall": {
                "args": {"url": "https://github.com/PennyLaneAI/pennylane/pull/9459"},
                "result": {"rejected": {"reason": "User Rejected"}},
            }
        },
    }
    audit = audit_stream(_stream(tmp_path, [rejected]), META)
    assert audit["verdict"] == "web_blocked"
    assert audit["contaminated"] is False


def test_started_then_rejected_is_blocked_not_access(tmp_path: Path) -> None:
    # The real shape of a denied fetch: a started event carrying the url, then
    # a completed event carrying the rejection. Scoring the started event alone
    # flagged clean runs as contaminated.
    cid = "call-abc-1"
    started = {
        "type": "tool_call",
        "subtype": "started",
        "call_id": cid,
        "tool_call": {
            "webFetchToolCall": {
                "args": {
                    "url": "https://github.com/PennyLaneAI/pennylane/pull/9459",
                    "toolCallId": cid,
                }
            }
        },
    }
    completed = {
        "type": "tool_call",
        "subtype": "completed",
        "call_id": cid,
        "tool_call": {"webFetchToolCall": {"result": {"rejected": {"reason": "User Rejected"}}}},
    }
    audit = audit_stream(_stream(tmp_path, [started, completed]), META)
    assert audit["verdict"] == "web_blocked"
    assert audit["contaminated"] is False
    assert audit["web_fetches"] == 0
    assert audit["web_blocked"] == 1


def test_started_then_completed_ok_counts_once(tmp_path: Path) -> None:
    cid = "call-xyz-2"
    started = {
        "type": "tool_call",
        "subtype": "started",
        "call_id": cid,
        "tool_call": {
            "webFetchToolCall": {"args": {"url": "https://docs.python.org/3/", "toolCallId": cid}}
        },
    }
    completed = {
        "type": "tool_call",
        "subtype": "completed",
        "call_id": cid,
        "tool_call": {"webFetchToolCall": {"result": {"content": "ok"}}},
    }
    audit = audit_stream(_stream(tmp_path, [started, completed]), META)
    assert audit["verdict"] == "web_used"
    assert audit["web_fetches"] == 1  # not double counted


def test_unrelated_browsing_is_web_used(tmp_path: Path) -> None:
    p = _stream(tmp_path, [_search("python asyncio docs"), _fetch("https://docs.python.org/3/")])
    audit = audit_stream(p, META)
    assert audit["verdict"] == "web_used"
    assert audit["contaminated"] is False
    assert audit["web_fetches"] == 1
    assert "docs.python.org" in audit["hosts"]


def test_upstream_repo_fetch_is_contaminated(tmp_path: Path) -> None:
    # Post-fix sources on main contain the merged solution.
    p = _stream(
        tmp_path,
        [_fetch("https://raw.githubusercontent.com/PennyLaneAI/pennylane/master/pennylane/x.py")],
    )
    audit = audit_stream(p, META)
    assert audit["verdict"] == "upstream_access"
    assert audit["contaminated"] is True


def test_exact_pr_fetch_is_solution_retrieval(tmp_path: Path) -> None:
    p = _stream(tmp_path, [_fetch("https://github.com/PennyLaneAI/pennylane/pull/9459")])
    assert audit_stream(p, META)["verdict"] == "solution_retrieval"


def test_fix_commit_fetch_is_solution_retrieval(tmp_path: Path) -> None:
    p = _stream(
        tmp_path,
        [_fetch("https://github.com/PennyLaneAI/pennylane/commit/7bf1af3c952c11bd716f")],
    )
    assert audit_stream(p, META)["verdict"] == "solution_retrieval"


def test_upstream_diff_fetch_is_solution_retrieval(tmp_path: Path) -> None:
    p = _stream(
        tmp_path,
        [_fetch("https://github.com/PennyLaneAI/pennylane/compare/a...b.diff")],
    )
    assert audit_stream(p, META)["verdict"] == "solution_retrieval"


def test_pr_number_alone_on_other_repo_is_not_solution(tmp_path: Path) -> None:
    # /pull/9459 on an unrelated repo must not trip the tripwire.
    p = _stream(tmp_path, [_fetch("https://github.com/some/other-repo/pull/9459")])
    audit = audit_stream(p, META)
    assert audit["verdict"] == "web_used"


def test_task_without_upstream_never_flags_contamination(tmp_path: Path) -> None:
    p = _stream(tmp_path, [_fetch("https://github.com/PennyLaneAI/pennylane/pull/9459")])
    audit = audit_stream(p, {"id": "synthetic-task"})
    assert audit["verdict"] == "web_used"
    assert audit["contaminated"] is False


# ---------------------------------------------------------------- filesystem


def _read(path: str) -> dict:
    return {
        "type": "tool_call",
        "subtype": "completed",
        "tool_call": {"readToolCall": {"args": {"path": path}}},
    }


def _shell(cmd: str) -> dict:
    return {
        "type": "tool_call",
        "subtype": "completed",
        "tool_call": {"shellToolCall": {"args": {"command": cmd}}},
    }


def test_fs_workspace_only_is_clean(tmp_path: Path) -> None:
    ws = tmp_path / "workspace"
    ws.mkdir()
    p = _stream(tmp_path, [_read(str(ws / "src" / "app.py")), _shell(f"cd {ws} && pytest")])
    audit = audit_filesystem(p, ws, "oss-task", repo_root=tmp_path / "repo")
    assert audit["verdict"] == "clean"
    assert audit["contaminated"] is False


def test_fs_unrelated_outside_read_is_recorded_not_fatal(tmp_path: Path) -> None:
    ws = tmp_path / "workspace"
    ws.mkdir()
    p = _stream(tmp_path, [_read("/etc/hosts")])
    audit = audit_filesystem(p, ws, "oss-task", repo_root=tmp_path / "repo")
    assert audit["verdict"] == "out_of_workspace"
    assert audit["contaminated"] is False


def test_fs_own_gold_patch_is_answer_key_access(tmp_path: Path) -> None:
    ws = tmp_path / "workspace"
    ws.mkdir()
    repo = tmp_path / "repo"
    p = _stream(tmp_path, [_read(f"{repo}/tasks/v3/oss-task/gold_patch.diff")])
    audit = audit_filesystem(p, ws, "oss-task", repo_root=repo)
    assert audit["verdict"] == "answer_key_access"
    assert audit["contaminated"] is True


def test_fs_own_hidden_tests_is_answer_key_access(tmp_path: Path) -> None:
    ws = tmp_path / "workspace"
    ws.mkdir()
    repo = tmp_path / "repo"
    p = _stream(tmp_path, [_shell(f"cat {repo}/tasks/v3/oss-task/tests/oss_tests.py")])
    audit = audit_filesystem(p, ws, "oss-task", repo_root=repo)
    assert audit["verdict"] == "answer_key_access"


def test_fs_other_task_data_is_still_contamination(tmp_path: Path) -> None:
    ws = tmp_path / "workspace"
    ws.mkdir()
    repo = tmp_path / "repo"
    p = _stream(tmp_path, [_read(f"{repo}/tasks/v4/oss-other/gold_patch.diff")])
    audit = audit_filesystem(p, ws, "oss-task", repo_root=repo)
    assert audit["verdict"] == "benchmark_data_access"
    assert audit["contaminated"] is True


_CC_LOG = (
    "/private/tmp/claude-501/-private-var-folders-q0-T-vulcanbench-legacy-x-1a2a4a6a-workspace"
    "/a72a30a1-22df-4fb1-8c29-b09c5303d9ac/tasks/b8ae8jspq.output"
)


def test_fs_claude_code_background_log_is_not_benchmark_data(tmp_path: Path) -> None:
    ws = tmp_path / "workspace"
    ws.mkdir()
    p = _stream(
        tmp_path,
        [
            _shell(f'until [ -s "{_CC_LOG}" ]; do sleep 10; done; cat "{_CC_LOG}"'),
            _read("/a72a30a1-22df-4fb1-8c29-b09c5303d9ac/tasks/b8ae8jspq.output"),
            _read("/tasks/b8ae8jspq.output"),
        ],
    )
    audit = audit_filesystem(p, ws, "oss-task", repo_root=tmp_path / "repo")
    assert audit["verdict"] == "out_of_workspace"
    assert audit["benchmark_data_paths"] == []
    assert audit["contaminated"] is False


def test_fs_task_tree_under_claude_tmp_is_still_flagged(tmp_path: Path) -> None:
    ws = tmp_path / "workspace"
    ws.mkdir()
    p = _stream(tmp_path, [_read("/tmp/claude-501/slug/tasks/v4/oss-other/gold_patch.diff")])
    audit = audit_filesystem(p, ws, "oss-task", repo_root=tmp_path / "repo")
    assert audit["verdict"] == "benchmark_data_access"
    assert audit["contaminated"] is True


def test_audit_run_combines_both_channels(tmp_path: Path) -> None:
    ws = tmp_path / "workspace"
    ws.mkdir()
    repo = tmp_path / "repo"
    p = _stream(tmp_path, [_read(f"{repo}/tasks/v3/oss-task/gold_patch.diff")])
    audit = audit_run(p, {"id": "oss-task"}, ws, repo_root=repo)
    assert audit["web"]["verdict"] == "no_web"
    assert audit["filesystem"]["verdict"] == "answer_key_access"
    # A run clean on the web is still contaminated if it read the answer key.
    assert audit["contaminated"] is True
