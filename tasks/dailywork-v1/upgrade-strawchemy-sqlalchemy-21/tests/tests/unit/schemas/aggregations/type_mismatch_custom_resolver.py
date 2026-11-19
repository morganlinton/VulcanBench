from __future__ import annotations

import strawberry

from strawchemy import Strawchemy

strawchemy = Strawchemy("postgresql")


@strawberry.type
class ColorType:
    name: str


@strawberry.type
class Query:
    @strawchemy.field(root_aggregations=True)
    def color_aggregations(self) -> list[ColorType]:
        return []
