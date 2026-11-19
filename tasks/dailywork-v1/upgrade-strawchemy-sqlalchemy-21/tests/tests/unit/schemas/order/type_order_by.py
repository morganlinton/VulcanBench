from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import SQLDataTypes

strawchemy = Strawchemy("postgresql")


@strawchemy.order(SQLDataTypes, include="all")
class SQLDataTypesOrderBy: ...


@strawchemy.type(SQLDataTypes, include="all", order=SQLDataTypesOrderBy)
class SQLDataTypesType: ...


@strawberry.type
class Query:
    sql_data_types: list[SQLDataTypesType] = strawchemy.field()
