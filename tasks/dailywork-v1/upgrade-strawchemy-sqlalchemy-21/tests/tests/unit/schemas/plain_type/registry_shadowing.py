from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawchemy.type(Color, include=["name"])
class ColorType: ...


RegistryColorType = ColorType


@strawberry.type(name="PlainColorType")
class ColorType:
    name: str


@strawberry.type
class Query:
    color: ColorType = strawchemy.field()
