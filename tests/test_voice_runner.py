"""Offline end-to-end tests: runner resume, manifest, CLI, report rendering,
and the TTS/STT HTTP seams (urllib faked)."""

from __future__ import annotations

import io
import json
import wave
from pathlib import Path
from typing import Any

import pytest

import harness.voice.cli as voice_cli
import harness.voice.runner as runner_mod
from harness.agent.providers import LLMProvider, LLMResponse, ProviderError, TokenUsage
from harness.voice.adapters import VoiceAnswer, VoiceModel
from harness.voice.audio import MASTER_RATE_HZ
from harness.voice.report import build_report, to_markdown
from harness.voice.runner import RunConfig, run_suite
from harness.voice.stt import transcribe
from harness.voice.tts import OpenAITTS, TTSProvider, get_tts
from tests.cli_runner import CliRunner


def _wav_bytes(seconds: float = 0.2) -> bytes:
    buf = io.BytesIO()
    with wave.open(buf, "wb") as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(MASTER_RATE_HZ)
        wf.writeframes(b"\x10\x10" * int(seconds * MASTER_RATE_HZ))
    return buf.getvalue()


class _FakeTTS(TTSProvider):
    name = "fake"
    model = "fake-1"

    def synthesize(self, text: str, voice: str, speed: float) -> bytes:
        return _wav_bytes()


class _EchoModel(VoiceModel):
    """Answers '4' to everything; optionally fails the first N calls."""

    min_interval_s = 0.0

    def __init__(self, fail_first: int = 0) -> None:
        self.slug = "fake-voice:echo"
        self.model = "echo"
        self.fail_first = fail_first
        self.calls = 0

    def _maybe_fail(self) -> None:
        self.calls += 1
        if self.calls <= self.fail_first:
            raise ProviderError("transient")

    def answer_text(self, question: str) -> VoiceAnswer:
        self._maybe_fail()
        return VoiceAnswer(text="4", t_first_s=0.1, t_total_s=0.2)

    def answer_audio(self, wav_path: Path) -> VoiceAnswer:
        self._maybe_fail()
        return VoiceAnswer(text="4", t_first_s=0.3, t_total_s=0.6)


class _AlwaysRight(LLMProvider):
    def __init__(self) -> None:
        super().__init__("fake-judge-model")

    @property
    def name(self) -> str:
        return "fake-judge"

    def complete(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        timeout_s: float | None = None,
        effort: str | None = None,
    ) -> LLMResponse:
        return LLMResponse(content='{"correct": true}', tool_calls=[], usage=TokenUsage())


def _write_questions(path: Path, n: int = 6) -> None:
    rows = [
        {
            "id": f"vq-ar-{i:03d}",
            "category": "arithmetic",
            "question": f"What is {i} plus {4 - i}?" if i <= 4 else "What is two plus two?",
            "answer": "4",
        }
        for i in range(n)
    ]
    path.write_text("\n".join(json.dumps(r) for r in rows) + "\n")


@pytest.fixture()
def offline(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> RunConfig:
    _write_questions(tmp_path / "q.jsonl")
    monkeypatch.setattr(runner_mod, "get_tts", lambda name: _FakeTTS())
    monkeypatch.setattr(runner_mod, "get_voice_model", lambda spec: _EchoModel())
    monkeypatch.setattr(runner_mod, "get_provider", lambda spec: _AlwaysRight())
    return RunConfig(
        models=["fake-voice:echo"],
        questions_path=tmp_path / "q.jsonl",
        out_dir=tmp_path / "runs",
        cache_dir=tmp_path / "cache",
        noise_dir=tmp_path / "noise",
        dry_run=True,
        run_id="voice-test-run",
    )


def test_run_suite_dry_run_end_to_end(offline: RunConfig) -> None:
    run_dir = run_suite(offline)
    rows = [json.loads(x) for x in (run_dir / "results.jsonl").read_text().splitlines()]
    assert len(rows) == 10  # 5 text + 5 audio
    assert all(r["correct"] for r in rows)
    manifest = json.loads((run_dir / "manifest.json").read_text())
    assert manifest["judge_model"] == offline.judge_model
    assert manifest["finished_at"] > manifest["started_at"]
    assert manifest["questions_sha256"]
    assert manifest["subset_ids"] == sorted(manifest["subset_ids"])


def test_run_suite_resumes_without_repeating(offline: RunConfig) -> None:
    run_dir = run_suite(offline)
    before = (run_dir / "results.jsonl").read_text()
    run_suite(offline)  # same run_id → everything already complete
    after = (run_dir / "results.jsonl").read_text()
    assert before == after


def test_run_suite_records_errors_and_retries_them_on_rerun(
    offline: RunConfig, monkeypatch: pytest.MonkeyPatch
) -> None:
    # First run: every call fails (fail_first > planned units * retries).
    broken = _EchoModel(fail_first=10_000)
    monkeypatch.setattr(runner_mod, "get_voice_model", lambda spec: broken)
    run_dir = run_suite(offline)
    rows = [json.loads(x) for x in (run_dir / "results.jsonl").read_text().splitlines()]
    assert rows and all(r["error"] for r in rows)
    # Second run with a healthy model: errored units are re-attempted.
    monkeypatch.setattr(runner_mod, "get_voice_model", lambda spec: _EchoModel())
    run_suite(offline)
    rows2 = [json.loads(x) for x in (run_dir / "results.jsonl").read_text().splitlines()]
    good = [r for r in rows2 if r["error"] is None]
    assert len(good) == 10


def test_transient_failures_retried_within_run(
    offline: RunConfig, monkeypatch: pytest.MonkeyPatch
) -> None:
    flaky = _EchoModel(fail_first=1)
    monkeypatch.setattr(runner_mod, "get_voice_model", lambda spec: flaky)
    monkeypatch.setattr(runner_mod.time, "sleep", lambda s: None)
    run_dir = run_suite(offline)
    rows = [json.loads(x) for x in (run_dir / "results.jsonl").read_text().splitlines()]
    assert all(r["error"] is None for r in rows)


def test_report_markdown_from_real_run(offline: RunConfig) -> None:
    run_dir = run_suite(offline)
    report = build_report(run_dir)
    md = to_markdown(report)
    assert "Voice tax" in md and "fake-voice:echo" in md
    assert report["models"]["fake-voice:echo"]["voice_tax"] == 0.0
    assert "latency" in report["models"]["fake-voice:echo"]


# --- CLI ---------------------------------------------------------------------


def test_cli_render_and_report(
    offline: RunConfig, monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    monkeypatch.setattr(voice_cli, "get_tts", lambda name: _FakeTTS())
    cli = CliRunner()
    res = cli.invoke(
        voice_cli.voice_main,
        [
            "render",
            "--questions", str(offline.questions_path),
            "--cache-dir", str(tmp_path / "cache2"),
            "--noise-dir", str(tmp_path / "noise"),
            "--subset-n", "0",
        ],
    )  # fmt: skip
    assert res.exit_code == 0, res.output
    assert "rendered" in res.output

    run_dir = run_suite(offline)
    out = tmp_path / "report.md"
    res = cli.invoke(voice_cli.voice_main, ["report", str(run_dir), "-o", str(out)])
    assert res.exit_code == 0, res.output
    assert "Voice tax" in out.read_text()
    res = cli.invoke(voice_cli.voice_main, ["report", str(run_dir), "--json"])
    assert res.exit_code == 0 and '"voice_tax"' in res.output


def test_cli_run_invokes_runner(offline: RunConfig, monkeypatch: pytest.MonkeyPatch) -> None:
    captured: dict[str, Any] = {}

    def fake_run_suite(cfg: RunConfig) -> Path:
        captured["cfg"] = cfg
        d = cfg.out_dir / cfg.run_id
        d.mkdir(parents=True, exist_ok=True)
        return d

    monkeypatch.setattr(voice_cli, "run_suite", fake_run_suite)
    cli = CliRunner()
    res = cli.invoke(
        voice_cli.voice_main,
        [
            "run", "-m", "fake-voice:echo,other:x",
            "--questions", str(offline.questions_path),
            "--output-dir", str(offline.out_dir),
            "--dry-run", "--run-id", "voice-fixed",
        ],
    )  # fmt: skip
    assert res.exit_code == 0, res.output
    cfg = captured["cfg"]
    assert cfg.models == ["fake-voice:echo", "other:x"]
    assert cfg.dry_run and cfg.run_id == "voice-fixed"


# --- TTS / STT HTTP seams ----------------------------------------------------


class _FakeHTTPResponse:
    def __init__(self, body: bytes) -> None:
        self.body = body

    def read(self) -> bytes:
        return self.body

    def __enter__(self) -> _FakeHTTPResponse:
        return self

    def __exit__(self, *a: object) -> None:
        return None


def test_openai_tts_posts_speed_and_voice(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "k")
    seen: dict[str, Any] = {}

    def fake_urlopen(req: Any, timeout: float = 0) -> _FakeHTTPResponse:
        seen["url"] = req.full_url
        seen["payload"] = json.loads(req.data.decode())
        return _FakeHTTPResponse(b"RIFFfake")

    monkeypatch.setattr("urllib.request.urlopen", fake_urlopen)
    out = OpenAITTS().synthesize("hello", "fable", 1.25)
    assert out == b"RIFFfake"
    assert seen["payload"]["voice"] == "fable"
    assert seen["payload"]["speed"] == 1.25
    assert seen["payload"]["response_format"] == "wav"


def test_openai_tts_requires_key(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("OPENAI_API_KEY", raising=False)
    with pytest.raises(ProviderError):
        OpenAITTS().synthesize("hi", "onyx", 1.0)


def test_get_tts_registry() -> None:
    assert isinstance(get_tts("openai"), OpenAITTS)
    with pytest.raises(ProviderError):
        get_tts("bogus")


def test_stt_transcribe_multipart(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "k")
    wav = tmp_path / "a.wav"
    wav.write_bytes(_wav_bytes())
    seen: dict[str, Any] = {}

    def fake_urlopen(req: Any, timeout: float = 0) -> _FakeHTTPResponse:
        seen["content_type"] = req.headers["Content-type"]
        seen["body"] = req.data
        return _FakeHTTPResponse(json.dumps({"text": "eighty eight"}).encode())

    monkeypatch.setattr("urllib.request.urlopen", fake_urlopen)
    assert transcribe(wav) == "eighty eight"
    assert "multipart/form-data" in seen["content_type"]
    assert b"gpt-4o-transcribe" in seen["body"]


def test_stt_missing_text_raises(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "k")
    wav = tmp_path / "a.wav"
    wav.write_bytes(_wav_bytes())
    monkeypatch.setattr(
        "urllib.request.urlopen",
        lambda req, timeout=0: _FakeHTTPResponse(json.dumps({"nope": 1}).encode()),
    )
    with pytest.raises(ProviderError):
        transcribe(wav)


def test_judge_error_rows_are_retried_on_resume(
    offline: RunConfig, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A row scored via judge-error is a non-verdict: resume must redo it."""

    class _LongAnswerModel(_EchoModel):
        def answer_text(self, question: str) -> VoiceAnswer:
            return VoiceAnswer(
                text="well now considering absolutely everything very carefully here my final answer to this question would definitely have to be 4 indeed",
                t_first_s=0.1,
                t_total_s=0.2,
            )

    class _BrokenJudge(_AlwaysRight):
        def complete(self, *a: Any, **k: Any) -> LLMResponse:
            raise ProviderError("judge down")

    monkeypatch.setattr(runner_mod, "get_voice_model", lambda spec: _LongAnswerModel())
    monkeypatch.setattr(runner_mod, "get_provider", lambda spec: _BrokenJudge())
    run_dir = run_suite(offline)
    rows = [json.loads(x) for x in (run_dir / "results.jsonl").read_text().splitlines()]
    text_rows = [r for r in rows if r["mode"] == "text"]
    assert all(r["score_method"] == "judge-error" for r in text_rows)

    # Judge recovers → resume re-attempts exactly the judge-error units.
    monkeypatch.setattr(runner_mod, "get_provider", lambda spec: _AlwaysRight())
    run_suite(offline)
    rows2 = [json.loads(x) for x in (run_dir / "results.jsonl").read_text().splitlines()]
    latest: dict[str, dict[str, Any]] = {}
    for r in rows2:
        latest[f"{r['model']}|{r['mode']}|{r['question_id']}|{r['condition_slug']}"] = r
    assert all(r["correct"] for r in latest.values())


def test_report_uses_latest_row_per_unit(offline: RunConfig, tmp_path: Path) -> None:
    """Superseded attempts in the append-only log must not dilute accuracy."""
    run = offline.out_dir / "voice-dedup"
    run.mkdir(parents=True)
    manifest = {
        "run_id": "voice-dedup", "models": ["m"], "judge_model": "j",
        "tts": {"provider": "openai", "voices": ["onyx"]}, "seed": 1, "n_items": 1,
    }  # fmt: skip
    (run / "manifest.json").write_text(json.dumps(manifest))
    base = {
        "model": "m", "question_id": "q0", "category": "arithmetic",
        "t_first_s": 0.1, "t_total_s": 0.2,
    }  # fmt: skip
    rows = [
        # attempt 1: infra error; attempt 2: judge-error scored wrong; attempt 3: correct
        {**base, "mode": "text", "condition_slug": "text", "condition": None,
         "error": "boom", "correct": None, "score_method": None},
        {**base, "mode": "text", "condition_slug": "text", "condition": None,
         "error": None, "correct": False, "score_method": "judge-error"},
        {**base, "mode": "text", "condition_slug": "text", "condition": None,
         "error": None, "correct": True, "score_method": "judge"},
        {**base, "mode": "audio", "condition_slug": "onyx_normal_clean",
         "condition": {"voice": "onyx", "rate": "normal", "noise": "clean"},
         "error": None, "correct": True, "score_method": "exact"},
    ]  # fmt: skip
    (run / "results.jsonl").write_text("\n".join(json.dumps(r) for r in rows) + "\n")
    report = build_report(run)
    assert report["models"]["m"]["text_accuracy"] == 1.0
    assert report["models"]["m"]["voice_tax"] == 0.0
    assert report["n_errors"] == 0  # superseded error row no longer counted
