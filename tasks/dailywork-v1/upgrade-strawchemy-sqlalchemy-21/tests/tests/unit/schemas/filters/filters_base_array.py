from __future__ import annotations

import strawberry
from sqlalchemy import ARRAY, Text
from sqlalchemy.orm import Mapped, mapped_column

from strawchemy import Strawchemy
from tests.unit.models import UUIDBase


class BaseArray(UUIDBase):
    __tablename__ = "base_array"

    array_str_col: Mapped[list[str]] = mapped_column(ARRAY(Text), default=list)


strawchemy = Strawchemy("postgresql")


@strawchemy.type(BaseArray, include="all")
class BaseArrayType: ...


@strawchemy.filter(BaseArray, include="all")
class BaseArrayFilter: ...


@strawberry.type
class Query:
    sql_data_types: list[BaseArrayType] = strawchemy.field(filter_input=BaseArrayFilter)
