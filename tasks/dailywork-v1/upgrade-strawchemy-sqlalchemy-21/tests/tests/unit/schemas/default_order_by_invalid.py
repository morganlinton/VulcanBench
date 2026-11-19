from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color, Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, include=["name"])
class FruitType:
    id: strawberry.auto


@strawchemy.type(Color, include=["name"])
class ColorType:
    id: strawberry.auto


@strawberry.type
class WrongModelQuery:
    fruits: list[FruitType] = strawchemy.field(default_order_by=Color.name.asc())
