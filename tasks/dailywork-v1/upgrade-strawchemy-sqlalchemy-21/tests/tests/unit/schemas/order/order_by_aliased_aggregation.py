# No `from __future__ import annotations` — SQLAlchemy needs concrete annotations.
from uuid import uuid4

import strawberry
from sqlalchemy import ForeignKey
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship

from strawchemy import Strawchemy
from strawchemy.dto.types import Purpose, PurposeConfig
from strawchemy.dto.utils import field as dto_field


class _AliasBase(DeclarativeBase):
    __abstract__ = True

    id: Mapped[str] = mapped_column(primary_key=True, default=lambda: str(uuid4()))


class _AliasedContainer(_AliasBase):
    __tablename__ = "aliased_container_regression"

    items: Mapped[list["_AliasedItem"]] = relationship("_AliasedItem", back_populates="parent")


class _AliasedItem(_AliasBase):
    __tablename__ = "aliased_item_regression"

    # 'score' is aggregatable (float) and carries a READ-purpose alias 'rating'.
    # Before the fix, _order_by_aggregation_fields keyed model_fields by prop.key
    # ("score") but looked up by name.field_definition.name ("rating"), raising KeyError.
    score: Mapped[float] = mapped_column(info=dto_field(configs={Purpose.READ: PurposeConfig(alias="rating")}))
    parent_id: Mapped[str | None] = mapped_column(ForeignKey("aliased_container_regression.id"), nullable=True)
    parent: Mapped[_AliasedContainer | None] = relationship("_AliasedContainer", back_populates="items")


strawchemy = Strawchemy("postgresql")


@strawchemy.type(_AliasedItem, include="all")
class AliasedItemType: ...


@strawchemy.type(_AliasedContainer, include="all")
class AliasedContainerType: ...


@strawchemy.order(_AliasedContainer, include="all")
class AliasedContainerOrderBy: ...


@strawberry.type
class Query:
    containers: list[AliasedContainerType] = strawchemy.field(order_by_input=AliasedContainerOrderBy)
