from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawberry.type
class Plain:
    name: str


@strawberry.type
class Query:
    @strawchemy.field(default_order_by=Color.name)
    def plain(self) -> list[Plain]:
        return []
