from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Color, include="all")
class ColorType:
    pass


@strawberry.type
class Query:
    colors: list[ColorType] = strawchemy.field(distinct_on="all")
