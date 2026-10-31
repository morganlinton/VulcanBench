"""First-attempt check verdicts over synthetic Claude and Codex streams."""

from __future__ import annotations

from datetime import timedelta
from pathlib import Path

from backend.runner import firstcheck
from tests.runner_fixtures import T0, claude_stream, codex_stream


def _claude(tmp_path: Path, text: str) -> Path:
    p = tmp_path / "claude-code.txt"
    p.write_text(text)
    return p


def _eval(ev: firstcheck.StreamEvidence | None, **kw: object) -> firstcheck.FirstCheck:
    args: dict[str, object] = {
        "requested_model": "claude-opus-5-5",
        "agent_started_at": T0,
        "now": T0 + timedelta(minutes=10),
        "window_s": 180,
        "trial_finished": False,
    }
    args.update(kw)
    return firstcheck.evaluate(ev, **args)  # type: ignore[arg-type]


def test_ok_when_requested_model_made_tool_calls(tmp_path: Path) -> None:
    ev = firstcheck.scan_claude_stream(_claude(tmp_path, claude_stream(tool_calls=2)))
    assert ev is not None and ev.init_model == "claude-opus-5-5" and ev.tool_calls == 2
    fc = _eval(ev)
    assert fc.status == "ok" and fc.models == {"claude-opus-5-5": 3} and fc.window_open is False


def test_pending_inside_window_then_fail_without_tool_calls(tmp_path: Path) -> None:
    ev = firstcheck.scan_claude_stream(_claude(tmp_path, claude_stream(turns=1, tool_calls=0)))
    early = _eval(ev, now=T0 + timedelta(seconds=30))
    assert early.status == "pending" and early.window_open is True
    late = _eval(ev, now=T0 + timedelta(minutes=5))
    assert late.status == "fail" and "no tool calls" in late.reasons[0]
    finished_early = _eval(ev, now=T0 + timedelta(seconds=30), trial_finished=True)
    assert finished_early.status == "fail"


def test_fallback_model_turns_fail(tmp_path: Path) -> None:
    ev = firstcheck.scan_claude_stream(_claude(tmp_path, claude_stream(wrong_model_turns=2)))
    fc = _eval(ev)
    assert fc.status == "fail"
    assert any("claude-opus-4-8" in r for r in fc.reasons)
    assert fc.models == {"claude-opus-5-5": 3, "claude-opus-4-8": 2}


def test_wrong_init_model_fails_even_with_tool_calls(tmp_path: Path) -> None:
    ev = firstcheck.scan_claude_stream(_claude(tmp_path, claude_stream(model="claude-opus-4-8")))
    fc = _eval(ev)
    assert fc.status == "fail" and "initialised with claude-opus-4-8" in fc.reasons[0]


def test_infra_pattern_and_rate_limit_status(tmp_path: Path) -> None:
    infra = '{"type":"result","is_error":true,"error":"authentication_failed"}'
    ev = firstcheck.scan_claude_stream(_claude(tmp_path, claude_stream(infra_line=infra)))
    fc = _eval(ev)
    assert fc.status == "fail" and fc.infra_hits == ['"error":"authentication_failed"']

    ev2 = firstcheck.scan_claude_stream(
        _claude(tmp_path / "b", claude_stream(rate_limit_status="allowed_warning"))
        if (tmp_path / "b").mkdir() is None
        else Path()
    )
    fc2 = _eval(ev2)
    assert fc2.status == "warning" and fc2.rate_limit_status == "allowed_warning"


def test_codex_stream_counts_commands_and_takes_model_from_trial_log(tmp_path: Path) -> None:
    trial = tmp_path / "t__abc"
    (trial / "agent").mkdir(parents=True)
    (trial / "agent" / "codex.txt").write_text(codex_stream(commands=4))
    (trial / "trial.log").write_text(
        "Running command: codex exec --dangerously-bypass-approvals-and-sandbox "
        "--model gpt-6-astra --json -c model_reasoning_effort=medium -- 'x'\n"
    )
    fc = firstcheck.check_agent_dir(
        trial,
        harness="codex",
        requested_model="gpt-6-astra",
        agent_started_at=T0,
        now=T0 + timedelta(minutes=1),
        window_s=180,
        trial_finished=False,
    )
    assert fc.status == "ok" and fc.tool_calls == 4 and fc.models == {"gpt-6-astra": 0}
    assert fc.evidence == "codex.txt"


def test_missing_stream_is_unknown_and_partial_lines_are_tolerated(tmp_path: Path) -> None:
    assert firstcheck.scan_claude_stream(tmp_path / "nope.txt") is None
    fc = _eval(None)
    assert fc.status == "unknown"
    p = _claude(tmp_path, claude_stream(tool_calls=1) + '{"type":"assistant","message":{"mod')
    ev = firstcheck.scan_claude_stream(p)
    assert ev is not None and ev.tool_calls == 1


def test_scan_is_cached_until_the_file_grows(tmp_path: Path) -> None:
    p = _claude(tmp_path, claude_stream(tool_calls=1))
    first = firstcheck.scan_claude_stream(p)
    assert firstcheck.scan_claude_stream(p) is first
    with p.open("a") as fh:
        fh.write(claude_stream(tool_calls=1).splitlines()[1] + "\n")
    second = firstcheck.scan_claude_stream(p)
    assert second is not first and second is not None and second.tool_calls == 2


def test_requested_model_unknown_is_a_warning(tmp_path: Path) -> None:
    ev = firstcheck.scan_claude_stream(_claude(tmp_path, claude_stream()))
    fc = _eval(ev, requested_model=None)
    assert fc.status == "warning" and fc.reasons == ["requested model unknown"]
