"""See `tests/unit/schemas/forwardref/a.py` — circular plain forward references, default scope."""

from __future__ import annotations

from typing import TYPE_CHECKING

import strawberry
from strawberry import auto

from tests.unit.models import Fruit
from tests.unit.schemas.forwardref.a import strawchemy

if TYPE_CHECKING:
    from tests.unit.schemas.forwardref.a import ANode


@strawchemy.type(Fruit)
class BNode:
    id: auto
    name: auto

    @strawberry.field
    def a(self) -> ANode:
        raise NotImplementedError
