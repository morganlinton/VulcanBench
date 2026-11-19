import strawberry

from strawchemy import Strawchemy, ValidationErrorType
from tests.unit.models import Color

strawchemy = Strawchemy("postgresql")


@strawberry.type
class Query:
    @strawchemy.field(default_order_by=Color.name)
    def errors(self) -> list[ValidationErrorType]:
        return []
