"""Voice model adapters: one contract, two input modalities.

Every adapter implements :class:`VoiceModel` with ``answer_text(question)``
and ``answer_audio(wav_path)``. Both paths share one system prompt and, per
adapter, one endpoint, for realtime models the text baseline goes through
the *same realtime session type* with a text item, so the only variable
between modes is the input modality, not the serving stack.

Network seams (``_connect`` / ``_post_stream``) are deliberately thin and
monkeypatched in tests; all framing/parsing logic is exercised offline.

Adapters:

- ``openai-realtime``: OpenAI Realtime API over websocket. Audio in as
  24 kHz PCM16; text output requested (``output_modalities: ["text"]``).
- ``gemini-live``: Gemini Live API over websocket. Audio downsampled to
  16 kHz at send time (API requirement); TEXT response modality.
- ``qwen-omni``: Qwen3-Omni via DashScope's OpenAI-compatible endpoint
  (audio as base64 WAV content part; streaming SSE, text modality).
- ``grok-voice``: xAI Grok Voice (speech-to-speech) over websocket. Pinned
  to ``grok-voice-think-fast-2.0`` (``grok-voice-latest`` aliases 1.0 until
  2026-08-05). Output is spoken audio plus the model's own transcript; the
  transcript is scored, with the pinned STT as fallback when absent. Both
  modes reply in audio, only the *input* modality differs, which is the
  quantity under measurement.
"""

from __future__ import annotations

import base64
import json
import os
import tempfile
import time
import urllib.request
from abc import ABC, abstractmethod
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

from websockets.sync.client import connect as _ws_connect

from harness.agent.providers import ProviderError
from harness.voice.audio import MASTER_RATE_HZ, read_wav, resample, write_wav
from harness.voice.stt import STT_MODEL, transcribe

#: Identical for every model and both modalities; recorded in the manifest.
SYSTEM_PROMPT = (
    "Answer the question with only the final answer, as briefly as possible. "
    "Do not explain or repeat the question."
)

GEMINI_INPUT_RATE_HZ = 16_000
_TURN_TIMEOUT_S = 120.0
_RECV_TIMEOUT_S = 90.0


@dataclass(kw_only=True)
class VoiceAnswer:
    text: str
    t_first_s: float
    t_total_s: float
    output_modality: str = "text"
    transcribed_by: str | None = None


class _WSLike(Protocol):
    def send(self, message: str) -> None: ...
    def recv(self, timeout: float | None = None) -> str | bytes: ...
    def close(self) -> None: ...


class VoiceModel(ABC):
    """A model reachable in both text and audio input modes."""

    slug: str
    model: str
    #: Minimum seconds between request starts (crude provider rate limit).
    min_interval_s: float = 1.0

    def __init__(self, model: str | None = None) -> None:
        """Subclasses set ``self.model``/``self.slug``; this default exists so
        the registry can instantiate any adapter as ``cls()`` or ``cls(model)``."""
        del model

    @abstractmethod
    def answer_text(self, question: str) -> VoiceAnswer: ...

    @abstractmethod
    def answer_audio(self, wav_path: Path) -> VoiceAnswer: ...


def _b64_wav_pcm(wav_path: Path, target_hz: int) -> str:
    frames, rate = read_wav(wav_path)
    frames = resample(frames, rate, target_hz)
    return base64.b64encode(frames).decode()


class OpenAIRealtimeModel(VoiceModel):
    """OpenAI Realtime API; both modes run through a realtime session."""

    min_interval_s = 1.0

    def __init__(self, model: str = "gpt-realtime") -> None:
        self.model = model
        self.slug = f"openai-realtime:{model}"

    def _connect(self) -> _WSLike:  # pragma: no cover - network seam
        api_key = os.environ.get("OPENAI_API_KEY")
        if not api_key:
            raise ProviderError("OPENAI_API_KEY is not set")
        return _ws_connect(
            f"wss://api.openai.com/v1/realtime?model={self.model}",
            additional_headers={"Authorization": f"Bearer {api_key}"},
            max_size=16 * 1024 * 1024,
        )

    def _session_update(self) -> dict[str, Any]:
        # GA Realtime session shape: session.type is required, text output is
        # requested via output_modalities, and audio input config (format +
        # manual turn control) nests under audio.input.
        return {
            "type": "session.update",
            "session": {
                "type": "realtime",
                "instructions": SYSTEM_PROMPT,
                "output_modalities": ["text"],
                "audio": {
                    "input": {
                        "format": {"type": "audio/pcm", "rate": MASTER_RATE_HZ},
                        "turn_detection": None,
                    }
                },
            },
        }

    def _run_turn(self, client_events: list[dict[str, Any]]) -> VoiceAnswer:
        ws = self._connect()
        t0 = time.monotonic()
        t_first: float | None = None
        parts: list[str] = []
        final_text: str | None = None
        try:
            ws.send(json.dumps(self._session_update()))
            for ev in client_events:
                ws.send(json.dumps(ev))
            ws.send(
                json.dumps({"type": "response.create", "response": {"output_modalities": ["text"]}})
            )
            while True:
                if time.monotonic() - t0 > _TURN_TIMEOUT_S:
                    raise ProviderError(f"{self.slug}: turn exceeded {_TURN_TIMEOUT_S:.0f}s")
                raw = ws.recv(timeout=_RECV_TIMEOUT_S)
                event = json.loads(raw)
                etype = str(event.get("type", ""))
                if etype == "error":
                    raise ProviderError(f"{self.slug}: {json.dumps(event.get('error'))[:300]}")
                if etype.endswith(".delta"):
                    delta = event.get("delta")
                    if isinstance(delta, str) and delta:
                        if t_first is None:
                            t_first = time.monotonic() - t0
                        parts.append(delta)
                if etype == "response.done":
                    final_text = _extract_realtime_final(event)
                    break
        finally:
            ws.close()
        text = final_text if final_text is not None else "".join(parts)
        t_total = time.monotonic() - t0
        return VoiceAnswer(text=text, t_first_s=t_first or t_total, t_total_s=t_total)

    def answer_text(self, question: str) -> VoiceAnswer:
        item = {
            "type": "conversation.item.create",
            "item": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": question}],
            },
        }
        return self._run_turn([item])

    def answer_audio(self, wav_path: Path) -> VoiceAnswer:
        audio_b64 = _b64_wav_pcm(wav_path, MASTER_RATE_HZ)
        return self._run_turn(
            [
                {"type": "input_audio_buffer.append", "audio": audio_b64},
                {"type": "input_audio_buffer.commit"},
            ]
        )


def _extract_realtime_final(done_event: dict[str, Any]) -> str | None:
    """Pull the final text out of ``response.done`` (authoritative over deltas)."""
    response = done_event.get("response")
    if not isinstance(response, dict):
        return None
    chunks: list[str] = []
    for item in response.get("output", []) or []:
        if not isinstance(item, dict):
            continue
        for part in item.get("content", []) or []:
            if isinstance(part, dict):
                text = part.get("text") or part.get("transcript")
                if isinstance(text, str):
                    chunks.append(text)
    return "".join(chunks) if chunks else None


class GeminiLiveModel(VoiceModel):
    """Gemini Live API (BidiGenerateContent websocket)."""

    min_interval_s = 2.0

    def __init__(self, model: str = "gemini-live-2.5-flash-preview") -> None:
        self.model = model
        self.slug = f"gemini-live:{model}"

    def _connect(self) -> _WSLike:  # pragma: no cover - network seam
        api_key = os.environ.get("GEMINI_API_KEY") or os.environ.get("GOOGLE_API_KEY")
        if not api_key:
            raise ProviderError("GEMINI_API_KEY is not set")
        host = "generativelanguage.googleapis.com"
        service = "google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent"
        return _ws_connect(f"wss://{host}/ws/{service}?key={api_key}", max_size=16 * 1024 * 1024)

    def _setup_message(self) -> dict[str, Any]:
        return {
            "setup": {
                "model": f"models/{self.model}",
                "generationConfig": {"responseModalities": ["TEXT"], "temperature": 0},
                "systemInstruction": {"parts": [{"text": SYSTEM_PROMPT}]},
            }
        }

    def _run_turn(self, parts: list[dict[str, Any]]) -> VoiceAnswer:
        ws = self._connect()
        t0 = time.monotonic()
        t_first: float | None = None
        chunks: list[str] = []
        try:
            ws.send(json.dumps(self._setup_message()))
            first = json.loads(ws.recv(timeout=_RECV_TIMEOUT_S))
            if "setupComplete" not in first:
                raise ProviderError(f"{self.slug}: setup failed: {json.dumps(first)[:300]}")
            ws.send(
                json.dumps(
                    {
                        "clientContent": {
                            "turns": [{"role": "user", "parts": parts}],
                            "turnComplete": True,
                        }
                    }
                )
            )
            while True:
                if time.monotonic() - t0 > _TURN_TIMEOUT_S:
                    raise ProviderError(f"{self.slug}: turn exceeded {_TURN_TIMEOUT_S:.0f}s")
                event = json.loads(ws.recv(timeout=_RECV_TIMEOUT_S))
                server = event.get("serverContent")
                if not isinstance(server, dict):
                    continue
                turn = server.get("modelTurn")
                if isinstance(turn, dict):
                    for part in turn.get("parts", []) or []:
                        text = part.get("text") if isinstance(part, dict) else None
                        if isinstance(text, str) and text:
                            if t_first is None:
                                t_first = time.monotonic() - t0
                            chunks.append(text)
                if server.get("turnComplete"):
                    break
        finally:
            ws.close()
        t_total = time.monotonic() - t0
        return VoiceAnswer(text="".join(chunks), t_first_s=t_first or t_total, t_total_s=t_total)

    def answer_text(self, question: str) -> VoiceAnswer:
        return self._run_turn([{"text": question}])

    def answer_audio(self, wav_path: Path) -> VoiceAnswer:
        audio_b64 = _b64_wav_pcm(wav_path, GEMINI_INPUT_RATE_HZ)
        mime = f"audio/pcm;rate={GEMINI_INPUT_RATE_HZ}"
        return self._run_turn([{"inlineData": {"mimeType": mime, "data": audio_b64}}])


class QwenOmniModel(VoiceModel):
    """Qwen3-Omni via DashScope's OpenAI-compatible endpoint (SSE streaming;
    the omni models only support streaming output)."""

    min_interval_s = 1.0

    def __init__(self, model: str = "qwen3-omni-flash") -> None:
        self.model = model
        self.slug = f"qwen-omni:{model}"

    def _base_url(self) -> str:
        return os.environ.get(
            "DASHSCOPE_BASE_URL", "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"
        ).rstrip("/")

    def _post_stream(self, payload: dict[str, Any]) -> Iterator[dict[str, Any]]:
        # pragma: no cover - network seam
        api_key = os.environ.get("DASHSCOPE_API_KEY")
        if not api_key:
            raise ProviderError("DASHSCOPE_API_KEY is not set")
        req = urllib.request.Request(
            f"{self._base_url()}/chat/completions",
            data=json.dumps(payload).encode(),
            headers={
                "Authorization": f"Bearer {api_key}",
                "Content-Type": "application/json",
            },
        )
        with urllib.request.urlopen(req, timeout=_TURN_TIMEOUT_S) as resp:
            for raw_line in resp:
                line = raw_line.decode("utf-8", errors="replace").strip()
                if not line.startswith("data:"):
                    continue
                data = line[len("data:") :].strip()
                if data == "[DONE]":
                    return
                yield json.loads(data)

    def _run(self, content: list[dict[str, Any]] | str) -> VoiceAnswer:
        payload = {
            "model": self.model,
            "stream": True,
            "temperature": 0,
            "modalities": ["text"],
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": content},
            ],
        }
        t0 = time.monotonic()
        t_first: float | None = None
        chunks: list[str] = []
        for event in self._post_stream(payload):
            for choice in event.get("choices", []) or []:
                delta = choice.get("delta") if isinstance(choice, dict) else None
                text = delta.get("content") if isinstance(delta, dict) else None
                if isinstance(text, str) and text:
                    if t_first is None:
                        t_first = time.monotonic() - t0
                    chunks.append(text)
        t_total = time.monotonic() - t0
        return VoiceAnswer(text="".join(chunks), t_first_s=t_first or t_total, t_total_s=t_total)

    def answer_text(self, question: str) -> VoiceAnswer:
        return self._run(question)

    def answer_audio(self, wav_path: Path) -> VoiceAnswer:
        b64 = base64.b64encode(wav_path.read_bytes()).decode()
        return self._run(
            [
                {
                    "type": "input_audio",
                    "input_audio": {"data": f"data:audio/wav;base64,{b64}", "format": "wav"},
                }
            ]
        )


class GrokVoiceModel(VoiceModel):
    """xAI Grok Voice realtime API (speech-to-speech websocket).

    Same event family as OpenAI Realtime. ``turn_detection: null`` puts the
    session in manual-turn mode; input audio is 24 kHz PCM16 (the suite's
    master format, no resampling). The model answers in audio with transcript
    events; ``response.done`` carries the authoritative transcript. When no
    transcript arrives, collected output audio goes through the pinned STT
    (recorded per-row as ``transcribed_by``). ``reasoning.effort`` is left at
    the provider default ("high") and recorded in the session payload.
    """

    min_interval_s = 1.0
    voice = "eve"

    def __init__(self, model: str = "grok-voice-think-fast-2.0") -> None:
        self.model = model
        self.slug = f"grok-voice:{model}"

    def _connect(self) -> _WSLike:  # pragma: no cover - network seam
        api_key = os.environ.get("XAI_API_KEY")
        if not api_key:
            raise ProviderError("XAI_API_KEY is not set")
        return _ws_connect(
            f"wss://api.x.ai/v1/realtime?model={self.model}",
            additional_headers={"Authorization": f"Bearer {api_key}"},
            max_size=16 * 1024 * 1024,
        )

    def _session_update(self) -> dict[str, Any]:
        return {
            "type": "session.update",
            "session": {
                "instructions": SYSTEM_PROMPT,
                "voice": self.voice,
                "turn_detection": None,
                "audio": {
                    "input": {"format": {"type": "audio/pcm", "rate": MASTER_RATE_HZ}},
                    "output": {"format": {"type": "audio/pcm", "rate": MASTER_RATE_HZ}},
                },
            },
        }

    def _run_turn(self, client_events: list[dict[str, Any]]) -> VoiceAnswer:  # noqa: PLR0912
        ws = self._connect()
        t0 = time.monotonic()
        t_first: float | None = None
        transcript_parts: list[str] = []
        audio_b64_parts: list[str] = []
        final_text: str | None = None
        try:
            ws.send(json.dumps(self._session_update()))
            for ev in client_events:
                ws.send(json.dumps(ev))
            ws.send(json.dumps({"type": "response.create"}))
            while True:
                if time.monotonic() - t0 > _TURN_TIMEOUT_S:
                    raise ProviderError(f"{self.slug}: turn exceeded {_TURN_TIMEOUT_S:.0f}s")
                raw = ws.recv(timeout=_RECV_TIMEOUT_S)
                if isinstance(raw, bytes):  # binary transport not requested; skip
                    continue
                event = json.loads(raw)
                etype = str(event.get("type", ""))
                if etype == "error":
                    raise ProviderError(f"{self.slug}: {json.dumps(event.get('error'))[:300]}")
                if etype.endswith(".delta"):
                    delta = event.get("delta")
                    if not isinstance(delta, str) or not delta:
                        continue
                    if t_first is None:
                        t_first = time.monotonic() - t0
                    if "audio_transcript" in etype or "text" in etype:
                        transcript_parts.append(delta)
                    elif "audio" in etype:
                        audio_b64_parts.append(delta)
                if etype == "response.done":
                    final_text = _extract_realtime_final(event)
                    break
        finally:
            ws.close()
        t_total = time.monotonic() - t0
        text = final_text if final_text is not None else "".join(transcript_parts)
        transcribed_by: str | None = None
        if not text.strip() and audio_b64_parts:
            text, transcribed_by = self._stt_fallback(audio_b64_parts)
        return VoiceAnswer(
            text=text,
            t_first_s=t_first or t_total,
            t_total_s=t_total,
            output_modality="audio",
            transcribed_by=transcribed_by,
        )

    def _stt_fallback(self, audio_b64_parts: list[str]) -> tuple[str, str]:
        """Transcribe collected output audio when no transcript was emitted."""
        pcm = b"".join(base64.b64decode(p) for p in audio_b64_parts)
        with tempfile.TemporaryDirectory() as td:
            wav = Path(td) / "grok-answer.wav"
            write_wav(wav, pcm, MASTER_RATE_HZ)
            return transcribe(wav), STT_MODEL

    def answer_text(self, question: str) -> VoiceAnswer:
        item = {
            "type": "conversation.item.create",
            "item": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": question}],
            },
        }
        return self._run_turn([item])

    def answer_audio(self, wav_path: Path) -> VoiceAnswer:
        audio_b64 = _b64_wav_pcm(wav_path, MASTER_RATE_HZ)
        return self._run_turn(
            [
                {"type": "input_audio_buffer.append", "audio": audio_b64},
                {"type": "input_audio_buffer.commit"},
            ]
        )


_ADAPTERS: dict[str, type[VoiceModel]] = {
    "openai-realtime": OpenAIRealtimeModel,
    "gemini-live": GeminiLiveModel,
    "qwen-omni": QwenOmniModel,
    "grok-voice": GrokVoiceModel,
}


def get_voice_model(spec: str) -> VoiceModel:
    """Resolve ``adapter`` or ``adapter:model`` (e.g. ``openai-realtime:gpt-realtime``)."""
    adapter, _, model = spec.partition(":")
    try:
        cls = _ADAPTERS[adapter]
    except KeyError:
        raise ProviderError(
            f"unknown voice adapter {adapter!r}; available: {sorted(_ADAPTERS)}"
        ) from None
    return cls(model) if model else cls()
