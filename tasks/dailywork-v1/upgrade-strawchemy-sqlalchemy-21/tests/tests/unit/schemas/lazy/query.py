from __future__ import annotations

import strawberry

from tests.unit.schemas.lazy.a import ANode, strawchemy
from tests.unit.schemas.lazy.b import BNode


@strawberry.type
class Query:
    a: ANode = strawchemy.field()
    b: BNode = strawchemy.field()
