"""See `tests/unit/schemas/lazy_global/a.py` — circular `strawberry.lazy` references, global scope."""

from __future__ import annotations

from typing import TYPE_CHECKING, Annotated

import strawberry

from tests.unit.models import Fruit
from tests.unit.schemas.lazy_global.a import strawchemy

if TYPE_CHECKING:
    from tests.unit.schemas.lazy_global.a import ColorNode


@strawchemy.type(Fruit, include="all")
class FruitNode:
    @strawberry.field
    def primary_color(self) -> Annotated[ColorNode, strawberry.lazy("tests.unit.schemas.lazy_global.a")]:
        raise NotImplementedError
