from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Color, include=["name"])
class ColorType: ...


@strawberry.type
class Plain:
    name: str


@strawberry.type
class Query:
    plain: Plain | ColorType = strawchemy.field()
