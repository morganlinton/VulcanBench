from __future__ import annotations

from uuid import uuid4

import pytest

from strawchemy import Strawchemy
from strawchemy.schema.mutation import Input
from tests.unit.dc_models import ColorDataclass, FruitDataclass
from tests.unit.models import Color, Fruit


@pytest.mark.parametrize(("color_model", "fruit_model"), [(Color, Fruit), (ColorDataclass, FruitDataclass)])
def test_add_non_input_relationships(
    color_model: type[Color | ColorDataclass], fruit_model: type[Fruit | FruitDataclass]
) -> None:
    strawchemy = Strawchemy("postgresql")

    @strawchemy.create_input(color_model, include="all")
    class ColorInput: ...

    color = ColorInput(name="Blue")  # ty: ignore[unknown-argument]
    color_input = Input(color)
    assert len(color_input.relations) == 0
    color_input.instances[0].fruits.append(fruit_model(name="Apple", color_id=uuid4(), sweetness=1, color=None))
    color_input.add_non_input_relations()
    assert len(color_input.relations) == 1
