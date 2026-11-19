"""Circular plain forward-reference annotations between two `@strawchemy.type` classes (default scope).

Same circular structure as `tests/unit/schemas/lazy/`, but using plain string forward
annotations (no `strawberry.lazy`). With `from __future__ import annotations` the
return annotation is a bare string `"BNode"` resolved by the registry's forward-ref
machinery at schema build.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import strawberry
from strawberry import auto

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")

if TYPE_CHECKING:
    from tests.unit.schemas.forwardref.b import BNode


@strawchemy.type(Color)
class ANode:
    id: auto
    name: auto

    @strawberry.field
    def b(self) -> BNode:
        raise NotImplementedError
