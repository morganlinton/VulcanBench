from __future__ import annotations

import warnings
from enum import Enum
from typing import ForwardRef, Optional
from uuid import UUID

import pytest
import sqlalchemy as sa
import strawberry
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column
from strawberry.scalars import JSON
from strawberry.types import get_object_definition
from typing_extensions import Self

from strawchemy import Strawchemy, StrawchemyConfig
from strawchemy.schema.factories.base import GraphQLFactory
from strawchemy.schema.scalars import DateTime
from tests.utils import as_dto


class _Unmappable:
    """A plain class with no GraphQL mapping."""


class _Color(Enum):
    RED = 1
    GREEN = 2


@strawberry.type
class _SomeStrawberryType:
    x: int


class _ModelBase(DeclarativeBase):
    pass


class _WithUnmappable(_ModelBase):
    __tablename__ = "with_unmappable"

    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column()
    data: Mapped[_Unmappable] = mapped_column(sa.JSON)


def _field_names(type_: type) -> set[str]:
    return {field.python_name for field in get_object_definition(type_, strict=True).fields}


@pytest.fixture
def factory() -> GraphQLFactory:
    # `type_factory` is an ObjectTypeFactory, a concrete GraphQLFactory subclass.
    return Strawchemy(StrawchemyConfig("postgresql")).type_factory


@pytest.mark.parametrize(
    "annotation",
    [
        int,
        str,
        float,
        bool,
        UUID,
        JSON,
        DateTime,
        _Color,
        _SomeStrawberryType,
        Optional[int],
        int | None,
        Self,
        list[str],
        list[Optional[UUID]],
        ForwardRef("Whatever"),
        "Whatever",
        ForwardRef("_SomeStrawberryType"),
        "_SomeStrawberryType",
    ],
)
def test_is_graphql_mappable_true(factory: GraphQLFactory, annotation: object) -> None:
    """Scalars, enums, Strawberry types, containers, and references map to GraphQL."""
    assert factory.is_graphql_mappable(annotation) is True


@pytest.mark.parametrize(
    "annotation",
    [_Unmappable, Optional[_Unmappable], list[_Unmappable]],
)
def test_is_graphql_mappable_false(factory: GraphQLFactory, annotation: object) -> None:
    """A plain class (alone, optional, or in a container) has no GraphQL mapping."""
    assert factory.is_graphql_mappable(annotation) is False


def test_forward_ref_assumed_true_when_unresolvable(factory: GraphQLFactory) -> None:
    """A forward ref that cannot be resolved against the namespace is assumed mappable."""
    # Name absent from the (default) namespace -> cannot resolve -> assume mappable.
    assert factory.is_graphql_mappable(ForwardRef("_DefinitelyAbsentName")) is True


@pytest.mark.parametrize(
    ("ref", "expected"),
    [(ForwardRef("_Color"), True), (ForwardRef("_Unmappable"), False), ("_Unmappable", False), ("list[_Color]", True)],
)
def test_forward_ref_resolved_against_namespace(factory: GraphQLFactory, ref: object, expected: bool) -> None:
    """A forward ref / string annotation is resolved against the namespace, then re-checked."""
    namespace = {"_Color": _Color, "_Unmappable": _Unmappable, "list": list}
    assert factory.is_graphql_mappable(ref, namespace) is expected


@pytest.mark.parametrize(("strict", "data_present"), [(False, False), (True, True)])
def test_strict_controls_unmappable_field_presence(strict: bool, data_present: bool) -> None:
    """strict=False drops the unmappable column from the type; strict=True keeps it."""
    strawchemy = Strawchemy(StrawchemyConfig("postgresql", strict=strict))

    with warnings.catch_warnings():
        warnings.simplefilter("ignore")

        @strawchemy.type(_WithUnmappable)
        class _Type:
            id: strawberry.auto
            name: strawberry.auto
            data: strawberry.auto

    names = _field_names(_Type)
    assert "name" in names
    assert ("data" in names) is data_present


def test_strict_false_warns_per_skipped_field() -> None:
    """Skipping an unmappable field under strict=False emits a warning."""
    strawchemy = Strawchemy(StrawchemyConfig("postgresql", strict=False))

    with pytest.warns(UserWarning, match="no GraphQL mapping"):

        @strawchemy.type(_WithUnmappable)
        class _Type:
            id: strawberry.auto
            name: strawberry.auto
            data: strawberry.auto


def test_strict_false_keeps_explicitly_overridden_unmappable_field() -> None:
    """An explicit type override is trusted and never skipped, even under strict=False."""
    strawchemy = Strawchemy(StrawchemyConfig("postgresql", strict=False))

    # `type_map` on the decorator becomes `dto_config.type_overrides`, which the override trusts.
    @strawchemy.type(_WithUnmappable, type_map={_Unmappable: JSON})
    class _Type:
        id: strawberry.auto
        name: strawberry.auto
        data: strawberry.auto

    assert "data" in _field_names(_Type)


def test_strict_false_skips_unmappable_in_order_by_input() -> None:
    """A skipped column does not leak into the generated order-by input (shared chokepoint)."""
    strawchemy = Strawchemy(StrawchemyConfig("postgresql", strict=False))

    with pytest.warns(UserWarning, match="no GraphQL mapping"):

        @strawchemy.type(_WithUnmappable, order="all")
        class OrderableType:
            id: strawberry.auto
            name: strawberry.auto
            data: strawberry.auto

    order_by_input = as_dto(OrderableType).__strawchemy_definition__.order_by
    assert order_by_input is not None
    names = _field_names(order_by_input)
    assert "name" in names
    assert "data" not in names
