from __future__ import annotations

import strawberry
from strawberry import auto

from strawchemy import Strawchemy
from tests.unit.models import Color, Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Color, include="all", override=True)
class ColorType:
    fruits: auto
    name: int


@strawchemy.type(Fruit, include="all", override=True)
class FruitType:
    name: int


@strawberry.type
class Query:
    fruit: FruitType = strawchemy.field()
