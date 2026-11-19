from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, include="all", paginate="all", order="all")
class FruitType:
    name: int


@strawchemy.order(Fruit, include="all", override=True)
class FruitOrderBy:
    override: bool = True


@strawberry.type
class Query:
    fruits: list[FruitType] = strawchemy.field(order_by_input=FruitOrderBy)
