from __future__ import annotations

import strawberry

from strawchemy import Strawchemy, StrawchemyConfig
from tests.unit.models import Fruit

strawchemy = Strawchemy(StrawchemyConfig("postgresql", pagination="all"))


@strawchemy.type(Fruit, include="all")
class FruitType:
    pass


@strawberry.type
class Query:
    fruits: list[FruitType] = strawchemy.field()
