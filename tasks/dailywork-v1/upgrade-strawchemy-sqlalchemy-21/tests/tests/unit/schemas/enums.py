from __future__ import annotations

import strawberry
from strawberry import auto

from strawchemy import Strawchemy
from tests.unit.models import Vegetable

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Vegetable)
class VegetableType:
    family: auto


@strawberry.type
class Query:
    vegetable: VegetableType = strawchemy.field()
