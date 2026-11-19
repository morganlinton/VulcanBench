from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Color)
class ColorType:
    name: strawberry.auto


@strawberry.type
class Query:
    color_aggregations: list[ColorType] = strawchemy.field(root_aggregations=True)
