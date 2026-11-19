from __future__ import annotations

import strawberry

from tests.unit.schemas.lazy_global.a import ColorNode, strawchemy
from tests.unit.schemas.lazy_global.b import FruitNode


@strawberry.type
class Query:
    fruit: FruitNode = strawchemy.field()
    color: ColorNode = strawchemy.field()
