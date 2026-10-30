"""Scoring logic for the voice suite: normalization, matching, judge path.

The scorer is the validity linchpin of the voice-tax comparison, it must be
modality-blind and stable under STT rendering differences (digits vs number
words, punctuation, filler phrasing).
"""

from __future__ import annotations

import inspect
from typing import Any

from harness.agent.providers import LLMProvider, LLMResponse, ProviderError, TokenUsage
from harness.voice.items import VoiceItem
from harness.voice.scorer import normalize, score_response


def _item(**kw: Any) -> VoiceItem:
    base: dict[str, Any] = {
        "id": "vq-ar-001",
        "category": "arithmetic",
        "question": "What is 17 times 6, minus 14?",
        "answer": "88",
    }
    base.update(kw)
    return VoiceItem.from_dict(base)


class _FakeJudge(LLMProvider):
    def __init__(self, reply: str | None, raises: bool = False) -> None:
        self.reply = reply
        self.raises = raises
        self.calls = 0

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
        self.calls += 1
        if self.raises:
            raise ProviderError("judge unavailable")
        return LLMResponse(content=self.reply, tool_calls=[], usage=TokenUsage())


# --- normalization -----------------------------------------------------------


def test_normalize_folds_case_punctuation_articles() -> None:
    assert normalize("The Answer, is: OTTAWA!") == "answer is ottawa"


def test_normalize_number_words() -> None:
    assert normalize("eighty-eight") == "88"
    assert normalize("forty two") == "42"
    assert normalize("seven") == "7"
    assert normalize("one hundred twenty") == "120"
    assert normalize("three hundred sixty five") == "365"
    assert normalize("two hundred eight") == "208"


def test_normalize_decimal_zero_stripped() -> None:
    assert normalize("88.0") == "88"
    assert normalize("88.00") == "88"


def test_normalize_leaves_unknown_words_alone() -> None:
    assert normalize("about a million") == "about million"


# --- exact/alias matching ----------------------------------------------------


def test_exact_match_digits_and_words_agree() -> None:
    item = _item()
    for text in ("88", "eighty-eight", "The answer is 88.", "It's eighty eight"):
        result = score_response(item, text)
        assert result.correct, text
        assert result.method == "exact"


def test_embedded_match_rejected_for_long_responses() -> None:
    # An enumeration that happens to contain the reference must not score.
    item = _item()
    ramble = "it could be 86 or 87 or 88 or 89 " * 3
    assert not score_response(item, ramble).correct


def test_alias_match() -> None:
    item = _item(answer="one quarter", accept=["1/4", "0.25", "25 percent"])
    assert score_response(item, "0.25").method == "alias"
    assert score_response(item, "one quarter").method == "exact"
    assert not score_response(item, "one half").correct


def test_empty_response_is_wrong() -> None:
    assert not score_response(_item(), "   ").correct


# --- judge path --------------------------------------------------------------


def test_judge_accepts_free_form_equivalent() -> None:
    judge = _FakeJudge('{"correct": true}')
    result = score_response(_item(answer="Mount Everest"), "That would be Everest.", judge)
    assert result.correct and result.method == "judge"
    assert judge.calls == 1


def test_judge_not_called_when_exact_match() -> None:
    judge = _FakeJudge('{"correct": true}')
    score_response(_item(), "88", judge)
    assert judge.calls == 0


def test_judge_rejects() -> None:
    judge = _FakeJudge('{"correct": false}')
    assert not score_response(_item(), "86", judge).correct


def test_judge_error_scores_wrong_with_method_flag() -> None:
    result = score_response(_item(), "eighty six maybe", _FakeJudge(None, raises=True))
    assert not result.correct
    assert result.method == "judge-error"


def test_judge_garbage_reply_flagged() -> None:
    result = score_response(_item(), "eighty six", _FakeJudge("no json here"))
    assert result.method == "judge-error"


def test_scorer_is_modality_blind() -> None:
    """Structural symmetry: the scorer takes only (item, text, judge), there
    is no modality parameter to diverge on."""

    params = list(inspect.signature(score_response).parameters)
    assert params == ["item", "response_text", "judge"]
