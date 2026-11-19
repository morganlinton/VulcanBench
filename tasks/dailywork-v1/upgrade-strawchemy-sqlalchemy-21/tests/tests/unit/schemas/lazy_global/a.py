"""Circular `strawberry.lazy` references with global-scope canonicalization.

`ColorNode` is the `scope="schema"` canonical type for `Color`. `FruitNode`
(include="all") auto-generates a `color` reference that canonicalizes to `ColorNode`
(no duplicate `ColorType` in the schema), and the two types reference each other via
circular `strawberry.lazy(...)` fields.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Annotated

import strawberry
from strawberry import auto

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")

if TYPE_CHECKING:
    from tests.unit.schemas.lazy_global.b import FruitNode


@strawchemy.type(Color, scope="schema")
class ColorNode:
    id: auto
    name: auto

    @strawberry.field
    def featured_fruit(self) -> Annotated[FruitNode, strawberry.lazy("tests.unit.schemas.lazy_global.b")]:
        raise NotImplementedError
