from __future__ import annotations

import strawberry
from strawberry import auto

from strawchemy import Strawchemy
from tests.unit.models import Fruit

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Fruit, exclude=["name"])
class FruitType:
    name: auto


@strawberry.type
class Query:
    fruit: FruitType
