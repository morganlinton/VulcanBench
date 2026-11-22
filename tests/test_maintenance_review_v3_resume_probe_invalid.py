"""invalidate_unrecoverable_probe when neither probe attempt is valid JSON.

The real case: v3.32 Grok probe submission-048 (codeccore). Both attempts quoted
the line with unescaped double quotes, so each reply failed json.loads with
"Expecting ',' delimiter" before any protocol check could run.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from harness import maintenance_review_v3_resume as resume


def make_call(root: Path, errors: tuple[str, str]) -> Path:
    folder = root / "calls" / "grok" / "probe" / "submission-048"
    folder.mkdir(parents=True)
    for n, error in enumerate(errors, start=1):
        (folder / f"attempt-{n}.json").write_text(
            json.dumps({"status": "failed", "retryable": True, "error": error})
        )
    return folder


@pytest.fixture
def out(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    monkeypatch.setattr(resume, "_out", lambda: tmp_path)
    return tmp_path


def test_two_malformed_replies_invalidate_the_probe(out: Path) -> None:
    folder = make_call(
        out,
        (
            "Expecting ',' delimiter: line 1 column 1978 (char 1977)",
            "Expecting ',' delimiter: line 1 column 2096 (char 2095)",
        ),
    )
    assert resume.invalidate_unrecoverable_probe(folder, "grok", "probe")
    marker = json.loads((folder / resume.INVALID_MARKER).read_text())
    assert marker["finding"] == "No valid probe response: neither attempt was valid JSON."
    assert set(marker["malformed_attempts"]) == {"1", "2"}
    assert marker["unsupported_excerpts"] == {}


def test_marked_call_is_not_marked_again(out: Path) -> None:
    folder = make_call(
        out, ("Expecting value: line 1 column 1 (char 0)", "Unterminated string starting at")
    )
    assert resume.invalidate_unrecoverable_probe(folder, "grok", "probe")
    assert not resume.invalidate_unrecoverable_probe(folder, "grok", "probe")


def test_extra_data_is_left_to_the_trailing_brace_rule(out: Path) -> None:
    folder = make_call(out, ("Extra data: line 3 column 1 (char 900)", "Expecting ',' delimiter"))
    assert not resume.invalidate_unrecoverable_probe(folder, "grok", "probe")
    assert not (folder / resume.INVALID_MARKER).exists()


def test_other_failures_still_stop(out: Path) -> None:
    folder = make_call(out, ("Expecting ',' delimiter", "Judge attempted tool use"))
    assert not resume.invalidate_unrecoverable_probe(folder, "grok", "probe")


def test_only_the_probe_stage(out: Path) -> None:
    folder = make_call(out, ("Expecting ',' delimiter", "Expecting ',' delimiter"))
    assert not resume.invalidate_unrecoverable_probe(folder, "grok", "primary")
