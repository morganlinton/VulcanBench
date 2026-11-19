"""See `tests/unit/schemas/lazy/a.py` — circular `strawberry.lazy` references, default scope."""

from __future__ import annotations

from typing import TYPE_CHECKING, Annotated

import strawberry
from strawberry import auto

from tests.unit.models import Fruit
from tests.unit.schemas.lazy.a import strawchemy

if TYPE_CHECKING:
    from tests.unit.schemas.lazy.a import ANode


@strawchemy.type(Fruit)
class BNode:
    id: auto
    name: auto

    @strawberry.field
    def a(self) -> Annotated[ANode, strawberry.lazy("tests.unit.schemas.lazy.a")]:
        raise NotImplementedError
