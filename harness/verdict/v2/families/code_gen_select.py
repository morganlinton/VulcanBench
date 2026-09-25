"""Balancing and shortcut helpers shared by the code-generation families.

Choice families pick, per item, among several candidate option sets (which
distractors, in which order) the one that keeps every shortcut baseline's
running hit rate closest to chance and the answer position closest to
uniform. Yes/no families are balanced by stratified sampling: items are
grouped by the vector of shortcut predictions and each group contributes as
many true items as false ones, which pins every shortcut at 50%.
"""

from __future__ import annotations

import random
import re
import statistics
from collections import Counter, defaultdict
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from typing import Any

_NUMBER = re.compile(r"-?\d+(?:\.\d+)?(?:e-?\d+)?")


def numbers(text: str) -> list[float]:
    return [float(m) for m in _NUMBER.findall(text)]


def shape(text: str) -> str:
    """Digits become 9, letters a, runs collapse: '[3, 10]' -> '[9, 9]'."""
    out = re.sub(r"\d+(?:\.\d+)?", "9", text)
    out = re.sub(r"[A-Za-z_]+", "a", out)
    return re.sub(r"\s+", " ", out)


def first_label_max(labels: Sequence[str], key: Callable[[str], float]) -> str:
    """Label with the largest key; ties go to the earliest label."""
    best = labels[0]
    for label in labels[1:]:
        if key(label) > key(best):
            best = label
    return best


def longest(texts: dict[str, str]) -> str:
    labels = list(texts)
    return first_label_max(labels, lambda label: float(len(texts[label])))


def most_common_shape(texts: dict[str, str]) -> str:
    labels = list(texts)
    counts = Counter(shape(t) for t in texts.values())
    return first_label_max(labels, lambda label: float(counts[shape(texts[label])]))


def nearest_center(texts: dict[str, str], center: str) -> str:
    """Option whose mean number is nearest the median (or mean) across options."""
    labels = list(texts)
    values = {label: statistics.fmean(nums) for label in labels if (nums := numbers(texts[label]))}
    if len(values) < 2:
        return labels[0]
    pool = list(values.values())
    target = statistics.median(pool) if center == "median" else statistics.fmean(pool)
    return first_label_max(
        [label for label in labels if label in values], lambda label: -abs(values[label] - target)
    )


@dataclass
class ChoiceConfig:
    """One way to present an item: option texts by label and the right label."""

    texts: dict[str, str]
    correct: str
    shortcuts: dict[str, str]
    payload: Any = None


@dataclass
class ChoiceBalancer:
    """Greedy chooser that keeps shortcut hit rates near chance across items."""

    hits: Counter[str] = field(default_factory=Counter)
    expected: defaultdict[str, float] = field(default_factory=lambda: defaultdict(float))
    positions: Counter[int] = field(default_factory=Counter)
    position_expected: defaultdict[int, float] = field(default_factory=lambda: defaultdict(float))
    position_weight: float = 0.5

    def cost(self, config: ChoiceConfig) -> float:
        chance = 1 / len(config.texts)
        total = 0.0
        for name, guess in config.shortcuts.items():
            hit = self.hits[name] + (guess == config.correct)
            total += (hit - self.expected[name] - chance) ** 2
        index = list(config.texts).index(config.correct)
        for pos in range(len(config.texts)):
            count = self.positions[pos] + (pos == index)
            total += self.position_weight * (count - self.position_expected[pos] - chance) ** 2
        return total

    def choose(self, configs: Sequence[ChoiceConfig]) -> ChoiceConfig:
        best = min(configs, key=self.cost)
        chance = 1 / len(best.texts)
        for name, guess in best.shortcuts.items():
            self.hits[name] += guess == best.correct
            self.expected[name] += chance
        index = list(best.texts).index(best.correct)
        for pos in range(len(best.texts)):
            self.positions[pos] += pos == index
            self.position_expected[pos] += chance
        return best


def labelled(
    rng: random.Random, correct: str, distractors: Sequence[str]
) -> tuple[dict[str, str], str]:
    """Shuffle texts under letter labels; returns (label to text, correct label)."""
    texts = [correct, *distractors]
    order = list(range(len(texts)))
    rng.shuffle(order)
    mapping = {chr(ord("A") + pos): texts[src] for pos, src in enumerate(order)}
    correct_label = next(label for label, text in mapping.items() if text == correct)
    return mapping, correct_label


def balance_noul[T](  # noqa: PLR0912
    candidates: Sequence[T],
    label: Callable[[T], bool],
    features: Callable[[T], tuple[bool, ...]],
    n: int,
    rng: random.Random,
    group: Callable[[T], str] | None = None,
    spread: float = 1.6,
) -> list[T]:
    """Pick ``n`` (even) candidates, half true, with features independent of the label.

    Candidates are grouped by feature vector; within a group true and false
    items are paired, so every feature predicts the label at exactly 50%.
    With ``group`` (a source unit), no unit takes more than ``spread`` times
    its even share while other units can still fill the quota. If the pairs
    run out, the rest is filled keeping exactly half true.
    """
    n -= n % 2
    cells: dict[tuple[bool, ...], dict[bool, list[T]]] = defaultdict(lambda: {True: [], False: []})
    for cand in candidates:
        cells[features(cand)][label(cand)].append(cand)
    for cell in cells.values():
        for side in (True, False):
            rng.shuffle(cell[side])
    groups = {group(c) for c in candidates} if group is not None else set()
    counts: Counter[str] = Counter()

    def take(items: list[T], cap: float) -> T | None:
        if group is None:
            return items.pop(0) if items else None
        best = None
        for i, item in enumerate(items):
            g = group(item)
            if counts[g] < cap and (best is None or counts[g] < counts[group(items[best])]):
                best = i
        return items.pop(best) if best is not None else None

    chosen: list[T] = []
    keys = sorted(cells)
    cap = max(2.0, spread * n / max(1, len(groups))) if groups else float("inf")
    for limit in (cap, float("inf")):
        progress = True
        while len(chosen) < n and progress:
            progress = False
            rng.shuffle(keys)
            for key in keys:
                cell = cells[key]
                if len(chosen) >= n:
                    break
                yes = take(cell[True], limit)
                if yes is None:
                    continue
                no = take(cell[False], limit)
                if no is None:
                    cell[True].insert(0, yes)
                    continue
                for item in (yes, no):
                    chosen.append(item)
                    if group is not None:
                        counts[group(item)] += 1
                progress = True
    if len(chosen) < n:
        rest = {side: [c for cell in cells.values() for c in cell[side]] for side in (True, False)}
        need = min((n - len(chosen)) // 2, len(rest[True]), len(rest[False]))
        for side in (True, False):
            chosen.extend(rest[side][:need])
    rng.shuffle(chosen)
    return chosen


def above_median(values: Sequence[float]) -> Callable[[float], bool]:
    cut = statistics.median(values) if values else 0.0
    return lambda v: v > cut


def rng_for(seed: int, *parts: object) -> random.Random:
    return random.Random(":".join([str(seed), *map(str, parts)]))
