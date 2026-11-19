from __future__ import annotations

import strawberry

from strawchemy import QueryHook, Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, query_hook=QueryHook(load=[(Fruit.name, (Fruit.id,))]))
class FruitType: ...


@strawberry.type
class Query:
    fruits: list[FruitType] = strawchemy.field()
