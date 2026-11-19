from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, exclude=["name"])
class FruitType:
    sweetness: str


@strawberry.type
class Query:
    fruit: FruitType
