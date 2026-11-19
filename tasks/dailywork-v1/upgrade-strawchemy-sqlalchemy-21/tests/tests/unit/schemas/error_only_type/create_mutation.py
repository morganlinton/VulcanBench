import strawberry

from strawchemy import Strawchemy, ValidationErrorType
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawchemy.create_input(Color, include=["name"])
class ColorCreate: ...


@strawberry.type
class Mutation:
    errors: ValidationErrorType = strawchemy.create(ColorCreate)
