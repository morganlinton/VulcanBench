import strawberry

from strawchemy import Strawchemy, ValidationErrorType

strawchemy = Strawchemy("postgresql")


@strawberry.type
class Query:
    errors: list[ValidationErrorType] = strawchemy.field()
