"""Circular plain forward-reference annotations with global-scope canonicalization.

Same as `tests/unit/schemas/lazy_global/` but using plain string forward annotations
instead of `strawberry.lazy`. `ColorNode` is the `scope="schema"` canonical Color type;
`FruitNode` (include="all") auto-generates a `color` reference that canonicalizes to
`ColorNode` (no duplicate `ColorType`).
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import strawberry
from strawberry import auto

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")

if TYPE_CHECKING:
    from tests.unit.schemas.forwardref_global.b import FruitNode


@strawchemy.type(Color, scope="schema")
class ColorNode:
    id: auto
    name: auto

    @strawberry.field
    def featured_fruit(self) -> FruitNode:
        raise NotImplementedError
