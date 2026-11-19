from __future__ import annotations

import strawberry

from strawchemy import Strawchemy, ValidationErrorType
from strawchemy.schema.interfaces import ErrorType
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawberry.type
class Plain:
    name: str


@strawberry.type
class CustomError(ErrorType):
    message: str


@strawchemy.type(Color, include=["name"])
class ColorType:
    @strawchemy.field
    def plain(self) -> list[Plain]:
        return []


@strawchemy.create_input(Color, include=["name"])
class ColorCreate: ...


@strawberry.type
class Query:
    colors: list[ColorType] = strawchemy.field()
    color_or_plain: ColorType | Plain = strawchemy.field()
    ordered_colors: list[ColorType | Plain] = strawchemy.field(default_order_by=Color.name)

    @strawchemy.field
    def plain(self) -> list[Plain]:
        return []

    @strawchemy.field
    def plain_by_id(self) -> Plain:
        return Plain(name="")

    @strawchemy.field
    def count(self) -> int:
        return 0


@strawberry.type
class Mutation:
    create_color: ColorType | ValidationErrorType = strawchemy.create(ColorCreate)
    create_color_custom_error: CustomError | ColorType = strawchemy.create(ColorCreate)
    create_colors: list[ColorType | Plain] = strawchemy.create(ColorCreate)

    @strawchemy.create(ColorCreate)
    def create_plain(self) -> Plain:
        return Plain(name="")
