from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, include="all")
class FruitType:
    pass


@strawchemy.order(Fruit, include="all")
class FruitOrderBy:
    pass


@strawberry.type
class Query:
    fruit: list[FruitType] = strawchemy.field(order_by_input=FruitOrderBy)
