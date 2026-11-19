from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawberry.type
class Query:
    colors: list[ColorType] = strawchemy.field()


@strawchemy.type(Color, include=["name"])
class ColorType: ...


RegistryColorType = ColorType


@strawberry.type(name="PlainColorType")
class ColorType:
    name: str
