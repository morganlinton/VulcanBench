"""See `tests/unit/schemas/forwardref_global/a.py` — circular plain forward references, global scope."""

from __future__ import annotations

from typing import TYPE_CHECKING

import strawberry

from tests.unit.models import Fruit
from tests.unit.schemas.forwardref_global.a import strawchemy

if TYPE_CHECKING:
    from tests.unit.schemas.forwardref_global.a import ColorNode


@strawchemy.type(Fruit, include="all")
class FruitNode:
    @strawberry.field
    def primary_color(self) -> ColorNode:
        raise NotImplementedError
