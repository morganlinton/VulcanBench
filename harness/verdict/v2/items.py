"""The v2 item shape and the shared helpers every family builder uses.

An item is a ``state`` (what the model reads), one typed ``question`` and a
verified ``answer``, plus the ``source_unit`` it came from (a task, a repo or
a generator seed group). Splits and bootstrap intervals both work on source
units, so correlated items never straddle dev and test or narrow an interval.

Builders also record, per item, what each of the family's shortcut
baselines would answer (``shortcuts``). Scoring takes the best of those as
part of the floor a model must beat.

Item files embed benchmark tasks: keep them out of git and out of anything
public.
"""

from __future__ import annotations

import hashlib
import json
import random
import re
from collections.abc import Sequence
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

SUITE_ID = "verdict-v2"
CANARY = "4c1d7e52-8a0b-4f3e-9d61-2b7f0c95e8a4"
DEV_FRACTION = 0.2
MAX_CHOICE_OPTIONS = 255
MAX_SCORE_LEVELS = 10
# The API allows 32k tokens for state plus the longest question; four
# characters a token is conservative for English and code.
MAX_STATE_CHARS = 110_000
QUESTION_TYPES = frozenset({"noul", "choice", "score"})
# Where an answer can come from. There is deliberately no "llm-panel" here:
# v2 answers never come from a model's opinion.
REFERENCES = frozenset({"execution", "tool", "generator", "verifier", "merged-fix", "advisory"})


@dataclass(frozen=True)
class Item:
    item_id: str
    family: str
    split: str
    source_unit: str
    state: str
    question: dict[str, Any]
    answer: Any
    reference: str
    shortcuts: dict[str, str] = field(default_factory=dict)
    source: dict[str, Any] = field(default_factory=dict)

    def to_json(self) -> str:
        return json.dumps({"suite": SUITE_ID, "canary": CANARY, **asdict(self)}, sort_keys=True)


_SEED_GROUP = re.compile(r"-g(\d+)$")


def split_for_unit(source_unit: str) -> str:
    """Dev or test, stable for a given source unit so splits never drift.

    Generated families name their units ``<family>-g<k>``; every fifth group
    goes to dev, so each such family gets exactly a fifth of its groups (a
    hash split left one with 3 dev groups of 50). Real sources (tasks, repos)
    split by hash.
    """
    group = _SEED_GROUP.search(source_unit)
    if group:
        return "dev" if int(group.group(1)) % round(1 / DEV_FRACTION) == 0 else "test"
    digest = hashlib.sha256(f"{SUITE_ID}:{source_unit}".encode()).hexdigest()
    return "dev" if int(digest[:8], 16) / 0xFFFFFFFF < DEV_FRACTION else "test"


def item_id(family: str, key: str) -> str:
    return f"{family}-{hashlib.sha256(key.encode()).hexdigest()[:12]}"


def answer_label(question: dict[str, Any], answer: Any) -> str:
    if question["type"] == "noul":
        return "true" if answer else "false"
    return str(answer)


def labels_for(question: dict[str, Any]) -> list[str]:
    kind = question["type"]
    if kind == "noul":
        return ["true", "false"]
    if kind == "choice":
        return [str(o) for o in question["options"]]
    return [str(level) for level in question["levels"]]


def validate(item: Item) -> None:
    """Raise ValueError when an item cannot be sent or scored as built."""
    q = item.question
    if q.get("type") not in QUESTION_TYPES:
        raise ValueError(f"{item.item_id}: unknown question type {q.get('type')!r}")
    if not q.get("instructions"):
        raise ValueError(f"{item.item_id}: question has no instructions")
    if item.reference not in REFERENCES:
        raise ValueError(f"{item.item_id}: reference {item.reference!r} is not ground truth")
    labels = labels_for(q)
    if len(set(labels)) != len(labels):
        raise ValueError(f"{item.item_id}: duplicate options or levels")
    if q["type"] == "choice" and not 2 <= len(labels) <= MAX_CHOICE_OPTIONS:
        raise ValueError(f"{item.item_id}: choice needs 2 to {MAX_CHOICE_OPTIONS} options")
    if q["type"] == "score" and not 2 <= len(labels) <= MAX_SCORE_LEVELS:
        raise ValueError(f"{item.item_id}: score needs 2 to {MAX_SCORE_LEVELS} levels")
    if answer_label(q, item.answer) not in labels:
        raise ValueError(f"{item.item_id}: answer {item.answer!r} is not an option")
    for name, guess in item.shortcuts.items():
        if guess not in labels:
            raise ValueError(f"{item.item_id}: shortcut {name} answers {guess!r}, not an option")
    if len(item.state) > MAX_STATE_CHARS:
        raise ValueError(f"{item.item_id}: state is {len(item.state)} chars")


def make_item(
    *,
    family: str,
    key: str,
    source_unit: str,
    state: str,
    question: dict[str, Any],
    answer: Any,
    reference: str,
    shortcuts: dict[str, str] | None = None,
    source: dict[str, Any] | None = None,
) -> Item:
    item = Item(
        item_id=item_id(family, key),
        family=family,
        split=split_for_unit(source_unit),
        source_unit=source_unit,
        state=state,
        question=question,
        answer=answer,
        reference=reference,
        shortcuts=shortcuts or {},
        source=source or {},
    )
    validate(item)
    return item


def shuffled_options(
    rng: random.Random, correct: str, distractors: Sequence[str]
) -> tuple[list[str], str]:
    """Options in random order; returns (option labels, correct label).

    Labels are letters so the position, not the text, is what the model picks;
    the text goes in the choice descriptions.
    """
    texts = [correct, *distractors]
    if len(set(texts)) != len(texts):
        raise ValueError("options must be distinct")
    order = list(range(len(texts)))
    rng.shuffle(order)
    options = [chr(ord("A") + i) for i in range(len(texts))]
    mapping = {options[pos]: texts[src] for pos, src in enumerate(order)}
    correct_label = next(label for label, text in mapping.items() if text == correct)
    return options, correct_label


def choice_question(instructions: str, descriptions: dict[str, str | None]) -> dict[str, Any]:
    return {
        "type": "choice",
        "instructions": instructions,
        "options": list(descriptions),
        "descriptions": descriptions,
    }


def noul_question(instructions: str) -> dict[str, Any]:
    return {"type": "noul", "instructions": instructions}


def score_question(instructions: str, levels: Sequence[str]) -> dict[str, Any]:
    return {"type": "score", "instructions": instructions, "levels": list(levels)}


def write_items(items: Sequence[Item], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(item.to_json() + "\n" for item in items))


def load_items(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
