from __future__ import annotations

from typing import TYPE_CHECKING, Any
from uuid import uuid4

from sqlalchemy import event, inspect

from strawchemy.schema.mutation.input import EventRegistry, RelationInput
from strawchemy.schema.mutation.types import RelationType
from tests.unit.models import Color, Fruit

if TYPE_CHECKING:
    import pytest
    from sqlalchemy.orm import RelationshipProperty


def _fruits_prop() -> RelationshipProperty[Any]:
    """The Color.fruits relationship MapperProperty (to-many)."""
    return inspect(Color).relationships["fruits"]


def _make_relation(registry: EventRegistry, parent: Color) -> RelationInput:
    return RelationInput(
        attribute=_fruits_prop(),
        related=Fruit,
        parent=parent,
        relation_type=RelationType.TO_MANY,
        event_registry=registry,
    )


def _color_prop() -> RelationshipProperty[Any]:
    """The Fruit.color relationship MapperProperty (to-one)."""
    return inspect(Fruit).relationships["color"]


def _make_to_one_relation(registry: EventRegistry, parent: Fruit) -> RelationInput:
    return RelationInput(
        attribute=_color_prop(),
        related=Color,
        parent=parent,
        relation_type=RelationType.TO_ONE,
        event_registry=registry,
    )


def test_append_event_updates_only_owning_relation() -> None:
    """An append on one parent's collection fills only that parent's RelationInput create bucket."""
    registry = EventRegistry()
    parent_a, parent_b = Color(name="A"), Color(name="B")
    relation_a = _make_relation(registry, parent_a)
    relation_b = _make_relation(registry, parent_b)
    fruit_a = Fruit(name="A", color_id=uuid4(), sweetness=1, color=None)
    fruit_b = Fruit(name="B", color_id=uuid4(), sweetness=1, color=None)

    parent_a.fruits.append(fruit_a)
    parent_b.fruits.append(fruit_b)

    assert relation_a.create == [fruit_a]
    assert relation_b.create == [fruit_b]


def test_remove_event_updates_only_owning_relation() -> None:
    """A remove on one parent's collection clears only that parent's RelationInput create bucket."""
    registry = EventRegistry()
    parent_a, parent_b = Color(name="A"), Color(name="B")
    relation_a = _make_relation(registry, parent_a)
    relation_b = _make_relation(registry, parent_b)
    fruit_a = Fruit(name="A", color_id=uuid4(), sweetness=1, color=None)
    fruit_b = Fruit(name="B", color_id=uuid4(), sweetness=1, color=None)
    parent_a.fruits.append(fruit_a)
    parent_b.fruits.append(fruit_b)
    assert relation_a.create == [fruit_a]
    assert relation_b.create == [fruit_b]

    parent_b.fruits.remove(fruit_b)

    assert relation_b.create == []
    assert relation_a.create == [fruit_a]  # owning-relation isolation preserved


def test_set_event_updates_only_owning_relation() -> None:
    """A set on one parent's to-one attribute fills only that parent's RelationInput create bucket."""
    registry = EventRegistry()
    fruit_a = Fruit(name="A", color_id=uuid4(), sweetness=1, color=None)
    fruit_b = Fruit(name="B", color_id=uuid4(), sweetness=1, color=None)
    relation_a = _make_to_one_relation(registry, fruit_a)
    relation_b = _make_to_one_relation(registry, fruit_b)
    color = Color(name="Blue")  # transient

    fruit_b.color = color  # set event -> handle_set -> create == [color]

    assert relation_b.create == [color]
    assert relation_a.create == []


def test_attribute_listener_registered_once(monkeypatch: pytest.MonkeyPatch) -> None:
    """Registering many relations for one attribute wires its listener a single time."""
    import strawchemy.schema.mutation.input as input_module

    real_listens_for = event.listens_for
    append_registrations = 0

    def counting_listens_for(target: Any, identifier: str, *args: Any, **kwargs: Any) -> Any:
        nonlocal append_registrations
        if target is _fruits_prop() and identifier == "append":
            append_registrations += 1
        return real_listens_for(target, identifier, *args, **kwargs)

    monkeypatch.setattr(input_module.event, "listens_for", counting_listens_for)

    registry = EventRegistry()
    _make_relation(registry, Color(name="A"))
    _make_relation(registry, Color(name="B"))
    _make_relation(registry, Color(name="C"))

    assert append_registrations == 1


def test_strawchemy_reuses_one_registry_across_inputs(monkeypatch: pytest.MonkeyPatch) -> None:
    """Two mutation inputs built from one Strawchemy share its registry and wire each attribute once."""
    import strawchemy.schema.mutation.input as input_module
    from strawchemy import Strawchemy
    from strawchemy.schema.mutation import Input

    strawchemy = Strawchemy("postgresql")

    @strawchemy.create_input(Color, include="all")
    class ColorInput: ...

    registry = strawchemy._event_registry  # noqa: SLF001

    real_listens_for = event.listens_for
    append_registrations = 0

    def counting_listens_for(target: Any, identifier: str, *args: Any, **kwargs: Any) -> Any:
        nonlocal append_registrations
        if target is _fruits_prop() and identifier == "append":
            append_registrations += 1
        return real_listens_for(target, identifier, *args, **kwargs)

    monkeypatch.setattr(input_module.event, "listens_for", counting_listens_for)

    for _ in range(2):
        color_input = Input(ColorInput(name="Blue"), registry=registry)  # ty: ignore[unknown-argument]
        color_input.instances[0].fruits.append(Fruit(name="Apple", color_id=uuid4(), sweetness=1, color=None))
        color_input.add_non_input_relations()

    # One wiring total despite two separate Input builds sharing the registry.
    assert append_registrations == 1
