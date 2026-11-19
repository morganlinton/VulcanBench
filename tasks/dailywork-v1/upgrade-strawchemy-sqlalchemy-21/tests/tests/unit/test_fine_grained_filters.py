from __future__ import annotations

from datetime import datetime, timezone
from datetime import datetime as _dt
from typing import TYPE_CHECKING, Any, cast, get_args

import pytest
import strawberry
from sqlalchemy import JSON, ForeignKey, Select
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship
from strawberry import UNSET
from strawberry.types import get_object_definition

from strawchemy import Strawchemy, StrawchemyConfig
from strawchemy.dto.strawberry import CustomFilter, CustomFilterFieldDefinition, Filter, QueryNode
from strawchemy.dto.types import DTOConfig, Purpose
from strawchemy.exceptions import StrawchemyFieldError
from strawchemy.schema.filters.fields import FilterFieldMarker
from strawchemy.schema.filters.inputs import (
    ArrayComparison,
    DateComparison,
    DateTimeComparison,
    EqualityComparison,
    OrderComparison,
    TextComparison,
    TimeComparison,
    TimeDeltaComparison,
)
from strawchemy.typing import (
    ArrayOperator,
    ComparisonOperator,
    DateOperator,
    DateTimeOperator,
    EqualityOperator,
    OrderOperator,
    TextOperator,
    TimeDeltaOperator,
    TimeOperator,
)

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy.dto.types import FieldSpec

# Module-level convenience: `filter_field` is a Strawchemy method, but markers are
# mapper-agnostic, so a shared alias keeps the rest of the tests terse. Tests that
# specifically exercise the method call `Strawchemy("sqlite").filter_field(...)` directly.
_module_strawchemy = Strawchemy("sqlite")
filter_field = _module_strawchemy.filter_field


def _aggregate_fields(dto: type[Any]) -> set[str]:
    return {f.graphql_name or f.name for f in get_object_definition(dto, strict=True).fields}


def _predicate_fields(dto: type[Any], function_name: str) -> set[str]:
    function_field = next(
        f for f in get_object_definition(dto, strict=True).fields if (f.graphql_name or f.name) == function_name
    )
    function_type = getattr(function_field.type, "of_type", function_field.type)
    predicate = next(
        f for f in get_object_definition(function_type, strict=True).fields if (f.graphql_name or f.name) == "predicate"
    )
    predicate_type = getattr(predicate.type, "of_type", predicate.type)
    return {f.graphql_name or f.name for f in get_object_definition(predicate_type, strict=True).fields}


def _argument_enum(dto: type[Any], function_name: str) -> type[Any]:
    function_field = next(
        f for f in get_object_definition(dto, strict=True).fields if (f.graphql_name or f.name) == function_name
    )
    function_type = getattr(function_field.type, "of_type", function_field.type)
    arguments = next(
        f for f in get_object_definition(function_type, strict=True).fields if (f.graphql_name or f.name) == "arguments"
    )
    argument_type = getattr(arguments.type, "of_type", arguments.type)
    list_type = getattr(argument_type, "of_type", argument_type)
    # strawberry represents an enum field type as a StrawberryEnumDefinition; unwrap to the actual Enum class.
    return cast("type[Any]", getattr(list_type, "wrapped_cls", list_type))


def _published_after(statement: Select[Any], value: Any, **_ctx: Any) -> Select[Any]:
    """Custom-apply callable for the tests declaring a virtual filter field."""
    return statement.where(_Ticket.published_at >= value)


class _Base(DeclarativeBase): ...


class _Ticket(_Base):
    __tablename__ = "fgf_ticket"
    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column()
    published_at: Mapped[datetime] = mapped_column()
    project_id: Mapped[int | None] = mapped_column(ForeignKey("fgf_project.id"), nullable=True, default=None)
    project: Mapped[_Project | None] = relationship("_Project", back_populates="tickets")


class _Project(_Base):
    __tablename__ = "fgf_project"
    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column()
    tickets: Mapped[list[_Ticket]] = relationship("_Ticket", back_populates="project")


class _TicketCountCol(_Base):
    """A model with a column literally named ``count``, colliding with the aggregation function."""

    __tablename__ = "fgf_ticket_count_col"
    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column()
    count: Mapped[int] = mapped_column()
    published_at: Mapped[datetime] = mapped_column()


class _Unmappable:
    """A plain class with no GraphQL mapping."""


class _TicketUnmappable(_Base):
    __tablename__ = "fgf_ticket_unmappable"
    id: Mapped[int] = mapped_column(primary_key=True)
    data: Mapped[_Unmappable] = mapped_column(JSON)


# Declared aggregate filter classes referenced as annotations by another declared filter class
# below must live at module scope: annotation resolution evaluates against the *defining
# module's* globals, not an enclosing function's locals.
@_module_strawchemy.aggregate_filter(_Ticket, functions=["count"], name="TicketCountAggregate")
class _TicketAggregateFilterWired:
    """Declared aggregate filter wired onto ``ProjectFilter.tickets_aggregate`` by annotation."""

    count: int = filter_field(ops=["gt"])


@_module_strawchemy.aggregate_filter(_Project, functions=["count"], name="ProjectCountAggregate")
class _ProjectAggregateFilterMismatch:
    """Built for ``_Project``; wiring it onto a ``_Ticket`` relation must raise."""


@_module_strawchemy.aggregate_filter(
    _Ticket, include="all", functions=["count", "max_datetime"], name="TicketAggregateFilter"
)
class _TicketAggregateFilterSchema:
    """Declared aggregate filter exercised by the schema snapshot test."""

    count: int = filter_field(ops=["gt", "eq"], arguments=["id"])


# Rejecting an out-of-scope selection is GraphQL validation, which runs before any resolver, so the
# rejection cases below need a schema but no database. They get their own mapper: sharing
# `_module_strawchemy` lets types cached by other tests decide what these filters generate.
_rejection_strawchemy = Strawchemy("sqlite")


@_rejection_strawchemy.aggregate_filter(_Ticket, include=["id"], functions=["count"], name="TicketScopedAggregate")
class _TicketScopedAggregate:
    """No marker on ``count``: the decorator's own ``include`` is what scopes its arguments."""


@_rejection_strawchemy.type(_Project, include=["name"])
class _RejectionProjectType: ...


@_rejection_strawchemy.type(_Ticket, include=["name"])
class _RejectionTicketType: ...


@_rejection_strawchemy.filter(_Project, include=["name", "tickets"], name="ProjectFilter")
class _RejectionProjectFilter:
    tickets_aggregate: _TicketAggregateFilterSchema  # ty: ignore[invalid-type-form]


@_rejection_strawchemy.filter(_Project, include=["name", "tickets"], name="ProjectScopedFilter")
class _RejectionProjectScopedFilter:
    tickets_aggregate: _TicketScopedAggregate  # ty: ignore[invalid-type-form]


# Declared on a model with no list relation: a declared column marker on `_Project` leaves its
# generated relation-aggregate enums empty, which makes the schema unbuildable.
@_rejection_strawchemy.filter(_Ticket, include=["name"], name="TicketColumnFilter")
class _RejectionTicketColumnFilter:
    name: str = filter_field(ops=["eq"])


@strawberry.type
class _RejectionQuery:
    projects: list[_RejectionProjectType] = _rejection_strawchemy.field(filter_input=_RejectionProjectFilter)
    projects_scoped: list[_RejectionProjectType] = _rejection_strawchemy.field(
        filter_input=_RejectionProjectScopedFilter
    )
    tickets: list[_RejectionTicketType] = _rejection_strawchemy.field(filter_input=_RejectionTicketColumnFilter)


_REJECTION_SCHEMA = strawberry.Schema(query=_RejectionQuery)


def test_filter_field_bare_returns_marker() -> None:
    marker = Strawchemy("sqlite").filter_field()
    assert isinstance(marker, FilterFieldMarker)
    assert marker.ops is None
    assert marker.apply is None


def test_filter_field_with_ops_normalizes_to_tuple() -> None:
    marker = Strawchemy("sqlite").filter_field(ops=["eq", "in", "like"])
    assert marker.ops == ("eq", "in", "like")
    assert marker.apply is None


def test_filter_field_with_apply_captures_callable_and_join() -> None:
    def _apply(statement: Any, *_args: Any, **_kwargs: Any) -> Any:
        return statement

    marker = Strawchemy("sqlite").filter_field(apply=_apply, join="in")
    assert marker.apply is _apply
    assert marker.join == "in"
    assert marker.ops is None


def test_filter_field_rejects_ops_and_apply_together() -> None:
    with pytest.raises(StrawchemyFieldError, match="mutually exclusive"):
        Strawchemy("sqlite").filter_field(ops=["eq"], apply=lambda s, _v, **_k: s)


def test_filter_field_rejects_unknown_join() -> None:
    with pytest.raises(StrawchemyFieldError, match="join"):
        Strawchemy("sqlite").filter_field(apply=lambda s, _v, **_k: s, join="bogus")  # ty: ignore[invalid-argument-type]


def test_restricted_exposes_only_selected_ops() -> None:
    cls = TextComparison.restricted(str, ("eq", "in", "like"))
    exposed = {f.graphql_name or f.name for f in get_object_definition(cls, strict=True).fields}
    assert exposed == {"eq", "in", "like"}


def test_restricted_keeps_unselected_attrs_unset_at_runtime() -> None:
    cls = TextComparison.restricted(str, ("eq",))
    instance = cls()
    assert instance.gt is UNSET  # unselected op still present (UNSET) so query-time filters work
    assert instance.like is UNSET


def test_restricted_carries_base_filter() -> None:
    cls = TextComparison.restricted(str, ("eq",))
    assert cls.__strawchemy_comparison__.filter is TextComparison.__strawchemy_comparison__.filter


def test_restricted_invalid_op_raises() -> None:
    with pytest.raises(StrawchemyFieldError, match="frobnicate"):
        TextComparison.restricted(str, ("eq", "frobnicate"))  # ty: ignore[invalid-argument-type]


def test_restricted_is_deduped() -> None:
    a = TextComparison.restricted(str, ("eq", "like"))
    b = TextComparison.restricted(str, ("like", "eq"))  # order-independent
    assert a is b


def test_restricted_over_int_carries_int() -> None:
    cls = OrderComparison.restricted(int, ("eq", "gt"))
    eq_field = next(f for f in get_object_definition(cls, strict=True).fields if f.name == "eq")
    eq_type = eq_field.type
    # strawberry wraps `int | None` as a StrawberryOptional whose `of_type` is the scalar
    eq_type = getattr(eq_type, "of_type", eq_type)
    assert int in getattr(eq_type, "__args__", (eq_type,))


def test_custom_filter_is_filter_member() -> None:
    cf = CustomFilter(apply=lambda s, _v, **_k: s, value=5, join="exists", field_node=QueryNode.root_node(_Ticket))
    f = Filter(and_=[cf])
    assert f.and_ == [cf]


def test_custom_filter_field_definition_carries_marker_data() -> None:
    def _apply(statement: Any, _value: Any, **_ctx: Any) -> Any:
        return statement

    field_def = CustomFilterFieldDefinition(
        dto_config=DTOConfig(Purpose.READ),
        model=_Ticket,
        model_field_name="published_after",
        type_hint=datetime,
        apply=_apply,
        join="exists",
    )
    assert field_def.apply is _apply
    assert field_def.join == "exists"


def test_parse_declared_fields_collects_restricted_and_custom() -> None:
    sc = Strawchemy("sqlite")

    def _apply(statement: Any, _value: Any, **_ctx: Any) -> Any:
        return statement

    class TicketFilter:
        name: str = sc.filter_field(ops=["eq", "in", "like"])
        published_after: _dt = sc.filter_field(apply=_apply)

    declared = sc.filter_factory.parse_declared_filter_fields(TicketFilter)
    assert set(declared) == {"name", "published_after"}

    name_marker, name_annotation = declared["name"]
    assert name_marker.ops == ("eq", "in", "like")
    assert name_annotation is str  # data type from the annotation

    custom_marker, custom_annotation = declared["published_after"]
    assert custom_marker.apply is _apply
    assert custom_annotation is _dt


def test_parse_declared_fields_requires_annotation() -> None:
    sc = Strawchemy("sqlite")

    class Bad:
        # restricted field with no annotation -> no data type to build a comparison
        name = sc.filter_field(ops=["eq"])

    with pytest.raises(StrawchemyFieldError, match="annotation"):
        sc.filter_factory.parse_declared_filter_fields(Bad)


def test_restricted_field_overrides_auto_comparison() -> None:
    strawchemy = Strawchemy("sqlite")

    @strawchemy.filter(_Ticket, include=["name"])
    class TicketFilter:
        # The annotation is the comparison type (documentary); the data type comes from the column.
        name: TextComparison = filter_field(ops=["eq", "like"])

    definition = get_object_definition(TicketFilter, strict=True)
    name_field = next(f for f in definition.fields if (f.graphql_name or f.name) == "name")
    comparison = name_field.type
    # strawberry wraps `Comparison | None` as a StrawberryOptional whose `of_type` is the comparison
    comparison_origin = getattr(comparison, "of_type", comparison)
    inner = get_object_definition(comparison_origin, strict=True)
    exposed = {f.graphql_name or f.name for f in inner.fields}
    assert exposed == {"eq", "like"}


def test_filter_field_forwards_strawberry_field_args() -> None:
    """filter_field()'s strawberry.field kwargs reach the generated GraphQL input field."""
    strawchemy = Strawchemy("sqlite")

    @strawchemy.filter(_Ticket, include=["name"])
    class TicketFilter:
        name: TextComparison = filter_field(
            ops=["eq"],
            name="renamedName",
            description="Filter by ticket name",
            deprecation_reason="use something else",
            metadata={"k": "v"},
        )

    definition = get_object_definition(TicketFilter, strict=True)
    field = next(f for f in definition.fields if (f.graphql_name or f.name) == "renamedName")
    assert field.description == "Filter by ticket name"
    assert field.deprecation_reason == "use something else"
    assert field.metadata == {"k": "v"}


def test_restricted_field_comparison_annotation_mismatch_raises() -> None:
    strawchemy = Strawchemy("sqlite")

    with pytest.raises(StrawchemyFieldError, match="OrderComparison"):

        @strawchemy.filter(_Ticket, include=["name"])
        class TicketFilter:
            # `name` is a str column -> TextComparison; OrderComparison is the wrong shape.
            name: OrderComparison = filter_field(ops=["eq"])


def test_custom_apply_field_injected_as_scalar() -> None:
    strawchemy = Strawchemy("sqlite")

    @strawchemy.filter(_Ticket, include=["name"])
    class TicketFilter:
        published_after: datetime = filter_field(apply=_published_after)

    definition = get_object_definition(TicketFilter, strict=True)
    # camelCase is applied at schema build time; the object definition keeps the python name
    field = next((f for f in definition.fields if (f.graphql_name or f.name) == "published_after"), None)
    assert field is not None


def test_filters_tree_wraps_custom_field() -> None:
    strawchemy = Strawchemy("sqlite")

    @strawchemy.filter(_Ticket, include=["name"])
    class TicketFilter:
        published_after: datetime = filter_field(apply=_published_after)

    instance: Any = TicketFilter()
    instance.published_after = datetime(2024, 1, 1, tzinfo=timezone.utc)
    _node, query = instance.filters_tree()

    assert any(isinstance(item, CustomFilter) for item in query.and_)
    custom = next(item for item in query.and_ if isinstance(item, CustomFilter))
    assert custom.apply is _published_after
    assert custom.value == datetime(2024, 1, 1, tzinfo=timezone.utc)


def test_public_surface_end_to_end() -> None:
    sc = Strawchemy("sqlite")
    assert callable(sc.filter_field)

    @sc.filter(_Ticket, include=["name"])
    class TicketFilter:
        name: str = sc.filter_field(ops=["eq"])
        published_after: datetime = sc.filter_field(apply=_published_after)

    definition = get_object_definition(TicketFilter, strict=True)
    exposed = {f.graphql_name or f.name for f in definition.fields}
    assert "name" in exposed
    assert "published_after" in exposed


def test_restricted_field_on_unknown_column_raises() -> None:
    from strawchemy.exceptions import StrawchemyFieldError

    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="nonexistent"):

        @sc.filter(_Ticket, include=["name"])
        class TicketFilter:
            nonexistent: str = filter_field(ops=["eq"])


def test_invalid_operator_raises_at_definition() -> None:
    from strawchemy.exceptions import StrawchemyFieldError

    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="frobnicate"):

        @sc.filter(_Ticket, include=["name"])
        class TicketFilter:
            name: str = filter_field(ops=["frobnicate"])  # ty: ignore[invalid-argument-type]


@pytest.mark.snapshot
def test_fine_grained_schema(graphql_snapshot: SnapshotAssertion) -> None:
    sc = Strawchemy("sqlite")

    @sc.type(_Ticket, include=["name", "published_at"])
    class TicketType: ...

    @sc.filter(_Ticket, include=["name", "published_at"])
    class TicketFilter:
        name: TextComparison = filter_field(ops=["eq", "in", "like"])
        published_after: datetime = filter_field(apply=_published_after)

    @strawberry.type
    class Query:
        tickets: list[TicketType] = sc.field(filter_input=TicketFilter)

    schema = strawberry.Schema(query=Query)
    assert str(schema) == graphql_snapshot


def test_filter_functions_carry_comparison_data_type() -> None:
    """Every generated aggregation filter function exposes the scalar its predicate compares."""
    sc = Strawchemy("sqlite")
    # `include="all"` is required: a bare DTOConfig excludes every field, leaving no
    # min/max/sum candidates for the aggregation-function factories to pick up.
    dto_config = DTOConfig(Purpose.READ, include="all")
    functions = sc.aggregate_filter_factory.aggregation_builder.filter_functions(_Ticket, dto_config)

    by_field_name = {function.field_name: function for function in functions}
    assert by_field_name["count"].comparison_data_type is int
    assert by_field_name["min_datetime"].comparison_data_type is datetime
    assert by_field_name["min_string"].comparison_data_type is str
    # the data type must be usable to build a restricted comparison
    for function in functions:
        assert isinstance(function.comparison_data_type, type)


def test_filter_field_with_arguments_normalizes_to_tuple() -> None:
    """arguments= is captured on the marker as an order-preserving tuple."""
    marker = Strawchemy("sqlite").filter_field(arguments=["sweetness", "water_percent"])
    assert marker.arguments == ("sweetness", "water_percent")
    assert marker.ops is None
    assert marker.apply is None


def test_filter_field_rejects_arguments_and_apply_together() -> None:
    """arguments= targets an aggregation function, apply= a virtual column filter."""
    with pytest.raises(StrawchemyFieldError, match="mutually exclusive"):
        Strawchemy("sqlite").filter_field(arguments=["name"], apply=lambda s, _v, **_k: s)


def test_arguments_marker_on_column_filter_raises() -> None:
    """arguments= is aggregation-only and is rejected by the column filter factory."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="arguments"):

        @sc.filter(_Ticket, include=["name"])
        class TicketFilter:
            name: str = sc.filter_field(arguments=["name"])


def test_functions_selects_exposed_aggregations() -> None:
    """functions= restricts the aggregate filter input to the named function fields.

    `include="all"` is required: a bare DTOConfig excludes every field, so `published_at`
    would not be a candidate for the `max_datetime` aggregation function.
    """
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, include="all", functions=["count", "max_datetime"])
    class TicketAggregateFilter: ...

    assert _aggregate_fields(TicketAggregateFilter) == {"count", "max_datetime"}


def test_functions_defaults_to_every_available_function() -> None:
    """Without functions=, the generated input keeps today's full set."""
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, include="all")
    class TicketAggregateFilter: ...

    exposed = _aggregate_fields(TicketAggregateFilter)
    assert "count" in exposed
    assert "max_datetime" in exposed


def test_declared_function_outside_functions_is_force_included() -> None:
    """A declared marker force-includes its function, like a bare filter_field() on a column."""
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, include="all", functions=["count"])
    class TicketAggregateFilter:
        max_datetime: datetime = sc.filter_field()

    assert _aggregate_fields(TicketAggregateFilter) == {"count", "max_datetime"}


def test_unknown_function_name_raises() -> None:
    """functions= is validated against what the model actually exposes."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="frobnicate"):

        @sc.aggregate_filter(_Ticket, functions=["frobnicate"])  # ty: ignore[invalid-argument-type]
        class TicketAggregateFilter: ...


def test_unknown_declared_function_raises() -> None:
    """A declared attribute must name a generated aggregation function field."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="frobnicate"):

        @sc.aggregate_filter(_Ticket)
        class TicketAggregateFilter:
            frobnicate: int = sc.filter_field(ops=["gt"])


def test_apply_marker_in_aggregate_filter_raises() -> None:
    """Custom-apply filters are column-only."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="apply"):

        @sc.aggregate_filter(_Ticket)
        class TicketAggregateFilter:
            count: int = sc.filter_field(apply=lambda s, _v, **_k: s)


def test_two_declared_aggregate_filters_on_one_model_are_distinct() -> None:
    """The DTO cache must not serve one declared aggregate filter in place of another.

    Both classes share the same model and dto_config (`include="all"`) so only the
    cache-key fix -- keying on the declaring class and `functions` -- can tell them apart.
    """
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, include="all", functions=["count"], name="TicketCountOnly")
    class TicketCountOnly: ...

    @sc.aggregate_filter(_Ticket, include="all", functions=["max_datetime"], name="TicketMaxOnly")
    class TicketMaxOnly: ...

    assert _aggregate_fields(TicketCountOnly) == {"count"}
    assert _aggregate_fields(TicketMaxOnly) == {"max_datetime"}


def test_ops_restricts_the_function_predicate() -> None:
    """ops= on an aggregation function exposes only those operators on its predicate."""
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, functions=["count"])
    class TicketAggregateFilter:
        count: int = sc.filter_field(ops=["gt", "eq"])

    assert _predicate_fields(TicketAggregateFilter, "count") == {"gt", "eq"}


def test_unrestricted_function_keeps_full_predicate() -> None:
    """A function without a marker keeps every operator of its comparison."""
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, functions=["count"])
    class TicketAggregateFilter: ...

    assert {"gt", "eq", "lte"} <= _predicate_fields(TicketAggregateFilter, "count")


def test_invalid_aggregation_operator_raises() -> None:
    """An operator not defined on the predicate comparison is rejected at definition time."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="frobnicate"):

        @sc.aggregate_filter(_Ticket, functions=["count"])
        class TicketAggregateFilter:
            count: int = sc.filter_field(ops=["frobnicate"])  # ty: ignore[invalid-argument-type]


def test_aggregation_comparison_annotation_mismatch_raises() -> None:
    """A comparison-type annotation must match the function's own comparison."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="TextComparison"):

        @sc.aggregate_filter(_Ticket, functions=["count"])
        class TicketAggregateFilter:
            # `count` compares an int -> OrderComparison; TextComparison is the wrong shape.
            count: TextComparison = sc.filter_field(ops=["gt"])


def test_aggregation_filter_field_forwards_strawberry_field_args() -> None:
    """filter_field()'s strawberry.field kwargs reach the generated aggregation function field."""
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, functions=["count"])
    class TicketAggregateFilter:
        count: int = sc.filter_field(ops=["gt"], name="howMany", description="Number of tickets")

    definition = get_object_definition(TicketAggregateFilter, strict=True)
    field = next(f for f in definition.fields if (f.graphql_name or f.name) == "howMany")
    assert field.description == "Number of tickets"


def test_arguments_narrows_the_function_enum() -> None:
    """arguments= limits the columns a function can aggregate over."""
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, functions=["count"])
    class TicketAggregateFilter:
        count: int = sc.filter_field(arguments=["name"])

    enum_type = _argument_enum(TicketAggregateFilter, "count")
    assert {member.name for member in enum_type} == {"name"}
    # Scoped by the declaring aggregate filter's own DTO name, not the model, so it can't collide
    # with the shared, unnarrowed `_TicketCountFieldsEnum` or another declared class's narrowing.
    assert enum_type.__name__ == "TicketAggregateFilterCountFieldsEnum"


def test_unnarrowed_function_keeps_every_column() -> None:
    """A function without arguments= keeps the inspector's full column set."""
    sc = Strawchemy("sqlite")

    # `include="all"` is required: a bare DTOConfig excludes every field, leaving `count`'s
    # own argument enum empty.
    @sc.aggregate_filter(_Ticket, include="all", functions=["count"])
    class TicketAggregateFilter: ...

    enum_type = _argument_enum(TicketAggregateFilter, "count")
    # enum member names are camelCased by the enum backend, e.g. `published_at` -> `publishedAt`.
    assert {"id", "name", "publishedAt"} <= {member.name for member in enum_type}


def test_unmarked_function_is_still_narrowed_by_enclosing_include() -> None:
    """An unmarked function can't escape its own decorator's include/exclude scope.

    A prior, unrestricted build of `_Ticket`'s count enum populates the shared, model-scoped
    cache; the declared, restricted aggregate filter's own unmarked `count` must not read it back.
    """
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, include="all", functions=["count"], name="TicketCountUnrestricted")
    class TicketCountUnrestricted: ...

    @sc.aggregate_filter(_Ticket, include=["id", "name"], functions=["count"], name="TicketCountRestricted")
    class TicketCountRestricted:
        count: int = sc.filter_field(ops=["gt"])

    enum_type = _argument_enum(TicketCountRestricted, "count")
    assert {member.name for member in enum_type} == {"id", "name"}
    # Scoped by the declaring aggregate filter's own DTO name, same as an explicit arguments=
    # marker -- the naming rule must hold even when narrowing comes from the enclosing include.
    assert enum_type.__name__ == "TicketCountRestrictedCountFieldsEnum"


def test_arguments_with_unknown_column_raises() -> None:
    """A column that the function cannot aggregate over is rejected at definition time."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="nonexistent"):

        @sc.aggregate_filter(_Ticket, functions=["count"])
        class TicketAggregateFilter:
            count: int = sc.filter_field(arguments=["nonexistent"])


def test_arguments_with_type_incompatible_column_raises() -> None:
    """max_datetime cannot aggregate a string column."""
    sc = Strawchemy("sqlite")
    # `include="all"` is required: a bare DTOConfig excludes every field, so `max_datetime`
    # would not even be an available function to select.
    with pytest.raises(StrawchemyFieldError, match="name"):

        @sc.aggregate_filter(_Ticket, include="all", functions=["max_datetime"])
        class TicketAggregateFilter:
            max_datetime: datetime = sc.filter_field(arguments=["name"])


def test_arguments_outside_enclosing_include_raises() -> None:
    """arguments= cannot re-expose a column the enclosing decorator's include already left out."""
    sc = Strawchemy("sqlite")
    # `published_at` is deliberately left out of `include`; `arguments=` must not silently
    # re-expose it by wholesale-replacing the enclosing include instead of narrowing within it.
    with pytest.raises(StrawchemyFieldError, match="published_at"):

        @sc.aggregate_filter(_Ticket, include=["id", "name"], functions=["count"])
        class TicketAggregateFilter:
            count: int = sc.filter_field(arguments=["published_at"])


def test_arguments_outside_enclosing_exclude_raises() -> None:
    """arguments= cannot re-expose a column the enclosing decorator's exclude already removed."""
    # A fresh Strawchemy instance so this doesn't accidentally pass via a stale cached DTO
    # from an earlier build instead of real validation.
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="published_at"):

        @sc.aggregate_filter(_Ticket, exclude=["published_at"], functions=["count"])
        class TicketAggregateFilter:
            count: int = sc.filter_field(arguments=["published_at"])


def test_arguments_scope_check_not_defeated_by_column_named_like_a_function() -> None:
    """A model column literally named `count` must not mask a real include restriction."""
    sc = Strawchemy("sqlite")
    # `_TicketCountCol.count` is a real column that happens to share its name with the
    # `count` aggregation function; validating scope by stripping declared-attribute names
    # from `include` would wrongly empty it out here and let `published_at` leak through.
    with pytest.raises(StrawchemyFieldError, match="published_at"):

        @sc.aggregate_filter(_TicketCountCol, include=["count"], functions=["count"])
        class TicketAggregateFilterCollision:
            count: int = sc.filter_field(arguments=["published_at"])


def test_declared_aggregate_filter_wired_on_relation() -> None:
    """Annotating <rel>_aggregate swaps the generated aggregate bool exp for the declared one."""
    sc = Strawchemy("sqlite")

    @sc.filter(_Project, include=["name", "tickets"])
    class ProjectFilter:
        tickets_aggregate: _TicketAggregateFilterWired  # ty: ignore[invalid-type-form]

    definition = get_object_definition(ProjectFilter, strict=True)
    field = next(f for f in definition.fields if (f.graphql_name or f.name) == "tickets_aggregate")
    field_type = getattr(field.type, "of_type", field.type)
    assert field_type is _TicketAggregateFilterWired
    assert _aggregate_fields(field_type) == {"count"}


def test_undeclared_relation_keeps_generated_aggregate() -> None:
    """A relation without a declared aggregate filter keeps the full generated one."""
    sc = Strawchemy("sqlite")

    # `include="all"` is required: the aggregate filter's column candidates are drawn from
    # this same config, and a bare/restricted include would leave `max_datetime` unavailable.
    @sc.filter(_Project, include="all")
    class ProjectFilter: ...

    definition = get_object_definition(ProjectFilter, strict=True)
    field = next(f for f in definition.fields if (f.graphql_name or f.name) == "tickets_aggregate")
    field_type = getattr(field.type, "of_type", field.type)
    assert {"count", "max_datetime"} <= _aggregate_fields(cast("type[Any]", field_type))


def test_to_one_relation_has_no_generated_aggregate() -> None:
    sc = Strawchemy("sqlite")

    @sc.filter(_Ticket, include="all")
    class TicketFilter: ...

    @sc.order(_Ticket, include="all")
    class TicketOrderBy: ...

    for dto in (TicketFilter, TicketOrderBy):
        fields = _aggregate_fields(dto)
        assert "project" in fields
        assert "project_aggregate" not in fields


def test_aggregate_annotation_on_to_one_relation_raises() -> None:
    sc = Strawchemy("sqlite")

    with pytest.raises(StrawchemyFieldError, match="to-one"):

        @sc.filter(_Ticket, include=["name", "project"])
        class TicketFilter:
            project_aggregate: _ProjectAggregateFilterMismatch  # ty: ignore[invalid-type-form]


def test_declared_aggregate_filter_model_mismatch_raises() -> None:
    """An aggregate filter built for another model cannot be attached to this relation."""
    sc = Strawchemy("sqlite")

    with pytest.raises(StrawchemyFieldError, match="_Ticket"):

        @sc.filter(_Project, include=["name", "tickets"])
        class ProjectFilter:
            tickets_aggregate: _ProjectAggregateFilterMismatch  # ty: ignore[invalid-type-form]


def test_unmatched_aggregate_annotation_raises() -> None:
    """A <relation>_aggregate annotation naming a relation excluded by include says so."""
    sc = Strawchemy("sqlite")

    with pytest.raises(StrawchemyFieldError, match="excluded by"):

        @sc.filter(_Project, include=["name"])  # `tickets` itself is left out of include
        class ProjectFilter:
            tickets_aggregate: _TicketAggregateFilterWired  # ty: ignore[invalid-type-form]


def test_aggregate_annotation_on_non_relation_raises() -> None:
    """A <relation>_aggregate annotation naming something that is not a relation at all says that."""
    sc = Strawchemy("sqlite")

    with pytest.raises(StrawchemyFieldError, match="is not a relation"):

        @sc.filter(_Project, include=["name", "tickets"])
        class ProjectFilter:
            nonexistent_aggregate: _TicketAggregateFilterWired  # ty: ignore[invalid-type-form]


def test_scope_without_aggregatable_column_raises() -> None:
    """An include leaving a function no candidate column fails at declaration, not at schema build."""
    sc = Strawchemy("sqlite")

    with pytest.raises(StrawchemyFieldError, match="count"):

        @sc.aggregate_filter(_Project, include=["tickets"], functions=["count"])
        class ProjectAggregateFilter: ...


def test_implicit_function_without_aggregatable_column_is_dropped() -> None:
    """A function nobody asked for is dropped when the scope leaves it no column, not raised over."""
    sc = Strawchemy("sqlite")

    # `name` is the only column in scope, so the datetime and numeric functions have nothing to
    # aggregate; `count` counts rows and survives.
    @sc.aggregate_filter(_Ticket, include=["name"], name="NameScopedAggregateFilter")
    class TicketAggregateFilter: ...

    exposed = _aggregate_fields(TicketAggregateFilter)
    assert "max_datetime" not in exposed
    assert "count" in exposed


def test_aggregate_filter_forwards_scope() -> None:
    """scope= reaches the DTO config instead of being silently dropped."""
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, include="all", functions=["count"], scope="schema", name="ScopedAggregateFilter")
    class TicketAggregateFilter: ...

    assert TicketAggregateFilter.__dto_config__.scope == "global"


def test_join_marker_in_aggregate_filter_raises() -> None:
    """join= is only meaningful for a custom column apply filter, not an aggregation function field."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="join"):

        @sc.aggregate_filter(_Ticket, functions=["count"])
        class TicketAggregateFilter:
            count: int = sc.filter_field(join="in")


def test_empty_functions_list_raises() -> None:
    """functions=[] would otherwise build a field-less input; it must raise instead."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="no aggregation function"):

        @sc.aggregate_filter(_Ticket, functions=[])
        class TicketAggregateFilter: ...


def test_aggregation_scalar_annotation_accepted() -> None:
    """An aggregation function annotated with the scalar its predicate compares is accepted."""
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, functions=["count"])
    class TicketAggregateFilter:
        count: int = sc.filter_field(ops=["gt"])

    assert _aggregate_fields(TicketAggregateFilter) == {"count"}


def test_aggregation_comparison_annotation_accepted() -> None:
    """An aggregation function annotated with its own comparison type is accepted."""
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, functions=["count"])
    class TicketAggregateFilter:
        count: OrderComparison[int] = sc.filter_field(ops=["gt"])

    assert _aggregate_fields(TicketAggregateFilter) == {"count"}


def test_aggregation_any_annotation_raises() -> None:
    """`Any` carries no information, so it is not a valid annotation."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="count"):

        @sc.aggregate_filter(_Ticket, functions=["count"])
        class TicketAggregateFilter:
            count: Any = sc.filter_field(ops=["gt"])


def test_aggregation_wrong_scalar_annotation_raises() -> None:
    """A scalar annotation must be the type the function's predicate actually compares."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="int"):

        @sc.aggregate_filter(_Ticket, functions=["count"])
        class TicketAggregateFilter:
            count: str = sc.filter_field(ops=["gt"])


def test_aggregation_bare_marker_needs_no_annotation() -> None:
    """A bare force-include adds no information an annotation could carry, so none is required."""
    sc = Strawchemy("sqlite")

    @sc.aggregate_filter(_Ticket, include="all", functions=["count"])
    class TicketAggregateFilter:
        max_datetime = sc.filter_field()

    assert _aggregate_fields(TicketAggregateFilter) == {"count", "max_datetime"}


def test_column_scalar_annotation_accepted() -> None:
    """A column filter field annotated with its column's data type is accepted."""
    sc = Strawchemy("sqlite")

    @sc.filter(_Ticket, include=["name"])
    class TicketFilter:
        name: str = sc.filter_field(ops=["eq"])

    definition = get_object_definition(TicketFilter, strict=True)
    assert "name" in {f.graphql_name or f.name for f in definition.fields}


def test_column_wrong_scalar_annotation_raises() -> None:
    """A scalar annotation must match the column it refines."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="str"):

        @sc.filter(_Ticket, include=["name"])
        class TicketFilter:
            name: int = sc.filter_field(ops=["eq"])


def test_column_any_annotation_raises() -> None:
    """`Any` is rejected on column filter fields too."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="name"):

        @sc.filter(_Ticket, include=["name"])
        class TicketFilter:
            name: Any = sc.filter_field(ops=["eq"])


def test_column_bare_marker_needs_no_annotation() -> None:
    """A bare column marker needs no annotation."""
    sc = Strawchemy("sqlite")

    @sc.filter(_Ticket, include=["name"])
    class TicketFilter:
        name = sc.filter_field()

    definition = get_object_definition(TicketFilter, strict=True)
    assert "name" in {f.graphql_name or f.name for f in definition.fields}


@pytest.mark.parametrize(
    ("include", "exclude"),
    [
        pytest.param(["id", "published_at"], None, id="include"),
        pytest.param(None, ["name"], id="exclude"),
    ],
)
def test_column_bare_marker_force_includes_out_of_scope_column(
    include: FieldSpec | None, exclude: FieldSpec | None
) -> None:
    """A bare marker without annotation force-includes a column left out by include/exclude."""
    sc = Strawchemy("sqlite")

    @sc.filter(_Ticket, include=include, exclude=exclude)
    class TicketFilter:
        name = sc.filter_field()

    name_field = next(
        f for f in get_object_definition(TicketFilter, strict=True).fields if (f.graphql_name or f.name) == "name"
    )
    assert getattr(name_field.type, "of_type", name_field.type) is TextComparison


def test_column_bare_marker_not_served_from_cache_of_same_scope_filter() -> None:
    """A bare marker is not dropped when a filter with the same include/exclude was built first."""
    sc = Strawchemy("sqlite")

    @sc.filter(_Ticket, exclude=["name"])
    class TicketFilter: ...

    @sc.filter(_Ticket, exclude=["name"])
    class TicketNameFilter:
        name = sc.filter_field()

    assert "name" not in {f.graphql_name or f.name for f in get_object_definition(TicketFilter, strict=True).fields}
    assert "name" in {f.graphql_name or f.name for f in get_object_definition(TicketNameFilter, strict=True).fields}


def test_column_bare_marker_on_unknown_column_raises() -> None:
    """A bare marker naming no model column raises an error saying so."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="'nonexistent' is not a column on _Ticket"):

        @sc.filter(_Ticket, include=["name"])
        class TicketFilter:
            nonexistent = sc.filter_field()


def test_column_bare_marker_on_relation_raises() -> None:
    """A bare marker naming a relation raises: only columns take a bare marker."""
    sc = Strawchemy("sqlite")
    with pytest.raises(StrawchemyFieldError, match="'project' is not a column on _Ticket"):

        @sc.filter(_Ticket, include=["name"])
        class TicketFilter:
            project = sc.filter_field()


def test_column_bare_marker_on_unexposed_column_does_not_claim_it_is_missing() -> None:
    """An existing column the filter cannot expose is reported as such, never as "not a column"."""
    sc = Strawchemy(StrawchemyConfig("sqlite", strict=False))
    with (
        pytest.warns(UserWarning, match="no GraphQL mapping"),
        pytest.raises(StrawchemyFieldError, match="column 'data' on _TicketUnmappable"),
    ):

        @sc.filter(_TicketUnmappable, include=["id"])
        class TicketFilter:
            data = sc.filter_field()


def test_custom_apply_annotation_is_the_input_type() -> None:
    """On an apply field the annotation IS the generated input scalar, so it is not column-checked."""
    sc = Strawchemy("sqlite")

    @sc.filter(_Ticket, include=["name"])
    class TicketFilter:
        # `datetime` matches no column named `published_after` — the annotation defines the input.
        published_after: datetime = sc.filter_field(apply=_published_after)

    definition = get_object_definition(TicketFilter, strict=True)
    assert "published_after" in {f.graphql_name or f.name for f in definition.fields}


@pytest.mark.parametrize(
    ("field", "query_filter", "expected_message"),
    [
        pytest.param(
            "projects",
            "{ ticketsAggregate: { avg: { arguments: [id], predicate: { gt: 1 } } } }",
            "Expected value of type 'TicketAggregateFilter' not to include unknown field 'avg', "
            "found: { avg: { arguments: [id], predicate: { gt: 1 } } }.",
            id="function-outside-functions",
        ),
        pytest.param(
            "projects",
            "{ ticketsAggregate: { count: { arguments: [id], predicate: { lt: 1 } } } }",
            "Expected value of type 'OrderComparisonIntEqGt' not to include unknown field 'lt'. "
            "Did you mean 'gt'? Found: { lt: 1 }.",
            id="operator-outside-ops",
        ),
        pytest.param(
            "projects",
            "{ ticketsAggregate: { count: { arguments: [name], predicate: { gt: 1 } } } }",
            "Value 'name' does not exist in 'TicketAggregateFilterCountFieldsEnum' enum.",
            id="column-outside-arguments",
        ),
        pytest.param(
            "projectsScoped",
            "{ ticketsAggregate: { count: { arguments: [name], predicate: { gt: 1 } } } }",
            "Value 'name' does not exist in 'TicketScopedAggregateCountFieldsEnum' enum.",
            id="column-outside-decorator-include",
        ),
        pytest.param(
            "tickets",
            '{ name: { contains: "x" } }',
            "Expected value of type 'TextComparisonStrEq' not to include unknown field 'contains', "
            'found: { contains: "x" }.',
            id="column-operator-outside-ops",
        ),
    ],
)
def test_out_of_scope_selection_is_rejected(field: str, query_filter: str, expected_message: str) -> None:
    """A selection the declared filter does not expose fails GraphQL validation."""
    result = _REJECTION_SCHEMA.execute_sync(f"{{ {field}(filter: {query_filter}) {{ name }} }}")

    assert result.errors is not None
    assert [error.message for error in result.errors] == [expected_message]


def _operator_names(alias: Any) -> set[str]:
    """Flattens an operator alias to its literal values, descending through composed unions."""
    names: set[str] = set()
    for arg in get_args(alias):
        names.add(arg) if isinstance(arg, str) else names.update(_operator_names(arg))
    return names


@pytest.mark.parametrize(
    ("comparison", "operator_alias"),
    [
        pytest.param(EqualityComparison, EqualityOperator, id="equality"),
        pytest.param(OrderComparison, OrderOperator, id="order"),
        pytest.param(TextComparison, TextOperator, id="text"),
        pytest.param(ArrayComparison, ArrayOperator, id="array"),
        pytest.param(DateComparison, DateOperator, id="date"),
        pytest.param(TimeComparison, TimeOperator, id="time"),
        pytest.param(DateTimeComparison, DateTimeOperator, id="datetime"),
        pytest.param(TimeDeltaComparison, TimeDeltaOperator, id="timedelta"),
    ],
)
def test_operator_alias_matches_its_comparison(comparison: type[Any], operator_alias: Any) -> None:
    """Each operator alias lists exactly the operators its comparison class defines."""
    assert {op.graphql_name for op in comparison._all_operators()} == _operator_names(operator_alias)  # noqa: SLF001


def test_comparison_operator_covers_every_alias() -> None:
    """The umbrella alias is the union of the per-comparison ones."""
    covered = (
        _operator_names(TextOperator)
        | _operator_names(ArrayOperator)
        | _operator_names(DateTimeOperator)
        | _operator_names(TimeDeltaOperator)
    )
    assert _operator_names(ComparisonOperator) == covered


@pytest.mark.snapshot
def test_fine_grained_aggregation_schema(graphql_snapshot: SnapshotAssertion) -> None:
    """A declared aggregate filter and a generated sibling coexist in the printed schema."""
    sc = Strawchemy("sqlite")

    @sc.type(_Project, include=["name"])
    class ProjectType: ...

    @sc.filter(_Project, include=["name", "tickets"])
    class ProjectFilter:
        tickets_aggregate: _TicketAggregateFilterSchema  # ty: ignore[invalid-type-form]

    @strawberry.type
    class Query:
        projects: list[ProjectType] = sc.field(filter_input=ProjectFilter)

    schema = strawberry.Schema(query=Query)
    assert str(schema) == graphql_snapshot
