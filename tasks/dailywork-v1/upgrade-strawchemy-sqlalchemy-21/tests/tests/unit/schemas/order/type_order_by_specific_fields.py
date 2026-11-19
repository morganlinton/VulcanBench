from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import SQLDataTypes

strawchemy = Strawchemy("postgresql")


@strawchemy.type(SQLDataTypes, include="all", order=["str_col", "int_col"])
class SQLDataTypesType: ...


@strawberry.type
class Query:
    sql_data_types: list[SQLDataTypesType] = strawchemy.field()
