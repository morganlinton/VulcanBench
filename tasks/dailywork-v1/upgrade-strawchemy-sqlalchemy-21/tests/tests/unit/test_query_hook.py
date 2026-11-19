from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any
from unittest.mock import MagicMock

import pytest
import strawberry
from sqlalchemy import ForeignKey, select
from sqlalchemy.ext.associationproxy import AssociationProxy, association_proxy
from sqlalchemy.ext.hybrid import hybrid_property
from sqlalchemy.orm import DeclarativeBase, Load, Mapped, composite, mapped_column, relationship, synonym
from sqlalchemy.orm.util import AliasedClass
from strawberry.types import get_object_definition

from strawchemy import QueryHook, Strawchemy, StrawchemySyncRepository
from strawchemy.exceptions import QueryHookError
from tests.unit.models import Color, Fruit, Group, User, UserWithGreeting

if TYPE_CHECKING:
    from collections.abc import Sequence

    from strawchemy.transpiler.hook import LoadType

_WRONG_KEY_MESSAGE = re.escape("Keys of mappings passed in `load` param must be relationship attributes: ")
_NOT_LOADABLE_MESSAGE = re.escape("Attributes passed in `load` param must be column or relationship attributes: ")


def _wrong_model_message(relationship: object, model: str, attribute: object) -> str:
    return re.escape(f"Attributes nested under {relationship} in `load` param must belong to {model}: {attribute}")


def _foreign_attribute_message(model: str, attribute: object) -> str:
    return re.escape(f"Attributes passed in `load` param must belong to {model}: {attribute}")


@dataclass
class _Point:
    x: int
    y: int


@dataclass
class _OwnerRef:
    owner_id: int
    owner: object


class _Base(DeclarativeBase): ...


class _Owner(_Base):
    __tablename__ = "query_hook_owner"

    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str]
    x: Mapped[int]
    y: Mapped[int]
    animals: Mapped[list[_Animal]] = relationship("_Animal", back_populates="owner")
    dogs: Mapped[list[_Dog]] = relationship("_Dog", viewonly=True)
    animal_names: AssociationProxy[list[str]] = association_proxy("animals", "name")
    alias: Mapped[str] = synonym("name")
    pets: Mapped[list[_Animal]] = synonym("animals")
    point: Mapped[_Point] = composite(_Point, "x", "y")

    @hybrid_property
    def display_name(self) -> str:
        return self.name

    @hybrid_property
    def exclaimed_name(self) -> str:
        return self.name + "!"

    @hybrid_property
    def first_animals(self) -> list[_Animal]:
        return self.animals


class _Animal(_Base):
    __tablename__ = "query_hook_animal"

    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str]
    kind: Mapped[str]
    owner_id: Mapped[int] = mapped_column(ForeignKey("query_hook_owner.id"))
    owner: Mapped[_Owner] = relationship(_Owner, back_populates="animals")
    owner_ref: Mapped[_OwnerRef] = composite(_OwnerRef, "owner_id", "owner")

    __mapper_args__ = {"polymorphic_on": "kind", "polymorphic_identity": "animal"}  # noqa: RUF012


class _Dog(_Animal):
    tricks: Mapped[int | None]

    __mapper_args__ = {"polymorphic_identity": "dog"}  # noqa: RUF012


class _ClassLoadHook(QueryHook[Fruit]):
    load: Sequence[LoadType] = [Fruit.name, Fruit.color]


class _NestedClassLoadHook(QueryHook[Color]):
    load: Sequence[LoadType] = [Color.name, (Color.fruits, [Fruit.name, (Fruit.color, [Color.name])])]


class _InheritedClassLoadHook(_ClassLoadHook): ...


class _OverridingClassLoadHook(_ClassLoadHook):
    load: Sequence[LoadType] = [Fruit.sweetness]


@dataclass
class _DataclassLoadHook(QueryHook[Fruit]):
    load: Sequence[LoadType] = field(default_factory=lambda: [Fruit.name])


_strawchemy = Strawchemy("postgresql")


@_strawchemy.type(Fruit, include="all")
class _FruitType: ...


@_strawchemy.type(Fruit, include="all", query_hook=QueryHook(load=[Fruit.name]))
class _HookedFruitType: ...


@_strawchemy.aggregate(Fruit, include="all")
class _FruitAggregationType: ...


def _resolved_load(hook: QueryHook[Any]) -> tuple[list[Any], list[Any]]:
    return hook._columns, hook._relationships  # noqa: SLF001


def test_default_load_is_empty() -> None:
    """Test that a hook built without ``load`` loads nothing."""
    hook = QueryHook()
    assert hook.load == []
    assert _resolved_load(hook) == ([], [])


def test_instance_load() -> None:
    """Test that ``load`` passed at instantiation splits into columns and relationships."""
    hook = QueryHook(load=[Fruit.name, Fruit.color])
    assert hook.load == [Fruit.name, Fruit.color]
    assert _resolved_load(hook) == ([Fruit.name], [(Fruit.color, [])])


def test_class_load() -> None:
    """Test that a subclass setting ``load`` as a class attribute loads it."""
    hook = _ClassLoadHook()
    assert hook.load == [Fruit.name, Fruit.color]
    assert _resolved_load(hook) == ([Fruit.name], [(Fruit.color, [])])


def test_class_load_nested_relationship() -> None:
    """Test that a class-level ``load`` keeps nested relationship specs."""
    hook = _NestedClassLoadHook()
    assert _resolved_load(hook) == ([Color.name], [(Color.fruits, [Fruit.name, (Fruit.color, [Color.name])])])


def test_instance_load_overrides_class_load() -> None:
    """Test that ``load`` passed at instantiation wins over the class attribute."""
    hook = _ClassLoadHook(load=[Fruit.sweetness])
    assert hook.load == [Fruit.sweetness]
    assert _resolved_load(hook) == ([Fruit.sweetness], [])


def test_empty_instance_load_overrides_class_load() -> None:
    """Test that an explicit empty ``load`` wins over the class attribute."""
    hook = _ClassLoadHook(load=[])
    assert hook.load == []
    assert _resolved_load(hook) == ([], [])


def test_class_load_is_not_shared_between_instances() -> None:
    """Test that instances of a class-level ``load`` hook get their own lists."""
    first, second = _ClassLoadHook(), _ClassLoadHook()
    assert first.load is not second.load
    assert first.load is not _ClassLoadHook.load
    assert all(a is not b for a, b in zip(_resolved_load(first), _resolved_load(second), strict=True))


@pytest.mark.parametrize(
    ("hook_class", "columns", "relationships"),
    [
        pytest.param(_InheritedClassLoadHook, [Fruit.name], [(Fruit.color, [])], id="inherited"),
        pytest.param(_OverridingClassLoadHook, [Fruit.sweetness], [], id="overridden"),
    ],
)
def test_class_load_on_subclass_of_subclass(
    hook_class: type[QueryHook[Fruit]], columns: list[object], relationships: list[object]
) -> None:
    """Test that a subclass of a subclass inherits or overrides the class-level ``load``."""
    hook = hook_class()
    assert _resolved_load(hook) == (columns, relationships)


def test_dataclass_subclass_load_field() -> None:
    """Test that a dataclass subclass redeclaring ``load`` as a field uses its default."""
    hook = _DataclassLoadHook()
    assert hook.load == [Fruit.name]
    assert _resolved_load(hook) == ([Fruit.name], [])
    assert _resolved_load(_DataclassLoadHook(load=[Fruit.sweetness])) == ([Fruit.sweetness], [])


def test_class_load_wrong_relationship_load_spec() -> None:
    """Test that a class-level ``load`` keyed by a column raises ``QueryHookError``."""

    class WrongLoadHook(QueryHook[Fruit]):
        load: Sequence[LoadType] = [(Fruit.name, [Fruit.id])]

    with pytest.raises(QueryHookError, match=_WRONG_KEY_MESSAGE):
        WrongLoadHook()


@pytest.mark.parametrize(
    "load",
    [
        pytest.param([(Fruit.name, [])], id="empty-list-key"),
        pytest.param([(Color.fruits, [(Fruit.name, [Fruit.id])])], id="nested-key"),
        pytest.param([(Color.fruits, [(Fruit.name, [])])], id="nested-empty-list-key"),
        pytest.param([(Color.fruits, [Fruit.name, (Fruit.color, [(Color.name, [Color.id])])])], id="deeply-nested-key"),
    ],
)
def test_wrong_relationship_load_spec(load: Sequence[LoadType]) -> None:
    """Test that a load spec keyed by a column at any depth raises ``QueryHookError``."""
    with pytest.raises(QueryHookError, match=_WRONG_KEY_MESSAGE):
        QueryHook(load=load)


def test_class_load_wrong_nested_relationship_load_spec() -> None:
    """Test that a class-level ``load`` with a nested key being a column raises ``QueryHookError``."""

    class WrongNestedLoadHook(QueryHook[Color]):
        load: Sequence[LoadType] = [(Color.fruits, [(Fruit.name, [Fruit.id])])]

    with pytest.raises(QueryHookError, match=_WRONG_KEY_MESSAGE):
        WrongNestedLoadHook()


@pytest.mark.parametrize(
    "load",
    [
        pytest.param([(Color.fruits, [])], id="empty-list"),
        pytest.param([(Color.fruits, [(Fruit.color, [])])], id="nested-empty-list"),
        pytest.param([(Color.fruits, [Fruit.name, (Fruit.color, [Color.name])])], id="nested"),
        pytest.param([(Color.fruits, [(Fruit.color, [(Color.fruits, [Fruit.name])])])], id="deeply-nested"),
    ],
)
def test_valid_relationship_load_spec(load: Sequence[LoadType]) -> None:
    """Test that load specs keyed by relationships at every depth are accepted."""
    assert _resolved_load(QueryHook(load=load)) == ([], load)


@pytest.mark.parametrize(
    ("load", "normalized"),
    [
        pytest.param([(Color.fruits, [Fruit.color])], [(Color.fruits, [(Fruit.color, [])])], id="nested"),
        pytest.param(
            [(Color.fruits, [Fruit.name, Fruit.color])],
            [(Color.fruits, [Fruit.name, (Fruit.color, [])])],
            id="nested-with-column",
        ),
        pytest.param(
            [(Color.fruits, [(Fruit.color, [Color.fruits])])],
            [(Color.fruits, [(Fruit.color, [(Color.fruits, [])])])],
            id="deeply-nested",
        ),
    ],
)
def test_nested_bare_relationship_is_normalized(load: Sequence[LoadType], normalized: list[LoadType]) -> None:
    """Test that a nested bare relationship loads as a relationship with an empty list, at any depth."""
    assert _resolved_load(QueryHook(load=load)) == ([], normalized)


def test_class_load_nested_bare_relationship_is_normalized() -> None:
    """Test that a class-level ``load`` with a nested bare relationship normalizes it."""

    class NestedBareLoadHook(QueryHook[Color]):
        load: Sequence[LoadType] = [(Color.fruits, [Fruit.color])]

    assert _resolved_load(NestedBareLoadHook()) == ([], [(Color.fruits, [(Fruit.color, [])])])


def test_nested_bare_relationship_loads_as_relationship() -> None:
    """Test that the loader option of a nested bare relationship loads it rather than a column of it."""
    option = QueryHook(load=[(Color.fruits, [Fruit.color])]).load_relationships(AliasedClass(Color))[0]
    assert isinstance(option, Load)
    load = option.context[-1]
    assert str(load.path).endswith("Fruit.color -> Mapper[Color(color)]]")
    assert load.strategy == (("lazy", "joined"),)


@pytest.mark.parametrize(
    ("load", "relationship", "model", "attribute"),
    [
        pytest.param([(Color.fruits, [User.name])], Color.fruits, "Fruit", User.name, id="column"),
        pytest.param([(Color.fruits, [User.group])], Color.fruits, "Fruit", User.group, id="relationship"),
        pytest.param(
            [(Color.fruits, [(User.group, [Group.name])])], Color.fruits, "Fruit", User.group, id="relationship-key"
        ),
        pytest.param(
            [(Color.fruits, [(Fruit.color, [User.name])])], Fruit.color, "Color", User.name, id="deeply-nested-column"
        ),
        pytest.param(
            [(Color.fruits, [(Fruit.color, [(Color.fruits, [Fruit.name, User.name])])])],
            Color.fruits,
            "Fruit",
            User.name,
            id="deeper-nested-column",
        ),
        pytest.param([(_Owner.animals, [_Dog.tricks])], _Owner.animals, "_Animal", _Dog.tricks, id="subclass-column"),
        pytest.param(
            [(_Animal.owner, [_Dog.owner])], _Animal.owner, "_Owner", _Dog.owner, id="relationship-of-another-model"
        ),
    ],
)
def test_nested_attribute_of_another_model(
    load: Sequence[LoadType], relationship: object, model: str, attribute: object
) -> None:
    """Test that a nested attribute not of its parent relationship's model raises ``QueryHookError``."""
    with pytest.raises(QueryHookError, match=_wrong_model_message(relationship, model, attribute)):
        QueryHook(load=load)


def test_class_load_nested_attribute_of_another_model() -> None:
    """Test that a class-level ``load`` with a nested attribute of another model raises ``QueryHookError``."""

    class WrongModelLoadHook(QueryHook[Color]):
        load: Sequence[LoadType] = [(Color.fruits, [User.name])]

    with pytest.raises(QueryHookError, match=_wrong_model_message(Color.fruits, "Fruit", User.name)):
        WrongModelLoadHook()


@pytest.mark.parametrize(
    "load",
    [
        pytest.param([(_Owner.animals, [_Animal.name, (_Animal.owner, [_Owner.name])])], id="same-model"),
        pytest.param([(_Owner.dogs, [_Dog.tricks, _Dog.name])], id="subclass"),
        pytest.param([(_Owner.dogs, [_Animal.name, (_Animal.owner, [])])], id="base-class"),
    ],
)
def test_nested_attribute_of_inherited_model(load: Sequence[LoadType]) -> None:
    """Test that a nested attribute of the relationship's model or of a model it inherits from is accepted."""
    assert _resolved_load(QueryHook(load=load)) == ([], load)


def test_nested_inherited_attribute_accessed_on_subclass() -> None:
    """Test that a subclass attribute inherited from the relationship's model loads as the model's own."""
    hook = QueryHook(load=[(_Owner.animals, [_Dog.name, (_Dog.owner, [])])])
    assert _resolved_load(hook) == ([], [(_Owner.animals, [_Animal.name, (_Animal.owner, [])])])


@pytest.mark.parametrize(
    ("load", "columns", "relationships"),
    [
        pytest.param([_Owner.alias], [_Owner.name], [], id="synonym"),
        pytest.param([_Owner.pets], [], [(_Owner.animals, [])], id="relationship-synonym"),
        pytest.param([_Owner.display_name], [_Owner.name], [], id="hybrid-property"),
        pytest.param([_Owner.first_animals], [], [(_Owner.animals, [])], id="relationship-hybrid-property"),
        pytest.param([_Owner.point], [_Owner.x, _Owner.y], [], id="composite"),
        pytest.param([_Animal.owner_ref], [_Animal.owner_id], [(_Animal.owner, [])], id="composite-with-relationship"),
        pytest.param(
            [(_Owner.pets, [_Animal.name])], [], [(_Owner.animals, [_Animal.name])], id="relationship-synonym-key"
        ),
        pytest.param(
            [(_Owner.first_animals, [_Animal.name])],
            [],
            [(_Owner.animals, [_Animal.name])],
            id="relationship-hybrid-property-key",
        ),
        pytest.param(
            [(_Animal.owner, [_Owner.alias, _Owner.display_name, _Owner.point, _Owner.pets, _Owner.first_animals])],
            [],
            [
                (
                    _Animal.owner,
                    [_Owner.name, _Owner.name, _Owner.x, _Owner.y, (_Owner.animals, []), (_Owner.animals, [])],
                )
            ],
            id="nested",
        ),
    ],
)
def test_proxied_attribute_loads_its_property(
    load: Sequence[LoadType], columns: list[object], relationships: list[object]
) -> None:
    """Test that synonyms, hybrids of a column or relationship, and composites load what they stand for."""
    assert _resolved_load(QueryHook(load=load)) == (columns, relationships)


def test_proxied_attribute_of_another_model() -> None:
    """Test that a synonym nested under a relationship of another model raises ``QueryHookError``."""
    with pytest.raises(QueryHookError, match=_wrong_model_message(Color.fruits, "Fruit", _Owner.alias)):
        QueryHook(load=[(Color.fruits, [_Owner.alias])])


def test_composite_column_load_options() -> None:
    """Test that a top-level composite loads its columns, as options and as selected columns."""
    hook = QueryHook(load=[_Owner.point])
    alias = AliasedClass(_Owner)
    assert len(hook.column_load_options(alias)) == 2
    statement, _ = hook.load_columns(select(alias), alias, "add")
    assert [column.key for column in statement.selected_columns][-2:] == ["x", "y"]


_NOT_LOADABLE_ATTRIBUTES = [
    pytest.param(UserWithGreeting.greeting_hybrid_property, id="hybrid-property"),
    pytest.param(_Owner.exclaimed_name, id="hybrid-property-expression"),
    pytest.param(_Owner.animal_names, id="association-proxy"),
]


@pytest.mark.parametrize("attribute", _NOT_LOADABLE_ATTRIBUTES)
def test_top_level_attribute_not_loadable(attribute: Any) -> None:
    """Test that a top-level attribute neither a column nor a relationship raises ``QueryHookError``."""
    with pytest.raises(QueryHookError, match=_NOT_LOADABLE_MESSAGE + re.escape(str(attribute))):
        QueryHook(load=[attribute])


@pytest.mark.parametrize("attribute", _NOT_LOADABLE_ATTRIBUTES[1:])
def test_nested_attribute_not_loadable(attribute: Any) -> None:
    """Test that a nested attribute neither a column nor a relationship raises ``QueryHookError``."""
    with pytest.raises(QueryHookError, match=_NOT_LOADABLE_MESSAGE + re.escape(str(attribute))):
        QueryHook(load=[(_Animal.owner, [attribute])])


@pytest.mark.parametrize("attribute", [*_NOT_LOADABLE_ATTRIBUTES[1:], _Owner.alias, _Owner.display_name, _Owner.point])
def test_key_not_loadable(attribute: Any) -> None:
    """Test that a key not loading a relationship raises ``QueryHookError``."""
    with pytest.raises(QueryHookError, match=_WRONG_KEY_MESSAGE + re.escape(str(attribute))):
        QueryHook(load=[(attribute, [])])


@pytest.mark.parametrize("attribute", _NOT_LOADABLE_ATTRIBUTES[:1])
def test_class_load_attribute_not_loadable(attribute: Any) -> None:
    """Test that a class-level ``load`` with an attribute neither a column nor a relationship raises."""

    class NotLoadableHook(QueryHook[UserWithGreeting]):
        load: Sequence[LoadType] = [UserWithGreeting.name, attribute]

    with pytest.raises(QueryHookError, match=_NOT_LOADABLE_MESSAGE):
        NotLoadableHook()


def test_column_property_is_loadable() -> None:
    """Test that a ``column_property`` loads as a column."""
    hook = QueryHook(load=[UserWithGreeting.greeting_column_property])
    assert _resolved_load(hook) == ([UserWithGreeting.greeting_column_property], [])


@pytest.mark.parametrize(
    ("load", "attribute"),
    [
        pytest.param([User.name], User.name, id="column"),
        pytest.param([User.group], User.group, id="relationship"),
        pytest.param([(User.group, [Group.name])], User.group, id="relationship-key"),
        pytest.param([Fruit.name], Fruit.name, id="column-with-a-key-of-the-model"),
        pytest.param([Color.id, Fruit.color], Fruit.color, id="after-an-attribute-of-the-model"),
        pytest.param([_Owner.alias], _Owner.alias, id="synonym"),
    ],
)
def test_top_level_attribute_of_another_model(load: Sequence[LoadType], attribute: object) -> None:
    """Test that a top-level attribute not of the hooked model raises ``QueryHookError``."""
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Color", attribute)):
        QueryHook(load=load).check_model(Color)


def test_top_level_subclass_attribute_on_base_class_model() -> None:
    """Test that a subclass-only attribute on a hook of the base class raises ``QueryHookError``."""
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("_Animal", _Dog.tricks)):
        QueryHook(load=[_Animal.name, _Dog.tricks]).check_model(_Animal)


@pytest.mark.parametrize(
    ("load", "model"),
    [
        pytest.param([Color.name, Color.fruits, (Color.fruits, [Fruit.name])], Color, id="same-model"),
        pytest.param([_Animal.name, _Animal.owner, _Dog.name, _Dog.tricks], _Dog, id="base-class-attribute"),
        pytest.param([_Owner.alias, _Owner.pets, _Owner.point, _Owner.display_name], _Owner, id="proxied-attributes"),
    ],
)
def test_top_level_attribute_of_the_model(load: Sequence[LoadType], model: type[Any]) -> None:
    """Test that top-level attributes of the hooked model or of a model it inherits from are accepted."""
    QueryHook(load=load).check_model(model)


def test_hook_checked_against_several_models() -> None:
    """Test that a hook accepted for one model is still checked against another."""
    hook = QueryHook(load=[Fruit.name])
    hook.check_model(Fruit)
    hook.check_model(Fruit)
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Color", Fruit.name)):
        hook.check_model(Color)


def test_type_hook_of_another_model(strawchemy: Strawchemy) -> None:
    """Test that a type-level hook loading another model's attribute raises when the type is defined."""
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Color", Fruit.name)):

        @strawchemy.type(Color, include="all", query_hook=QueryHook(load=[Fruit.name]))
        class ColorType: ...


def test_type_hook_list_of_another_model(strawchemy: Strawchemy) -> None:
    """Test that each hook of a type-level hook list is checked against the type's model."""
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Color", User.group)):

        @strawchemy.type(Color, include="all", query_hook=[QueryHook(load=[Color.name]), QueryHook(load=[User.group])])
        class ColorType: ...


def test_resolver_field_hook_of_another_model(strawchemy: Strawchemy) -> None:
    """Test that a resolver field hook is checked against the model of the type it belongs to."""
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Color", Fruit.name)):

        @strawchemy.type(Color, include="all")
        class ColorType:
            @strawchemy.field(query_hook=QueryHook(load=[Fruit.name]))
            def label(self) -> str:
                return "label"


def test_column_field_hook_of_another_model(strawchemy: Strawchemy) -> None:
    """Test that a column field hook is checked against the model of the type it belongs to."""
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Color", Fruit.sweetness)):

        @strawchemy.type(Color, include="all")
        class ColorType:
            name: str = strawchemy.field(query_hook=QueryHook(load=[Fruit.sweetness]))


def test_relation_field_hook_of_another_model(strawchemy: Strawchemy) -> None:
    """Test that a relation field hook is checked against the relation's model, not the type's."""
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Fruit", Color.name)):

        @strawchemy.type(Color, include="all")
        class ColorType:
            fruits: list[_FruitType] = strawchemy.field(query_hook=QueryHook(load=[Color.name]))


def test_relation_resolver_field_hook_of_another_model(strawchemy: Strawchemy) -> None:
    """Test that a resolver named after a relation has its hook checked against the type's model."""
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Color", Fruit.sweetness)):

        @strawchemy.type(Color, include="all")
        class ColorType:
            @strawchemy.field(query_hook=QueryHook(load=[Fruit.sweetness]))
            def fruits(self) -> str:
                return "fruits"


def test_field_hooks_of_their_models(strawchemy: Strawchemy) -> None:
    """Test that type and field hooks loading attributes of the model they apply to are accepted."""

    @strawchemy.type(Color, include="all", query_hook=QueryHook(load=[Color.name]))
    class ColorType:
        fruits: list[_HookedFruitType] = strawchemy.field(query_hook=QueryHook(load=[Fruit.sweetness]))

        @strawchemy.field(query_hook=[QueryHook(load=[Color.name]), QueryHook(load=[(Color.fruits, [Fruit.name])])])
        def label(self) -> str:
            return "label"

    assert {field.name for field in get_object_definition(ColorType, strict=True).fields} >= {"fruits", "label"}


def test_root_field_hook_of_another_model() -> None:
    """Test that a root field hook loading another model's attribute raises when its type is resolved."""
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Fruit", Color.name)):

        @strawberry.type
        class Query:
            fruits: list[_FruitType] = _strawchemy.field(query_hook=QueryHook(load=[Color.name]))


def test_root_aggregation_field_hook_of_another_model() -> None:
    """Test that a root aggregation field hook is checked against the aggregated model."""
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Fruit", Color.name)):

        @strawberry.type
        class Query:
            fruits: list[_FruitAggregationType] = _strawchemy.field(
                root_aggregations=True, query_hook=QueryHook(load=[Color.name])
            )


def test_shared_hook_of_another_model(strawchemy: Strawchemy) -> None:
    """Test that a hook shared by a field of its model and a type of another model raises for the latter."""
    hook = QueryHook(load=[Fruit.name])

    @strawberry.type
    class Query:
        fruits: list[_FruitType] = _strawchemy.field(query_hook=hook)

    strawberry.Schema(query=Query)
    with pytest.raises(QueryHookError, match=_foreign_attribute_message("Color", Fruit.name)):

        @strawchemy.type(Color, include="all", query_hook=hook)
        class ColorType: ...


def test_repository_hook_of_another_model() -> None:
    """Test that a hook given to a repository built in a resolver raises when the repository is built."""

    @strawberry.type
    class Query:
        @strawberry.field
        def fruits(self, info: strawberry.Info) -> list[_FruitType]:
            StrawchemySyncRepository(_FruitType, info, session=MagicMock(), query_hook=QueryHook(load=[Color.name]))
            return []

    result = strawberry.Schema(query=Query).execute_sync("{ fruits { id } }")
    assert result.errors
    assert isinstance(result.errors[0].original_error, QueryHookError)
    assert re.fullmatch(_foreign_attribute_message("Fruit", Color.name), result.errors[0].message)
