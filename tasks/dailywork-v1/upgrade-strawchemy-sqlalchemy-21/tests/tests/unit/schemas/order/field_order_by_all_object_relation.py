from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color, Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Color, include={"id", "name"})
class ShadeType:
    pass


@strawchemy.type(Fruit, include={"id", "name"})
class FruitType:
    color: ShadeType


@strawchemy.type(Fruit, include={"id", "name"})
class AliasedFruitType:
    shade: ShadeType = strawchemy.field(model_field="color")


@strawberry.type
class Query:
    fruits: list[FruitType] = strawchemy.field(order_by_input="all")
    aliased_fruits: list[AliasedFruitType] = strawchemy.field(order_by_input="all")
