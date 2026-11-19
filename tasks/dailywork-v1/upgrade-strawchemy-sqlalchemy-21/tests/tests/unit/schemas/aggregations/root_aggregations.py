from __future__ import annotations

import strawberry

from strawchemy import Strawchemy
from tests.unit.models import SQLDataTypes

strawchemy = Strawchemy("postgresql")


@strawchemy.aggregate(SQLDataTypes, include="all")
class SQLDataTypesAggregationType: ...


@strawberry.type
class Query:
    sql_data_types: list[SQLDataTypesAggregationType] = strawchemy.field(root_aggregations=True)
