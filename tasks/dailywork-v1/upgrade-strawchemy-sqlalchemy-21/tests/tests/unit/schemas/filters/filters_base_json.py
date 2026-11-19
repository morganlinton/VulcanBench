from __future__ import annotations

from typing import Any

import strawberry
from sqlalchemy import JSON
from sqlalchemy.orm import Mapped, mapped_column

from strawchemy import Strawchemy
from tests.unit.models import UUIDBase


class BaseJSON(UUIDBase):
    __tablename__ = "base_json"

    dict_col: Mapped[dict[str, Any]] = mapped_column(JSON, default=dict)


strawchemy = Strawchemy("postgresql")


@strawchemy.type(BaseJSON, include="all")
class BaseJSONType: ...


@strawchemy.filter(BaseJSON, include="all")
class BaseJSONFilter: ...


@strawberry.type
class Query:
    sql_data_types: list[BaseJSONType] = strawchemy.field(filter_input=BaseJSONFilter)
