"""Circular `strawberry.lazy` references between two `@strawchemy.type` classes (default scope).

`ANode` (over `Color`) lazily references `BNode` in module `b`, which lazily
references `ANode` back. Both lazy fields resolve to their target types when the
schema is built.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Annotated

import strawberry
from strawberry import auto

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")

if TYPE_CHECKING:
    from tests.unit.schemas.lazy.b import BNode


@strawchemy.type(Color)
class ANode:
    id: auto
    name: auto

    @strawberry.field
    def b(self) -> Annotated[BNode, strawberry.lazy("tests.unit.schemas.lazy.b")]:
        raise NotImplementedError
