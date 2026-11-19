from __future__ import annotations

import dataclasses as dc
import re
import textwrap
from datetime import timedelta
from importlib import import_module
from importlib.util import find_spec
from typing import TYPE_CHECKING, Any

import pytest
import strawberry
from strawberry import auto
from strawberry.scalars import JSON
from strawberry.schema.types.scalar import DEFAULT_SCALAR_REGISTRY
from strawberry.types import get_object_definition
from strawberry.types.object_type import StrawberryObjectDefinition

from strawchemy import RELATIONSHIPS, SCALARS
from strawchemy.exceptions import EmptyDTOError, QueryHookError, StrawchemyError, StrawchemyFieldError
from strawchemy.schema.scalars import Interval
from strawchemy.testing import MockContext
from strawchemy.utils.strawberry import strawberry_contained_user_type
from tests.fixtures import DefaultQuery
from tests.unit.models import Book as BookModel
from tests.unit.models import Color, Fruit, User
from tests.utils import DTOInspect

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy.mapper import Strawchemy

SCALAR_OVERRIDES: dict[object, Any] = {dict[str, Any]: DEFAULT_SCALAR_REGISTRY[JSON], timedelta: Interval}


def test_type_instance(strawchemy: Strawchemy) -> None:
    @strawchemy.type(User)
    class UserType:
        id: auto
        name: auto

    user = UserType(id=1, name="user")  # ty: ignore[unknown-argument]
    assert user.id == 1
    assert user.name == "user"


def test_override_same_name_does_not_leak_fields(strawchemy: Strawchemy) -> None:
    """The last override wins when used with the same model/name pair.

    A second `override=True` registration under the same GraphQL type name must
    reflect its own (narrower) config, not inherit fields from the earlier
    registration.
    """
    from tests.unit.models import Color, Fruit

    @strawchemy.type(Fruit, name="FruitNode", include=["id"], override=True)
    class FruitNode:
        pass

    @strawchemy.type(Color, name="ColorNode", include=["id", "name", "fruits"], override=True)
    class ColorWide:
        pass

    @strawchemy.type(Color, name="ColorNode", include=["id"], override=True)
    class ColorSlim:
        pass

    wide_fields = {f.name for f in get_object_definition(ColorWide, strict=True).fields}
    slim_fields = {f.name for f in get_object_definition(ColorSlim, strict=True).fields}

    assert wide_fields == {"fruits_aggregate", "fruits", "id", "name"}
    assert slim_fields == {"id"}


def test_type_instance_auto_as_str(strawchemy: Strawchemy) -> None:
    @strawchemy.type(User)
    class UserType:
        id: auto
        name: auto

    user = UserType(id=1, name="user")  # ty: ignore[unknown-argument]
    assert user.id == 1
    assert user.name == "user"


def test_input_instance(strawchemy: Strawchemy) -> None:
    @strawchemy.create_input(User)
    class InputType:
        id: auto
        name: auto

    user = InputType(id=1, name="user")  # ty: ignore[unknown-argument]
    assert user.id == 1
    assert user.name == "user"


@pytest.mark.parametrize(
    ("decorator", "default_value"),
    [
        pytest.param("type", "anon", id="type"),
        pytest.param("order", "anon", id="order"),
        pytest.param("filter", "anon", id="filter"),
        pytest.param("create_input", "anon", id="create_input"),
        pytest.param("pk_update_input", "anon", id="pk_update_input"),
        pytest.param("filter_update_input", "anon", id="filter_update_input"),
    ],
)
def test_class_body_literal_defaults_preserved(strawchemy: Strawchemy, decorator: str, default_value: Any) -> None:
    """User-written class-body literal defaults survive every row-shaped strawchemy decorator."""

    @getattr(strawchemy, decorator)(Fruit, include=["name", "sweetness"])
    class FruitTestType:
        name: str | None = default_value

    fields = {f.name: f for f in FruitTestType.__strawberry_definition__.fields}
    assert fields["name"].default == default_value, FruitTestType.__name__
    # Fields without a class-body default keep their generated default
    assert {f.name: f for f in FruitTestType.__strawberry_definition__.fields}["sweetness"].default in (
        dc.MISSING,
        strawberry.UNSET,
    )
    # The literal default applies when instantiating the type
    assert FruitTestType(sweetness=1).name == default_value


@pytest.mark.parametrize(
    ("decorator", "default_value"),
    [
        pytest.param("aggregate", "anon", id="filter_update_input"),
        pytest.param("aggregate_filter", "anon", id="filter_update_input"),
    ],
)
def test_class_body_literal_defaults_preserved_aggregate(
    strawchemy: Strawchemy, decorator: str, default_value: Any
) -> None:
    @getattr(strawchemy, decorator)(Fruit, include=["name", "sweetness"])
    class FruitTestType:
        name: str | None = default_value

    fields = {f.name: f for f in FruitTestType.__strawberry_definition__.fields}
    assert fields["name"].default == default_value, FruitTestType.__name__


def test_field_metadata_default(strawchemy: Strawchemy) -> None:
    """Test metadata default.

    Test that textual metadata from the SQLAlchemy model isn't reflected in the Strawberry
    type by default.
    """

    @strawchemy.type(BookModel)
    class Book:
        title: auto

    type_def = get_object_definition(Book, strict=True)
    assert type_def.description == "GraphQL type"
    title_field = type_def.get_field("title")
    assert title_field is not None
    assert title_field.description is None


def test_type_resolution_with_resolvers() -> None:
    from tests.unit.schemas.resolver.custom_resolver import ColorType, Query

    schema = strawberry.Schema(query=Query)
    type_def = schema.get_type_by_name("FruitType")
    assert isinstance(type_def, StrawberryObjectDefinition)
    field = type_def.get_field("color")
    assert field
    assert field.type is ColorType


@pytest.mark.parametrize(
    "path",
    [pytest.param("tests.unit.schemas.override.auto_type_existing", id="auto_type_existing")],
)
def test_multiple_types_error(path: str) -> None:
    with pytest.raises(StrawchemyError, match=re.escape("Type `FruitType` is already registered")):
        import_module(path)


@pytest.mark.parametrize(
    "module",
    [
        pytest.param("tests.unit.schemas.aggregations.type_mismatch", id="strawchemy_type"),
        pytest.param("tests.unit.schemas.aggregations.type_mismatch_plain_type", id="plain_type"),
        pytest.param("tests.unit.schemas.aggregations.type_mismatch_custom_resolver", id="custom_resolver"),
    ],
)
def test_aggregation_type_mismatch(module: str) -> None:
    with pytest.raises(
        StrawchemyFieldError,
        match=re.escape(
            """The `color_aggregations` field is defined with `root_aggregations` enabled but the field type is not a root aggregation type."""
        ),
    ):
        import_module(module)


@pytest.mark.parametrize(
    ("module", "type_name"),
    [
        pytest.param("root_list", "Plain", id="root_list"),
        pytest.param("root_single", "Plain", id="root_single"),
        pytest.param("root_scalar", "int", id="root_scalar"),
        pytest.param("root_union_plain_first", "Plain", id="root_union_plain_first"),
        pytest.param("type_body_field", "Plain", id="type_body_field"),
        pytest.param("default_order_by_no_resolver", "Plain", id="default_order_by_no_resolver"),
        pytest.param("create_mutation", "Plain", id="create_mutation"),
        pytest.param("upsert_mutation", "Plain", id="upsert_mutation"),
        pytest.param("update_mutation", "Plain", id="update_mutation"),
        pytest.param("delete_mutation", "Plain", id="delete_mutation"),
    ],
)
def test_resolverless_field_on_plain_type_fail(module: str, type_name: str) -> None:
    """Test that a root field without resolver is rejected when its type is not a strawchemy type."""
    with pytest.raises(
        StrawchemyFieldError,
        match=re.escape(f"The `plain` field has no resolver but its type `{type_name}` is not a strawchemy type."),
    ):
        import_module(f"tests.unit.schemas.plain_type.{module}")


def test_default_order_by_on_plain_type_fail() -> None:
    """Test that `default_order_by` is rejected on a field with a resolver returning a plain type."""
    with pytest.raises(
        StrawchemyFieldError,
        match=re.escape(
            "`default_order_by` cannot be set on `plain` because its type `Plain` is not a strawchemy type."
        ),
    ):
        import_module("tests.unit.schemas.plain_type.default_order_by_resolver")


@pytest.mark.parametrize("module", ["delete_mutation", "update_mutation"])
def test_mutation_type_resolved_after_definition(module: str) -> None:
    """Test that a list mutation type defined after the field is validated once resolved."""
    schema_module = import_module(f"tests.unit.schemas.unresolved_type.{module}")
    strawberry.Schema(query=schema_module.Query, mutation=schema_module.Mutation)


@pytest.mark.parametrize(
    ("module", "message"),
    [
        pytest.param("delete_mutation_not_list", "Type of delete mutation must be a list: delete_color", id="delete"),
        pytest.param(
            "update_mutation_not_list", "Type of update mutation by filter must be a list: update_color", id="update"
        ),
    ],
)
def test_mutation_type_resolved_after_definition_not_list_fail(module: str, message: str) -> None:
    """Test that a non-list mutation type defined after the field fails the list check once resolved."""
    schema_module = import_module(f"tests.unit.schemas.unresolved_type.{module}")
    # Strawberry wraps errors raised while resolving field types at schema build.
    with pytest.raises(TypeError, match=re.escape(message)) as exc_info:
        strawberry.Schema(query=schema_module.Query, mutation=schema_module.Mutation)
    assert isinstance(exc_info.value.__cause__, StrawchemyFieldError)


@pytest.mark.parametrize(
    ("module", "context"),
    [
        pytest.param("root_list", "The `errors` field has no resolver but", id="root_list"),
        pytest.param("create_mutation", "The `errors` field has no resolver but", id="create_mutation"),
        pytest.param(
            "default_order_by_resolver",
            "`default_order_by` cannot be set on `errors` because",
            id="default_order_by_resolver",
        ),
    ],
)
def test_error_only_type_fail(module: str, context: str) -> None:
    """Test that a field needing a strawchemy type is rejected when its type only contains error types."""
    with pytest.raises(StrawchemyFieldError, match=re.escape(f"{context} its type only contains error types.")):
        import_module(f"tests.unit.schemas.error_only_type.{module}")


def test_error_only_type_with_resolver_accepted() -> None:
    """Test that a field with a resolver can return only error types."""
    from tests.unit.schemas.error_only_type.resolver import Query

    assert "errors: [ValidationErrorType!]!" in str(strawberry.Schema(query=Query))


@pytest.mark.snapshot
def test_plain_type_fields_accepted(graphql_snapshot: SnapshotAssertion) -> None:
    """Test that plain types are accepted behind a resolver or after the strawchemy type of a union."""
    from tests.unit.schemas.plain_type.accepted import Mutation, Query

    schema = strawberry.Schema(query=Query, mutation=Mutation)
    assert textwrap.dedent(str(schema)).strip() == graphql_snapshot


def test_registry_type_shadows_module_name() -> None:
    """Test that a postponed annotation resolves a registry type over a module name bound to another type."""
    from tests.unit.schemas.plain_type.registry_shadowing import Query, RegistryColorType

    field = get_object_definition(Query, strict=True).get_field("color")
    assert field is not None
    assert field.type is RegistryColorType


def test_postponed_field_resolves_type_defined_after_it() -> None:
    """Test that a postponed field annotation resolves a registry type defined later over a module name."""
    from tests.unit.schemas.late_type.root_field import Query, RegistryColorType

    strawberry.Schema(query=Query)
    field = get_object_definition(Query, strict=True).get_field("colors")
    assert field is not None
    assert strawberry_contained_user_type(field.type) is RegistryColorType


def test_postponed_delete_mutation_resolves_type_defined_after_it() -> None:
    """Test that a postponed delete mutation annotation resolves a strawchemy type defined later in the module."""
    from tests.unit.schemas.late_type.delete_mutation import ColorType, Mutation

    strawberry.Schema(query=DefaultQuery, mutation=Mutation)
    field = get_object_definition(Mutation, strict=True).get_field("delete_colors")
    assert field is not None
    assert strawberry_contained_user_type(field.type) is ColorType


def test_query_hooks_wrong_relationship_load_spec() -> None:
    with pytest.raises(
        QueryHookError, match=re.escape("Keys of mappings passed in `load` param must be relationship attributes: ")
    ):
        import_module("tests.unit.schemas.query_hooks")


def test_excluding_pk_from_update_input_fail() -> None:
    with pytest.raises(
        StrawchemyError,
        match=re.escape(
            "You cannot exclude primary key columns from an input type intended for create or update mutations"
        ),
    ):
        import_module("tests.unit.schemas.mutations.invalid_pk_update_input")


def test_read_only_pk_on_update_input_fail() -> None:
    with pytest.raises(
        EmptyDTOError,
        match=re.escape(
            "Cannot generate `NewGroupUsersIdFieldsInput` input type from `NewUser` model because primary key columns are disabled for write purpose"
        ),
    ):
        import_module("tests.unit.schemas.mutations.read_only_pk_with_update_input")


def test_delete_mutation_type_not_list_fail() -> None:
    with pytest.raises(
        StrawchemyFieldError,
        match=re.escape("Type of delete mutation must be a list: delete_group"),
    ):
        import_module("tests.unit.schemas.mutations.delete_mutation_type_not_list")


def test_update_mutation_by_filter_type_not_list_fail() -> None:
    with pytest.raises(
        StrawchemyFieldError,
        match=re.escape("Type of update mutation by filter must be a list: update_groups"),
    ):
        import_module("tests.unit.schemas.mutations.invalid_filter_update_field")


@pytest.mark.parametrize(
    "path",
    [
        pytest.param("include.all_fields.Query", id="all_fields"),
        pytest.param("include.all_fields_override.Query", id="all_fields_override"),
        pytest.param("include.all_fields_filter.Query", id="all_fields_with_filter"),
        pytest.param("include.all_order_by.Query", id="all_fields_order_by"),
        pytest.param("include.include_explicit.Query", id="include_explicit"),
        pytest.param("include.include_non_existent.Query", id="include_non_existent"),
        pytest.param("exclude.exclude_explicit.Query", id="exclude_explicit"),
        pytest.param("exclude.exclude_non_existent.Query", id="exclude_non_existent"),
        pytest.param("exclude.exclude_and_override_type.Query", id="exclude_and_override_type"),
        pytest.param("exclude.exclude_and_override_field.Query", id="exclude_and_override_field"),
        pytest.param("resolver.primary_key_resolver.Query", id="primary_key_resolver"),
        pytest.param("resolver.list_resolver.Query", id="list_resolver"),
        pytest.param("resolver.class_body_resolver_override.Query", id="class_body_resolver_override"),
        pytest.param("model_field.field_options.Query", id="model_field_options"),
        pytest.param("override.override_argument.Query", id="argument_override"),
        pytest.param("override.override_auto_type.Query", id="override_auto_type"),
        pytest.param("override.override_with_custom_name.Query", id="override_with_custom_name"),
        pytest.param("override.nested_overrides.Query", id="nested_overrides"),
        pytest.param("pagination.pagination.Query", id="pagination"),
        pytest.param("pagination.pagination_defaults.Query", id="pagination_defaults"),
        pytest.param("pagination.children_pagination.Query", id="children_pagination"),
        pytest.param("pagination.children_pagination_defaults.Query", id="children_pagination_defaults"),
        pytest.param("pagination.pagination_config_default.Query", id="pagination_config_default"),
        pytest.param("pagination.pagination_default_limit.Query", id="pagination_default_limit"),
        pytest.param("pagination.pagination_default_offset.Query", id="pagination_default_offset"),
        pytest.param("pagination.paginate_specific_fields.Query", id="paginate_specific_fields"),
        pytest.param("pagination.paginate_empty.Query", id="paginate_empty"),
        pytest.param("pagination.pagination_config_empty.Query", id="pagination_config_empty"),
        pytest.param("pagination.pagination_config_specific_fields.Query", id="pagination_config_specific_fields"),
        pytest.param(
            "pagination.pagination_config_with_type_override.Query", id="pagination_config_with_type_override"
        ),
        pytest.param("pagination.paginate_and_order_combined.Query", id="paginate_and_order_combined"),
        pytest.param("pagination.paginate_all_with_default.Query", id="paginate_all_with_default"),
        pytest.param("custom_id_field_name.Query", id="custom_id_field_name"),
        pytest.param("enums.Query", id="enums"),
        pytest.param("filters.filters.Query", id="filters"),
        pytest.param("filters.filters_aggregation.Query", id="aggregation_filters"),
        pytest.param("filters.type_filter.Query", id="type_filter"),
        pytest.param("filters.field_filter_auto_generate.Query", id="field_filter_auto_generate"),
        pytest.param("filters.field_filter_object_relation.Query", id="field_filter_object_relation"),
        pytest.param("order.type_order_by.Query", id="type_order_by"),
        pytest.param("order.field_order_by.Query", id="field_order_by"),
        pytest.param("order.field_order_by_all.Query", id="field_order_by_all"),
        pytest.param("order.field_order_by_all_object_relation.Query", id="field_order_by_all_object_relation"),
        pytest.param("order.field_order_by_specific_fields.Query", id="field_order_by_specific_fields"),
        pytest.param("order.order_config_all.Query", id="order_config_all"),
        pytest.param(
            "order.order_config_specific_fields_with_type_override.Query",
            id="order_config_specific_fields_with_type_override",
        ),
        pytest.param(
            "order.order_config_empty_with_empty_type_override.Query", id="order_config_empty_with_empty_type_override"
        ),
        pytest.param("order.order_config_all_with_field_override.Query", id="order_config_all_with_field_override"),
        pytest.param("order.order_config_specific_fields.Query", id="order_config_specific_fields"),
        pytest.param("order.order_config_empty.Query", id="order_config_empty"),
        pytest.param("order.order_config_with_field_override.Query", id="order_config_with_field_override"),
        pytest.param("order.type_order_by_specific_fields.Query", id="type_order_by_specific_fields"),
        pytest.param("aggregations.root_aggregations.Query", id="root_aggregations"),
        pytest.param("distinct.type_distinct_manual_enum.Query", id="type_distinct_manual_enum"),
        pytest.param("distinct.distinct_config_all.Query", id="distinct_config_all"),
        pytest.param("distinct.field_distinct_all.Query", id="field_distinct_all"),
        pytest.param("distinct.field_distinct_specific_fields.Query", id="field_distinct_specific_fields"),
        pytest.param("distinct.distinct_config_with_field_override.Query", id="distinct_config_with_field_override"),
        pytest.param("distinct.distinct_config_specific_fields.Query", id="distinct_config_specific_fields"),
        pytest.param("distinct.distinct_config_empty.Query", id="distinct_config_empty"),
        pytest.param("scope.schema_before.Query", id="scope_schema_before"),
        pytest.param("scope.schema_after.Query", id="scope_schema_after"),
        pytest.param("scope.schema_middle.Query", id="scope_schema_middle"),
        pytest.param("lazy.query.Query", id="lazy_circular_default_scope"),
        pytest.param("lazy_global.query.Query", id="lazy_circular_global_scope"),
        pytest.param("forwardref.query.Query", id="forwardref_circular_default_scope"),
        pytest.param("forwardref_global.query.Query", id="forwardref_circular_global_scope"),
        pytest.param("union_override_lazy.Query", id="union_override_lazy"),
        pytest.param("union_override_plain.Query", id="union_override_plain"),
    ],
)
@pytest.mark.snapshot
def test_query_schemas(path: str, graphql_snapshot: SnapshotAssertion) -> None:
    module, query_name = f"tests.unit.schemas.{path}".rsplit(".", maxsplit=1)
    query_class = getattr(import_module(module), query_name)

    schema = strawberry.Schema(query=query_class, scalar_overrides=SCALAR_OVERRIDES)
    assert textwrap.dedent(str(schema)).strip() == graphql_snapshot


@pytest.mark.parametrize(
    "path", [pytest.param("geo.geo_filters.Query", id="geo_filters"), pytest.param("geo.geo.Query", id="geo_type")]
)
@pytest.mark.geo
@pytest.mark.extras
@pytest.mark.snapshot
@pytest.mark.skipif(not find_spec("geoalchemy2"), reason="geoalchemy2 is not installed")
def test_geo_schemas(path: str, graphql_snapshot: SnapshotAssertion) -> None:
    from strawchemy.schema.scalars.geo import GEO_SCALAR_OVERRIDES

    module, query_name = f"tests.unit.schemas.{path}".rsplit(".", maxsplit=1)
    query_class = getattr(import_module(module), query_name)

    schema = strawberry.Schema(query=query_class, scalar_overrides=(SCALAR_OVERRIDES | GEO_SCALAR_OVERRIDES))
    assert textwrap.dedent(str(schema)).strip() == graphql_snapshot


@pytest.mark.parametrize(
    "path",
    [
        pytest.param("create.Mutation", id="create_mutation"),
        pytest.param("update.Mutation", id="update_mutation"),
        pytest.param("delete.Mutation", id="delete_mutation"),
        pytest.param("create_no_id.Mutation", id="create_no_id"),
        pytest.param("upsert.Mutation", id="upsert"),
        pytest.param("sql_expression_default.Mutation", id="sql_expression_default"),
    ],
)
@pytest.mark.snapshot
def test_mutation_schemas(path: str, graphql_snapshot: SnapshotAssertion) -> None:
    module, query_name = f"tests.unit.schemas.mutations.{path}".rsplit(".", maxsplit=1)
    mutation_class = getattr(import_module(module), query_name)

    @strawberry.type
    class Query:
        @strawberry.field
        def hello(self) -> str:
            return "world"

    schema = strawberry.Schema(query=Query, mutation=mutation_class, scalar_overrides=SCALAR_OVERRIDES)
    assert textwrap.dedent(str(schema)).strip() == graphql_snapshot


@pytest.mark.parametrize(
    ("path", "field_name", "description"),
    [
        pytest.param("create.Mutation", "create_group", "Create object in the GroupType collection", id="create"),
        pytest.param(
            "create.Mutation", "create_groups", "Create objects in the GroupType collection", id="create_list"
        ),
        pytest.param("update.Mutation", "update_group_by_id", "Update object in the GroupType collection", id="update"),
        pytest.param(
            "update.Mutation", "update_groups", "Update objects in the GroupType collection", id="update_filter"
        ),
        pytest.param("upsert.Mutation", "upsert_fruit", "Upsert object in the FruitType collection", id="upsert"),
        pytest.param(
            "delete.Mutation", "delete_groups", "Delete objects in the GroupType collection", id="delete_list"
        ),
    ],
)
def test_mutation_field_description(path: str, field_name: str, description: str) -> None:
    """Test that generated mutation fields describe the mutation instead of a fetch."""
    module, mutation_name = f"tests.unit.schemas.mutations.{path}".rsplit(".", maxsplit=1)
    mutation_class = getattr(import_module(module), mutation_name)
    strawberry.Schema(query=DefaultQuery, mutation=mutation_class, scalar_overrides=SCALAR_OVERRIDES)

    field = get_object_definition(mutation_class, strict=True).get_field(field_name)
    assert field is not None
    assert field.description == description


@pytest.mark.parametrize(
    ("type_name", "field_name", "description"),
    [
        pytest.param("Query", "plain", None, id="plain_list_resolver"),
        pytest.param("Query", "plain_by_id", None, id="plain_resolver"),
        pytest.param("ColorType", "plain", None, id="plain_type_body_resolver"),
        pytest.param("Mutation", "create_plain", None, id="plain_create_resolver"),
        pytest.param("Mutation", "create_color", "Create object in the ColorType collection", id="error_union"),
        pytest.param("Query", "colors", "Fetch objects from the ColorType collection", id="strawchemy_list"),
        pytest.param(
            "Query", "color_or_plain", "Fetch object from the ColorType collection by id", id="strawchemy_first_union"
        ),
    ],
)
def test_plain_type_field_description(type_name: str, field_name: str, description: str | None) -> None:
    """Test that fields only get an auto description when they return a strawchemy type."""
    module = import_module("tests.unit.schemas.plain_type.accepted")
    strawberry.Schema(query=module.Query, mutation=module.Mutation)

    field = get_object_definition(getattr(module, type_name), strict=True).get_field(field_name)
    assert field is not None
    assert field.description == description


def test_mutation_field_explicit_description(strawchemy: Strawchemy) -> None:
    """Test that an explicit description wins over the generated mutation description."""

    @strawchemy.type(Color, include=["name"])
    class ColorType: ...

    @strawchemy.create_input(Color, include=["name"])
    class ColorCreate: ...

    @strawberry.type
    class Mutation:
        create_color: ColorType = strawchemy.create(ColorCreate, description="custom")

    strawberry.Schema(query=DefaultQuery, mutation=Mutation)
    field = get_object_definition(Mutation, strict=True).get_field("create_color")
    assert field is not None
    assert field.description == "custom"


@pytest.mark.snapshot
def test_query_and_mutations(graphql_snapshot: SnapshotAssertion) -> None:
    from tests.unit.schemas.mutation_and_query import Mutation, Query

    schema = strawberry.Schema(query=Query, mutation=Mutation)
    assert textwrap.dedent(str(schema)).strip() == graphql_snapshot


def test_field_filter_equals_type_filter() -> None:
    from tests.unit.schemas.filters.filters import Query as FieldFilterQuery
    from tests.unit.schemas.filters.type_filter import Query as TypeFilterQuery

    field_filter_schema = strawberry.Schema(query=FieldFilterQuery, scalar_overrides=SCALAR_OVERRIDES)
    type_filter_schema = strawberry.Schema(query=TypeFilterQuery, scalar_overrides=SCALAR_OVERRIDES)

    assert textwrap.dedent(str(field_filter_schema)).strip() == textwrap.dedent(str(type_filter_schema)).strip()


def test_field_order_by_equals_type_order_by() -> None:
    from tests.unit.schemas.order.field_order_by import Query as FieldOrderQuery
    from tests.unit.schemas.order.type_order_by import Query as TypeOrderQuery

    field_filter_schema = strawberry.Schema(query=FieldOrderQuery, scalar_overrides=SCALAR_OVERRIDES)
    type_filter_schema = strawberry.Schema(query=TypeOrderQuery, scalar_overrides=SCALAR_OVERRIDES)

    assert textwrap.dedent(str(field_filter_schema)).strip() == textwrap.dedent(str(type_filter_schema)).strip()


@pytest.mark.parametrize(
    ("query", "name", "is_list"),
    [
        pytest.param(
            """
            mutation {
                createUser(
                    data: {
                        name: "Bob",
                        group: { set: { id: "da636751-b276-4546-857f-3c73ea914467" } },
                        tag: { set: { id: "da636751-b276-4546-857f-3c73ea914467" } }
                    }
                ) {
                    __typename
                    ... on UserType {
                        name
                    }
                    ... on ValidationErrorType {
                        id
                        errors {
                            id
                            loc
                            message
                            type
                        }
                    }
                }
            }
            """,
            "createUser",
            False,
            id="create",
        ),
        pytest.param(
            """
            mutation {
                createUserCustom(
                    data: {
                        name: "Bob",
                        group: { set: { id: "da636751-b276-4546-857f-3c73ea914467" } },
                        tag: { set: { id: "da636751-b276-4546-857f-3c73ea914467" } }
                    }
                ) {
                    __typename
                    ... on UserType {
                        name
                    }
                    ... on ValidationErrorType {
                        id
                        errors {
                            id
                            loc
                            message
                            type
                        }
                    }
                }
            }
            """,
            "createUserCustom",
            False,
            id="create-custom",
        ),
        pytest.param(
            """
            mutation {
                updateUsers(
                    filter: { id: { eq: "da636751-b276-4546-857f-3c73ea914467" } },
                    data: { name: "Bob" }
                ) {
                    __typename
                    ... on UserType {
                        name
                    }
                    ... on ValidationErrorType {
                        id
                        errors {
                            id
                            loc
                            message
                            type
                        }
                    }
                }
            }
            """,
            "updateUsers",
            True,
            id="update_by_filter",
        ),
        pytest.param(
            """
            mutation {
                updateUserByIds(
                    data: [
                        {
                            id: "da636751-b276-4546-857f-3c73ea914467",
                            name: "Bob"
                        }
                    ]
                ) {
                    __typename
                    ... on UserType {
                        name
                    }
                    ... on ValidationErrorType {
                        id
                        errors {
                            id
                            loc
                            message
                            type
                        }
                    }
                }
            }
            """,
            "updateUserByIds",
            True,
            id="update_by_ids",
        ),
        pytest.param(
            """
            mutation {
                updateUserById(
                    data: {
                        id: "da636751-b276-4546-857f-3c73ea914467",
                        name: "Bob"
                    }
                ) {
                    __typename
                    ... on UserType {
                        name
                    }
                    ... on ValidationErrorType {
                        id
                        errors {
                            id
                            loc
                            message
                            type
                        }
                    }
                }
            }
            """,
            "updateUserById",
            False,
            id="update_by_id",
        ),
    ],
)
@pytest.mark.skipif(not find_spec("pydantic"), reason="pydantic is not installed")
def test_pydantic_validation(query: str, name: str, is_list: bool) -> None:
    from tests.unit.schemas.pydantic.validation import Mutation

    schema = strawberry.Schema(query=DefaultQuery, mutation=Mutation, scalar_overrides=SCALAR_OVERRIDES)
    result = schema.execute_sync(query, context_value=MockContext("postgresql"))
    assert not result.errors
    assert result.data

    error = result.data[name][0] if is_list else result.data[name]
    assert error["__typename"] == "ValidationErrorType"
    assert error["id"] == "ERROR"
    assert error["errors"] == [
        {
            "id": "ERROR",
            "loc": ["name"],
            "message": "Value error, Name must be lower cased",
            "type": "value_error",
        }
    ]


@pytest.mark.skipif(not find_spec("pydantic"), reason="pydantic is not installed")
def test_pydantic_validation_nested() -> None:
    from tests.unit.schemas.pydantic.validation import Mutation

    query = """
        mutation {
            createUser(
                data: {
                    name: "bob",
                    tag: { set: { id: "da636751-b276-4546-857f-3c73ea914467" } }
                    group: {
                        create: {
                            name: "Group",
                            tag: { set: { id: "da636751-b276-4546-857f-3c73ea914467" } },
                            color: { set: { id: "da636751-b276-4546-857f-3c73ea914467" } }
                        }
                    }
                }
            ) {
                __typename
                ... on UserType {
                    name
                }
                ... on ValidationErrorType {
                    id
                    errors {
                        id
                        loc
                        message
                        type
                    }
                }
            }
        }
    """
    schema = strawberry.Schema(query=DefaultQuery, mutation=Mutation, scalar_overrides=SCALAR_OVERRIDES)
    result = schema.execute_sync(query, context_value=MockContext("postgresql"))
    assert not result.errors
    assert result.data

    assert result.data["createUser"]["__typename"] == "ValidationErrorType"
    assert result.data["createUser"]["id"] == "ERROR"
    assert result.data["createUser"]["errors"] == [
        {
            "id": "ERROR",
            "loc": ["group", "name"],
            "message": "Value error, Name must be lower cased",
            "type": "value_error",
        }
    ]


@pytest.mark.parametrize(
    "module_name",
    [
        pytest.param("schema_after", id="scope-after"),
        pytest.param("schema_before", id="scope-before"),
        pytest.param("schema_middle", id="scope-middle"),
    ],
)
def test_schema_scope_override(module_name: str) -> None:
    """Test schema scope is working properly no matter where the override is declared."""
    query_class = import_module(f"tests.unit.schemas.scope.{module_name}").Query

    schema = strawberry.Schema(query=query_class, scalar_overrides=SCALAR_OVERRIDES)
    schemas_str = textwrap.dedent(str(schema)).strip()

    assert "GroupType" not in schemas_str
    assert "GraphQLGroup" in schemas_str


def test_json_column_class_body_resolver_executes() -> None:
    """The overridden `dict_col` resolver runs and returns its own value, not the JSON projection (#161)."""
    from tests.unit.schemas.resolver.class_body_resolver_override import ExecutableQuery

    schema = strawberry.Schema(query=ExecutableQuery, scalar_overrides=SCALAR_OVERRIDES)
    result = schema.execute_sync("{ overriddenJson { dictCol } }")
    assert not result.errors
    assert result.data == {"overriddenJson": {"dictCol": "OVERRIDE"}}


def test_exclude_relationships_avoids_stub_collision(strawchemy: Strawchemy) -> None:
    """Test that exclude=[RELATIONSHIPS] walks no relationships, so a later explicit type for a related model does not collide with a pre-registered walker stub (#162)."""

    # First slice scopes Fruit with a relationship-free walk...
    @strawchemy.type(Fruit, exclude=[RELATIONSHIPS])
    class FruitNode:
        pass

    # ...so a later explicit Color type must NOT collide with a walker stub.
    @strawchemy.type(Color, include=[SCALARS])
    class ColorNode:
        pass

    fruit_fields = set(DTOInspect(FruitNode).annotations())
    assert "color" not in fruit_fields
    assert {"id", "name", "sweetness", "color_id"} <= fruit_fields


def test_default_order_by_on_non_list_field_raises() -> None:
    with pytest.raises(StrawchemyFieldError, match="list field"):
        import_module("tests.unit.schemas.default_order_by_non_list")


def test_default_order_by_wrong_model_column_raises() -> None:
    with pytest.raises(StrawchemyFieldError, match="not a column"):
        import_module("tests.unit.schemas.default_order_by_invalid")


def test_validation_for_another_model_raises() -> None:
    """Test that a validation type mapping a different model than the input type raises."""
    with pytest.raises(StrawchemyFieldError, match="GroupCreateValidation validates Group, but UserCreate maps User"):
        import_module("tests.unit.schemas.pydantic.validation_model_mismatch")


def test_aggregation_order_by_aliased_column_no_key_error() -> None:
    """Schema build must not raise KeyError for aggregatable columns with a field-level alias.

    Regression test for the bug introduced by the "centralize field_map" refactor.
    `_order_by_aggregation_fields` was keying `model_fields` by `prop.key` (raw SQLAlchemy
    attribute name) but looking up by `name.field_definition.name` (alias-aware). For any
    aggregatable column with a Purpose.READ alias the subscript raised a KeyError.
    """
    from tests.unit.schemas.order.order_by_aliased_aggregation import Query

    # Schema build must not raise KeyError.
    schema = strawberry.Schema(query=Query, scalar_overrides=SCALAR_OVERRIDES)
    schema_sdl = str(schema)
    # The aggregate order-by input type for the aliased model must appear in the schema.
    assert "AliasedItemAggregateOrderBy" in schema_sdl
