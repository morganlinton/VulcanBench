from __future__ import annotations

import strawberry

from strawchemy import Strawchemy, StrawchemyConfig
from tests.unit.models import Color

strawchemy = Strawchemy(StrawchemyConfig("postgresql", distinct_on=[]))


@strawchemy.type(Color, include="all")
class ColorType:
    pass


@strawberry.type
class Query:
    colors: list[ColorType] = strawchemy.field()
