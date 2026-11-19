from __future__ import annotations

import strawberry

from strawchemy import Strawchemy

strawchemy = Strawchemy("postgresql")


@strawberry.type
class ColorType:
    name: str


@strawberry.type
class Query:
    color_aggregations: list[ColorType] = strawchemy.field(root_aggregations=True)
