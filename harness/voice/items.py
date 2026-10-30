"""Question set loading and validation for the voice suite.

Items live in ``tasks/voice-v1/questions.jsonl``: one JSON object per line.
The set is held out by construction (written for this suite, never copied
from public benchmarks) and is reviewed in-repo like any other task source.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field, fields
from pathlib import Path
from typing import Literal

Category = Literal[
    "arithmetic",
    "multi-step-reasoning",
    "general-knowledge",
    "instruction-following",
    "numeric-extraction",
]

CATEGORIES: tuple[str, ...] = (
    "arithmetic",
    "multi-step-reasoning",
    "general-knowledge",
    "instruction-following",
    "numeric-extraction",
)


@dataclass(kw_only=True)
class VoiceItem:
    """One short-form question with an objectively checkable answer."""

    id: str
    category: Category
    question: str
    answer: str
    #: Alternative surface forms accepted as correct verbatim (post-normalization).
    accept: list[str] = field(default_factory=list)

    def __post_init__(self) -> None:
        for name in ("id", "category", "question", "answer"):
            if not isinstance(getattr(self, name), str):
                raise ValueError(f"{name} must be a string")
        if self.category not in CATEGORIES:
            raise ValueError(
                f"category must be one of {', '.join(CATEGORIES)}, got {self.category!r}"
            )
        self.question = self.question.strip()
        self.answer = self.answer.strip()
        if not self.question:
            raise ValueError("question must be non-empty")
        if not self.answer:
            raise ValueError("answer must be non-empty")
        if not isinstance(self.accept, list) or not all(isinstance(a, str) for a in self.accept):
            raise ValueError("accept must be a list of strings")

    @classmethod
    def from_dict(cls, raw: object) -> VoiceItem:
        """Validate one decoded ``questions.jsonl`` record (unknown keys are ignored)."""
        if not isinstance(raw, dict):
            raise ValueError("item must be a JSON object")
        missing = [k for k in ("id", "category", "question", "answer") if k not in raw]
        if missing:
            raise ValueError(f"missing field(s): {', '.join(missing)}")
        names = {f.name for f in fields(cls)}
        return cls(**{k: v for k, v in raw.items() if k in names})


def load_items(path: Path) -> list[VoiceItem]:
    """Load and validate ``questions.jsonl``; IDs must be unique."""
    items: list[VoiceItem] = []
    seen: set[str] = set()
    for lineno, raw in enumerate(path.read_text().splitlines(), start=1):
        if not raw.strip():
            continue
        try:
            item = VoiceItem.from_dict(json.loads(raw))
        except (json.JSONDecodeError, ValueError) as exc:
            raise ValueError(f"{path}:{lineno}: invalid item: {exc}") from exc
        if item.id in seen:
            raise ValueError(f"{path}:{lineno}: duplicate item id {item.id!r}")
        seen.add(item.id)
        items.append(item)
    if not items:
        raise ValueError(f"{path}: no items found")
    return items


def questions_sha256(path: Path) -> str:
    """Content hash of the question file, recorded in run manifests."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def item_text_sha256(item: VoiceItem) -> str:
    """Hash of the spoken text; used to invalidate cached audio on edits."""
    return hashlib.sha256(item.question.encode()).hexdigest()
