"""Audio cache keying, noise mixing SNR, subset determinism, adapter parsing,
and report aggregation for the voice suite, all offline."""

from __future__ import annotations

import audioop
import base64
import io
import json
import math
import struct
import wave
from pathlib import Path
from typing import Any

import pytest

import harness.voice.adapters as adapters_mod
from harness.agent.providers import ProviderError
from harness.voice.adapters import (
    GeminiLiveModel,
    GrokVoiceModel,
    OpenAIRealtimeModel,
    QwenOmniModel,
    _extract_realtime_final,
    get_voice_model,
)
from harness.voice.audio import (
    MASTER_RATE_HZ,
    AudioCache,
    Condition,
    mix_noise,
    pick_noise_clip,
    read_wav,
    resample,
    write_wav,
)
from harness.voice.items import VoiceItem, item_text_sha256, load_items
from harness.voice.report import build_report, latency_stats
from harness.voice.runner import RunConfig, plan_units, subset_ids
from harness.voice.tts import TTSProvider


def _tone(seconds: float = 0.5, rate: int = MASTER_RATE_HZ, freq: float = 440.0) -> bytes:
    n = int(seconds * rate)
    return b"".join(
        struct.pack("<h", int(12000 * math.sin(2 * math.pi * freq * i / rate))) for i in range(n)
    )


def _item(item_id: str = "vq-ar-001", question: str = "What is two plus two?") -> VoiceItem:
    return VoiceItem.from_dict(
        {"id": item_id, "category": "arithmetic", "question": question, "answer": "4"}
    )


class _FakeTTS(TTSProvider):
    name = "fake"
    model = "fake-1"

    def __init__(self) -> None:
        self.calls: list[tuple[str, str, float]] = []

    def synthesize(self, text: str, voice: str, speed: float) -> bytes:
        self.calls.append((text, voice, speed))
        buf = io.BytesIO()
        with wave.open(buf, "wb") as wf:
            wf.setnchannels(1)
            wf.setsampwidth(2)
            wf.setframerate(MASTER_RATE_HZ)
            wf.writeframes(_tone())
        return buf.getvalue()


# --- cache keying ------------------------------------------------------------


def test_cache_path_encodes_full_condition(tmp_path: Path) -> None:
    cache = AudioCache(tmp_path)
    cond = Condition(voice="onyx", rate="fast", noise="noise10db")
    assert cache.path_for("vq-x-001", cond) == tmp_path / "vq-x-001" / "onyx_fast_noise10db.wav"


def test_cache_renders_once_then_reuses(tmp_path: Path) -> None:
    cache = AudioCache(tmp_path)
    tts = _FakeTTS()
    item = _item()
    cond = Condition(voice="onyx", rate="normal", noise="clean")
    p1 = cache.ensure(item, cond, tts)
    p2 = cache.ensure(item, cond, tts)
    assert p1 == p2 and p1.exists()
    assert len(tts.calls) == 1  # second call was a cache hit


def test_cache_invalidated_when_question_text_changes(tmp_path: Path) -> None:
    cache = AudioCache(tmp_path)
    tts = _FakeTTS()
    cond = Condition(voice="onyx", rate="normal", noise="clean")
    cache.ensure(_item(), cond, tts)
    edited = _item(question="What is three plus three?")
    assert not cache.is_fresh(edited, cond)
    cache.ensure(edited, cond, tts)
    assert len(tts.calls) == 2
    manifest = json.loads((tmp_path / "vq-ar-001" / "manifest.json").read_text())
    assert manifest[cond.slug] == item_text_sha256(edited)


def test_cache_distinct_conditions_distinct_files(tmp_path: Path) -> None:
    cache = AudioCache(tmp_path)
    tts = _FakeTTS()
    item = _item()
    a = cache.ensure(item, Condition("onyx", "normal", "clean"), tts)
    b = cache.ensure(item, Condition("shimmer", "normal", "clean"), tts)
    c = cache.ensure(item, Condition("onyx", "fast", "clean"), tts)
    assert len({a, b, c}) == 3
    assert [call[2] for call in tts.calls] == [1.0, 1.0, 1.25]  # speed passed through


def test_tts_speed_used_for_fast_condition(tmp_path: Path) -> None:
    cache = AudioCache(tmp_path)
    tts = _FakeTTS()
    cache.ensure(_item(), Condition("fable", "fast", "clean"), tts)
    assert tts.calls[0][1:] == ("fable", 1.25)


# --- noise mixing ------------------------------------------------------------


def test_mix_noise_hits_target_snr(tmp_path: Path) -> None:
    speech = _tone(seconds=1.0, freq=440.0)
    noise = _tone(seconds=0.3, freq=97.0)  # will be looped
    mixed = mix_noise(speech, noise, snr_db=10.0)
    assert len(mixed) == len(speech)
    # Reconstruct the scaled noise the mixer should have produced and check
    # the achieved SNR is within 0.5 dB of the target.
    speech_rms = audioop.rms(speech, 2)
    reps = (len(speech) // len(noise)) + 1
    looped = (noise * reps)[: len(speech)]
    target_rms = speech_rms / (10 ** (10.0 / 20.0))
    scaled = audioop.mul(looped, 2, target_rms / audioop.rms(looped, 2))
    achieved = 20 * math.log10(speech_rms / audioop.rms(scaled, 2))
    assert abs(achieved - 10.0) < 0.5


def test_mix_noise_rejects_silence() -> None:
    with pytest.raises(ValueError):
        mix_noise(b"\x00" * 4800, _tone(0.1))


def test_pick_noise_clip_deterministic() -> None:
    clips = [Path("noise/street.wav"), Path("noise/crowd.wav")]
    first = pick_noise_clip("vq-ar-001", clips)
    assert all(pick_noise_clip("vq-ar-001", clips) == first for _ in range(5))
    picks = {pick_noise_clip(f"vq-x-{i:03d}", clips) for i in range(40)}
    assert picks == set(clips)  # both clips actually get used


def test_resample_halves_length() -> None:
    frames = _tone(seconds=1.0, rate=24_000)
    down = resample(frames, 24_000, 12_000)
    assert abs(len(down) - len(frames) // 2) <= 4


def test_wav_roundtrip(tmp_path: Path) -> None:
    frames = _tone()
    write_wav(tmp_path / "t.wav", frames, MASTER_RATE_HZ)
    back, rate = read_wav(tmp_path / "t.wav")
    assert rate == MASTER_RATE_HZ and back == frames


# --- subset + planning determinism ------------------------------------------


def _items(n: int = 20) -> list[VoiceItem]:
    return [_item(item_id=f"vq-ar-{i:03d}") for i in range(n)]


def test_subset_ids_deterministic_and_sorted() -> None:
    items = _items(50)
    a = subset_ids(items, 10, 20260729)
    b = subset_ids(items, 10, 20260729)
    assert a == b == sorted(a)
    assert subset_ids(items, 10, 1) != a  # seed actually matters


def _cfg(tmp_path: Path, **kw: Any) -> RunConfig:
    defaults: dict[str, Any] = {
        "models": ["openai-realtime"],
        "questions_path": tmp_path / "q.jsonl",
        "out_dir": tmp_path / "runs",
        "cache_dir": tmp_path / "cache",
        "noise_dir": tmp_path / "noise",
        "subset_n": 5,
    }
    defaults.update(kw)
    return RunConfig(**defaults)


def test_plan_full_matrix_counts(tmp_path: Path) -> None:
    items = _items(20)
    cfg = _cfg(tmp_path)
    units = plan_units(cfg, items)
    # text(20) + 3 voices * clean/normal(20 each) + subset(5) * (fast + noise)
    assert len(units) == 20 + 60 + 10
    assert len({u.key for u in units}) == len(units)


def test_plan_dry_run_is_small(tmp_path: Path) -> None:
    units = plan_units(_cfg(tmp_path, dry_run=True), _items(20))
    # 5 text + 5 audio (primary voice, clean/normal)
    assert len(units) == 10


def test_questions_file_loads_and_matches_suite_manifest() -> None:
    items = load_items(Path("tasks/voice-v1/questions.jsonl"))
    suite = json.loads(Path("tasks/voice-v1/suite.json").read_text())
    assert len(items) == suite["tasks"] == 200
    by_cat: dict[str, int] = {}
    for it in items:
        by_cat[it.category] = by_cat.get(it.category, 0) + 1
    for cat, count in by_cat.items():
        assert suite[cat] == count


# --- adapter parsing (transport faked) --------------------------------------


class _FakeWS:
    def __init__(self, incoming: list[dict[str, Any]]) -> None:
        self.incoming = [json.dumps(e) for e in incoming]
        self.sent: list[dict[str, Any]] = []
        self.closed = False

    def send(self, message: str) -> None:
        self.sent.append(json.loads(message))

    def recv(self, timeout: float | None = None) -> str:
        return self.incoming.pop(0)

    def close(self) -> None:
        self.closed = True


def test_openai_realtime_text_turn(monkeypatch: pytest.MonkeyPatch) -> None:
    events = [
        {"type": "response.output_text.delta", "delta": "8"},
        {"type": "response.output_text.delta", "delta": "8"},
        {
            "type": "response.done",
            "response": {"output": [{"content": [{"type": "output_text", "text": "88"}]}]},
        },
    ]
    ws = _FakeWS(events)
    model = OpenAIRealtimeModel()
    monkeypatch.setattr(model, "_connect", lambda: ws)
    answer = model.answer_text("What is 17 times 6, minus 14?")
    assert answer.text == "88"
    assert ws.closed
    types = [m["type"] for m in ws.sent]
    assert types == ["session.update", "conversation.item.create", "response.create"]


def test_openai_realtime_error_event_raises(monkeypatch: pytest.MonkeyPatch) -> None:
    ws = _FakeWS([{"type": "error", "error": {"message": "bad session"}}])
    model = OpenAIRealtimeModel()
    monkeypatch.setattr(model, "_connect", lambda: ws)
    with pytest.raises(ProviderError):
        model.answer_text("q")
    assert ws.closed


def test_openai_realtime_audio_sends_buffer(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    write_wav(tmp_path / "q.wav", _tone(0.2), MASTER_RATE_HZ)
    ws = _FakeWS(
        [{"type": "response.done", "response": {"output": [{"content": [{"text": "ok"}]}]}}]
    )
    model = OpenAIRealtimeModel()
    monkeypatch.setattr(model, "_connect", lambda: ws)
    answer = model.answer_audio(tmp_path / "q.wav")
    assert answer.text == "ok"
    types = [m["type"] for m in ws.sent]
    assert "input_audio_buffer.append" in types and "input_audio_buffer.commit" in types


def test_extract_realtime_final_prefers_done_payload() -> None:
    assert _extract_realtime_final({"response": {"output": []}}) is None
    done = {"response": {"output": [{"content": [{"text": "a"}, {"transcript": "b"}]}]}}
    assert _extract_realtime_final(done) == "ab"


def test_gemini_live_turn(monkeypatch: pytest.MonkeyPatch) -> None:
    ws = _FakeWS(
        [
            {"setupComplete": {}},
            {"serverContent": {"modelTurn": {"parts": [{"text": "Ottawa"}]}}},
            {"serverContent": {"turnComplete": True}},
        ]
    )
    model = GeminiLiveModel()
    monkeypatch.setattr(model, "_connect", lambda: ws)
    answer = model.answer_text("Capital of Canada?")
    assert answer.text == "Ottawa"
    assert "setup" in ws.sent[0]
    assert ws.sent[1]["clientContent"]["turnComplete"] is True


def test_gemini_audio_downsampled_to_16k(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    write_wav(tmp_path / "q.wav", _tone(0.5), MASTER_RATE_HZ)
    ws = _FakeWS([{"setupComplete": {}}, {"serverContent": {"turnComplete": True}}])
    model = GeminiLiveModel()
    monkeypatch.setattr(model, "_connect", lambda: ws)
    model.answer_audio(tmp_path / "q.wav")
    part = ws.sent[1]["clientContent"]["turns"][0]["parts"][0]["inlineData"]
    assert part["mimeType"] == "audio/pcm;rate=16000"
    pcm = base64.b64decode(part["data"])
    assert abs(len(pcm) - int(0.5 * 16000) * 2) <= 8


def test_qwen_omni_stream_and_payload(monkeypatch: pytest.MonkeyPatch) -> None:
    seen: dict[str, Any] = {}

    def fake_stream(payload: dict[str, Any]) -> Any:
        seen.update(payload)
        yield {"choices": [{"delta": {"content": "12"}}]}
        yield {"choices": [{"delta": {"content": "6"}}]}

    model = QwenOmniModel()
    monkeypatch.setattr(model, "_post_stream", fake_stream)
    answer = model.answer_text("18 dollars an hour for 7 hours?")
    assert answer.text == "126"
    assert seen["stream"] is True and seen["temperature"] == 0
    assert seen["messages"][0]["role"] == "system"


def test_get_voice_model_specs() -> None:
    assert get_voice_model("openai-realtime").slug == "openai-realtime:gpt-realtime"
    assert get_voice_model("gemini-live:custom-model").model == "custom-model"
    with pytest.raises(ProviderError):
        get_voice_model("nope")


# --- report ------------------------------------------------------------------


def test_latency_stats_percentiles() -> None:
    stats = latency_stats([float(i) for i in range(1, 101)])
    assert stats["median"] == 50.5
    assert stats["p95"] == 95.05
    assert stats["p99"] == 99.01


def test_build_report_voice_tax(tmp_path: Path) -> None:
    run = tmp_path / "voice-test"
    run.mkdir()
    manifest = {
        "run_id": "voice-test",
        "models": ["m"],
        "judge_model": "j",
        "tts": {"provider": "openai", "voices": ["onyx"]},
        "seed": 1,
        "n_items": 4,
    }
    (run / "manifest.json").write_text(json.dumps(manifest))
    rows = []
    for i in range(4):
        rows.append(
            {
                "model": "m", "mode": "text", "question_id": f"q{i}", "category": "arithmetic",
                "condition_slug": "text", "condition": None, "correct": True,
                "score_method": "exact", "t_first_s": 0.5, "t_total_s": 1.0, "error": None,
            }
        )  # fmt: skip
        rows.append(
            {
                "model": "m", "mode": "audio", "question_id": f"q{i}", "category": "arithmetic",
                "condition_slug": "onyx_normal_clean",
                "condition": {"voice": "onyx", "rate": "normal", "noise": "clean"},
                "correct": i < 3, "score_method": "exact",
                "t_first_s": 1.0, "t_total_s": 2.0, "error": None,
            }
        )  # fmt: skip
    (run / "results.jsonl").write_text("\n".join(json.dumps(r) for r in rows) + "\n")
    report = build_report(run)
    model = report["models"]["m"]
    assert model["text_accuracy"] == 1.0
    assert model["audio_accuracy_clean_normal"] == 0.75
    assert model["voice_tax"] == 0.25
    assert model["categories"]["arithmetic"]["tax"] == 0.25


# --- grok-voice adapter ------------------------------------------------------


def test_grok_voice_text_turn_scores_transcript(monkeypatch: pytest.MonkeyPatch) -> None:
    events = [
        {"type": "response.output_audio.delta", "delta": "QUJD"},  # audio, ignored for text
        {"type": "response.output_audio_transcript.delta", "delta": "Otta"},
        {"type": "response.output_audio_transcript.delta", "delta": "wa"},
        {
            "type": "response.done",
            "response": {"output": [{"content": [{"transcript": "Ottawa"}]}]},
        },
    ]
    ws = _FakeWS(events)
    model = GrokVoiceModel()
    monkeypatch.setattr(model, "_connect", lambda: ws)
    answer = model.answer_text("Capital of Canada?")
    assert answer.text == "Ottawa"
    assert answer.output_modality == "audio"
    assert answer.transcribed_by is None
    session = ws.sent[0]["session"]
    assert session["turn_detection"] is None
    assert session["audio"]["input"]["format"] == {"type": "audio/pcm", "rate": 24000}
    assert ws.sent[-1] == {"type": "response.create"}


def test_grok_voice_audio_turn_sends_pcm_and_commit(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    write_wav(tmp_path / "q.wav", _tone(0.2), MASTER_RATE_HZ)
    ws = _FakeWS(
        [{"type": "response.done", "response": {"output": [{"content": [{"transcript": "88"}]}]}}]
    )
    model = GrokVoiceModel()
    monkeypatch.setattr(model, "_connect", lambda: ws)
    answer = model.answer_audio(tmp_path / "q.wav")
    assert answer.text == "88"
    types = [m["type"] for m in ws.sent]
    assert types == [
        "session.update",
        "input_audio_buffer.append",
        "input_audio_buffer.commit",
        "response.create",
    ]


def test_grok_voice_stt_fallback_when_no_transcript(monkeypatch: pytest.MonkeyPatch) -> None:

    pcm_b64 = base64.b64encode(_tone(0.1)).decode()
    events = [
        {"type": "response.output_audio.delta", "delta": pcm_b64},
        {"type": "response.done", "response": {"output": []}},
    ]
    ws = _FakeWS(events)
    model = GrokVoiceModel()
    monkeypatch.setattr(model, "_connect", lambda: ws)
    seen: dict[str, Any] = {}

    def fake_transcribe(wav_path: Path, model: str = "gpt-4o-transcribe") -> str:
        seen["wav"] = wav_path.name
        return "forty two"

    monkeypatch.setattr(adapters_mod, "transcribe", fake_transcribe)
    answer = model.answer_text("q")
    assert answer.text == "forty two"
    assert answer.transcribed_by == "gpt-4o-transcribe"
    assert seen["wav"].endswith(".wav")


def test_grok_voice_default_model_is_pinned() -> None:
    model = get_voice_model("grok-voice")
    assert model.model == "grok-voice-think-fast-2.0"  # not grok-voice-latest (aliases 1.0)
