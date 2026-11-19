import strawberry

from strawchemy import Strawchemy, ValidationErrorType

strawchemy = Strawchemy("postgresql")


@strawberry.type
class Query:
    @strawchemy.field
    def errors(self) -> list[ValidationErrorType]:
        return []
