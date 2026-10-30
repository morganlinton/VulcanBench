"""Mode-blind scoring for the voice suite.

Validity of the voice-tax comparison rests on one property: the scorer is
**byte-identical between text mode and audio mode**. There is exactly one
entry point (:func:`score_response`) taking ``(item, response_text)``: it
has no knowledge of which modality produced the text.

Pipeline: normalize → exact/alias match → optional LLM judge for free-form
phrasings. Normalization folds case, punctuation, articles, and number words
(``"eighty-eight"`` → ``"88"``) so that STT rendering differences ("42" vs
"forty-two") cannot masquerade as accuracy differences. The judge model is
pinned in the run manifest and the rubric lives at
``tasks/voice-v1/RUBRIC.md``.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Literal

from harness.agent.providers import LLMProvider, ProviderError
from harness.voice.items import VoiceItem

JUDGE_RUBRIC_PATH = "tasks/voice-v1/RUBRIC.md"

_UNITS = {
    "zero": 0, "one": 1, "two": 2, "three": 3, "four": 4, "five": 5, "six": 6,
    "seven": 7, "eight": 8, "nine": 9, "ten": 10, "eleven": 11, "twelve": 12,
    "thirteen": 13, "fourteen": 14, "fifteen": 15, "sixteen": 16,
    "seventeen": 17, "eighteen": 18, "nineteen": 19,
}  # fmt: skip
_TENS = {
    "twenty": 20, "thirty": 30, "forty": 40, "fifty": 50,
    "sixty": 60, "seventy": 70, "eighty": 80, "ninety": 90,
}  # fmt: skip
_ARTICLES = {"a", "an", "the"}


def _fold_number_words(tokens: list[str]) -> list[str]:
    """Fold spoken-number tokens into digits, conservatively.

    Handles units, tens, ``tens units`` pairs, and ``N hundred [M]``: the
    forms STT actually produces for this suite's short numeric answers.
    Anything more elaborate is left untouched rather than guessed at.
    """
    out: list[str] = []
    i = 0
    while i < len(tokens):
        tok = tokens[i]
        value: int | None = None
        consumed = 1
        if tok in _TENS:
            value = _TENS[tok]
            if i + 1 < len(tokens) and tokens[i + 1] in _UNITS and _UNITS[tokens[i + 1]] < 10:
                value += _UNITS[tokens[i + 1]]
                consumed = 2
        elif tok in _UNITS:
            value = _UNITS[tok]
            if i + 1 < len(tokens) and tokens[i + 1] == "hundred":
                value *= 100
                consumed = 2
                j = i + consumed
                if j < len(tokens) and tokens[j] in _TENS:
                    extra = _TENS[tokens[j]]
                    consumed += 1
                    k = i + consumed
                    if k < len(tokens) and tokens[k] in _UNITS and _UNITS[tokens[k]] < 10:
                        extra += _UNITS[tokens[k]]
                        consumed += 1
                    value += extra
                elif j < len(tokens) and tokens[j] in _UNITS:
                    value += _UNITS[tokens[j]]
                    consumed += 1
        if value is not None:
            out.append(str(value))
            i += consumed
        else:
            out.append(tok)
            i += 1
    return out


def normalize(text: str) -> str:
    """Canonical comparison form: casefold, strip punctuation and articles,
    fold number words to digits, collapse whitespace."""
    text = text.casefold()
    # "42.0" / "42.00" → "42" before punctuation strip would split the dot.
    text = re.sub(r"(\d+)\.0+\b", r"\1", text)
    # Hyphenated compounds ("eighty-eight") split before punctuation strip.
    text = text.replace("-", " ")
    # Note: this also splits true decimals ("5.2" → "5 2"), identically on
    # both the reference and the response, so comparisons stay symmetric.
    text = re.sub(r"[^\w\s]", " ", text)
    tokens = [t for t in text.split() if t not in _ARTICLES]
    tokens = _fold_number_words(tokens)
    return " ".join(tokens)


@dataclass(kw_only=True)
class ScoreResult:
    correct: bool
    method: Literal["exact", "alias", "judge", "judge-error"]
    judge_raw: str | None = None


def _judge_messages(item: VoiceItem, response_text: str) -> list[dict[str, str]]:
    """Build the judge conversation. Rubric wording lives in RUBRIC.md and
    must stay in sync with this prompt."""
    return [
        {
            "role": "system",
            "content": (
                "You grade short quiz answers. Decide whether the candidate answer "
                "is factually equivalent to the reference answer for the question. "
                "Ignore phrasing, verbosity, casing, and number formatting. Extra "
                "correct context is fine; a hedge between multiple answers is wrong. "
                'Respond with ONLY a JSON object: {"correct": true|false}.'
            ),
        },
        {
            "role": "user",
            "content": (
                f"Question: {item.question}\n"
                f"Reference answer: {item.answer}\n"
                f"Candidate answer: {response_text}"
            ),
        },
    ]


def score_response(  # noqa: PLR0911 - each branch is one scoring method
    item: VoiceItem,
    response_text: str,
    judge: LLMProvider | None = None,
) -> ScoreResult:
    """Score a response against the reference. Modality-blind by design."""
    got = normalize(response_text)
    want = normalize(item.answer)
    if not got:
        return ScoreResult(correct=False, method="exact")
    if got == want or (f" {want} " in f" {got} " and len(got.split()) <= 12):
        # Exact, or the reference appears intact inside a short direct answer
        # ("the answer is 88"). Long responses must go to the judge instead so
        # enumerations can't sneak a hit.
        return ScoreResult(correct=True, method="exact")
    for alias in item.accept:
        norm_alias = normalize(alias)
        if got == norm_alias or (f" {norm_alias} " in f" {got} " and len(got.split()) <= 12):
            return ScoreResult(correct=True, method="alias")
    if judge is None:
        return ScoreResult(correct=False, method="exact")
    try:
        resp = judge.complete(_judge_messages(item, response_text), [])
    except ProviderError as exc:
        return ScoreResult(correct=False, method="judge-error", judge_raw=str(exc)[:300])
    raw = (resp.content or "").strip()
    match = re.search(r"\{[^{}]*\}", raw)
    if match:
        try:
            verdict = json.loads(match.group(0))
            if isinstance(verdict.get("correct"), bool):
                return ScoreResult(correct=verdict["correct"], method="judge", judge_raw=raw[:300])
        except json.JSONDecodeError:
            pass
    return ScoreResult(correct=False, method="judge-error", judge_raw=raw[:300])
