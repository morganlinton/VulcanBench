from __future__ import annotations

import strawberry

from strawchemy import Strawchemy, StrawchemyConfig
from tests.unit.models import Fruit

strawchemy = Strawchemy(StrawchemyConfig("postgresql", order_by="all"))


@strawchemy.type(Fruit, include="all")
class FruitType:
    pass


@strawberry.type
class Query:
    fruits: list[FruitType] = strawchemy.field(order_by_input=["name"])
