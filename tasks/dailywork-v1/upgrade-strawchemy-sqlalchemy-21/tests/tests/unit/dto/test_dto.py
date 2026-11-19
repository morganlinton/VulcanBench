from __future__ import annotations

import re
from typing import Any, Optional, get_args
from uuid import UUID, uuid4

import pytest
from typing_extensions import Self

from strawchemy import ALL, RELATIONSHIPS, SCALARS, StrawchemyConfig
from strawchemy.dto import DTOConfig, Purpose, PurposeConfig, config, field
from strawchemy.dto.constants import DTO_INFO_KEY
from strawchemy.dto.types import FieldSpec
from strawchemy.dto.utils import DTOFieldConfig, read_all_config, write_all_config
from strawchemy.exceptions import EmptyDTOError
from tests.typing import AnyFactory, MappedPydanticFactory
from tests.unit.dc_models import (
    AdminDataclass,
    ColorDataclass,
    FruitDataclass,
    SponsoredUserDataclass,
    TomatoDataclass,
    UserWithGreetingDataclass,
)
from tests.unit.models import Admin, Book, Color, Fruit, SponsoredUser, Tag, Tomato, UserWithGreeting
from tests.utils import DTOInspect, factory_iterator


@pytest.mark.parametrize("model", [Tomato, TomatoDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_config_function_produces_same_default(factory: AnyFactory, model: type[Tomato | TomatoDataclass]) -> None:
    """Test that config() and DTOConfig produce DTOs with identical fields."""
    from_function = factory.factory(model, config(Purpose.READ, include="all"), name="FromFunction")
    from_class = factory.factory(model, DTOConfig(Purpose.READ, include="all"), name="FromClass")

    assert DTOInspect(from_function).annotations() == DTOInspect(from_class).annotations()


def test_default_field_config() -> None:
    assert field()[DTO_INFO_KEY] == DTOFieldConfig(
        purposes={Purpose.READ, Purpose.WRITE}, configs={}, default_config=PurposeConfig()
    )


@pytest.mark.parametrize("model", [Tomato, TomatoDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_base_annotations_include(factory: AnyFactory, model: type[Tomato | TomatoDataclass]) -> None:
    class Base:
        name: str

    config = DTOConfig(Purpose.READ).with_base_annotations(Base)
    dto = factory.factory(model, config)

    assert DTOInspect(dto).annotations() == {"name": str}


@pytest.mark.parametrize("model", [Tomato, TomatoDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_base_annotations_include_override(factory: AnyFactory, model: type[Tomato | TomatoDataclass]) -> None:
    class Base:
        name: int

    config = DTOConfig(Purpose.READ, include={"name"}).with_base_annotations(Base)
    dto = factory.factory(model, config)

    assert DTOInspect(dto).annotations() == {"name": int}


@pytest.mark.parametrize("model", [Tomato, TomatoDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_base_annotations_exclude_override(factory: AnyFactory, model: type[Tomato | TomatoDataclass]) -> None:
    class Base:
        name: str

    config = DTOConfig(Purpose.READ, exclude={"name"}).with_base_annotations(Base)
    dto = factory.factory(model, config)

    assert DTOInspect(dto).annotations() == {
        "name": str,
        "id": UUID,
        "popularity": float,
        "sweetness": float,
        "weight": float,
    }


@pytest.mark.parametrize("models", [(Fruit, Color), (FruitDataclass, ColorDataclass)])
@pytest.mark.parametrize("factory", factory_iterator())
def test_to_mapped(
    factory: AnyFactory, models: tuple[type[Fruit | FruitDataclass], type[Color | ColorDataclass]]
) -> None:
    fruit_model, color_model = models
    fruit_dto = factory.factory(fruit_model, read_all_config)
    color_dto = factory.factory(color_model, read_all_config)
    fruit_uuid, color_uuid = uuid4(), uuid4()
    dto_instance = fruit_dto(
        **{  # noqa: PIE804
            "name": "foo",
            "id": fruit_uuid,
            "color": color_dto(id=color_uuid, name="red", fruits=[]),  # ty: ignore[pydantic-discarded-extra-argument]
            "color_id": color_uuid,
            "sweetness": 1,
        }
    )
    instance = dto_instance.to_mapped()
    # Test fruit
    assert isinstance(instance, fruit_model)
    assert instance.id == fruit_uuid
    assert instance.name == "foo"
    # Test color
    assert isinstance(instance.color, color_model)
    assert instance.color.id == color_uuid
    assert instance.color.name == "red"


@pytest.mark.parametrize("model", [Color, ColorDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_to_mapped_override(factory: AnyFactory, model: type[Color | ColorDataclass]) -> None:
    fruit_dto = factory.factory(model, read_all_config)
    uuid = uuid4()
    dto_instance = fruit_dto(**{"id": uuid, "name": "Green", "fruits": []})  # noqa: PIE804
    instance = dto_instance.to_mapped(override={"name": "Red"})
    assert instance.name == "Red"


@pytest.mark.parametrize("model", [Color, ColorDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_to_mapped_override_excluded(factory: AnyFactory, model: type[Color | ColorDataclass]) -> None:
    fruit_dto = factory.factory(model, DTOConfig(Purpose.READ, exclude={"name"}))
    uuid = uuid4()
    dto_instance = fruit_dto(**{"id": uuid, "fruits": []})  # noqa: PIE804
    instance = dto_instance.to_mapped(override={"name": "Red"})
    assert instance.name == "Red"


@pytest.mark.parametrize("model", [Admin, AdminDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_default_read_write(factory: AnyFactory, model: type[Admin | AdminDataclass]) -> None:
    write_dto = factory.factory(model, write_all_config)
    read_dto = factory.factory(model, read_all_config)
    assert DTOInspect(write_dto).has_init_field("name")
    assert DTOInspect(read_dto).has_init_field("name")


@pytest.mark.parametrize("model", [Admin, AdminDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_write_only_field(factory: AnyFactory, model: type[Admin | AdminDataclass]) -> None:
    write_dto = factory.factory(model, write_all_config)
    read_dto = factory.factory(model, read_all_config)
    assert DTOInspect(write_dto).has_init_field("password")
    assert not DTOInspect(read_dto).has_init_field("password")


@pytest.mark.parametrize("factory", factory_iterator())
def test_read_only_field(factory: AnyFactory) -> None:
    read_dto = factory.factory(Book, read_all_config)
    write_dto = factory.factory(Book, write_all_config)
    assert DTOInspect(read_dto).has_init_field("isbn")
    assert not DTOInspect(write_dto).has_init_field("isbn")


@pytest.mark.parametrize("model", [Admin, AdminDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_private_field(factory: AnyFactory, model: type[Admin | AdminDataclass]) -> None:
    read_dto = factory.factory(model, read_all_config)
    write_dto = factory.factory(model, write_all_config)
    assert not DTOInspect(read_dto).has_init_field("private")
    assert not DTOInspect(write_dto).has_init_field("private")


@pytest.mark.parametrize("model", [Tomato, TomatoDataclass])
@pytest.mark.parametrize("config", [write_all_config, read_all_config])
@pytest.mark.parametrize("factory", factory_iterator())
def test_model_field_config(factory: AnyFactory, config: DTOConfig, model: type[Tomato | TomatoDataclass]) -> None:
    tomato_dto = factory.factory(model, config)

    if config.purpose is Purpose.WRITE:
        assert DTOInspect(tomato_dto).has_init_field("sugarness")
        assert DTOInspect(tomato_dto).field_type("weight") is int
        assert DTOInspect(tomato_dto).field_type("popularity") == Optional[float]
    else:
        assert not DTOInspect(tomato_dto).has_init_field("sugarness")
        assert DTOInspect(tomato_dto).field_type("weight") is float
        assert DTOInspect(tomato_dto).field_type("popularity") is float


@pytest.mark.parametrize("model", [Tomato, TomatoDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_field_validator(factory: AnyFactory, model: type[Tomato | TomatoDataclass]) -> None:
    tomato_dto = factory.factory(model, write_all_config)

    with pytest.raises(ValueError, match=re.escape("We do not allow rotten tomato.")):
        tomato_dto(name="rotten", weight=1, sugarness=1, popularity=1)  # ty: ignore[pydantic-discarded-extra-argument]


@pytest.mark.parametrize("model", [Tomato, TomatoDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_field_alias(factory: AnyFactory, model: type[Tomato | TomatoDataclass]) -> None:
    tomato_dto = factory.factory(model, write_all_config)

    tomato = tomato_dto(name="good", weight=1, sugarness=1.25, popularity=1)  # ty: ignore[pydantic-discarded-extra-argument]

    assert tomato.sugarness == 1.25  # ty: ignore[unresolved-attribute]
    assert tomato.to_mapped().sweetness == 1.25


@pytest.mark.parametrize("model", [UserWithGreeting, UserWithGreetingDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_hybrid_property_excluded(
    factory: AnyFactory, model: type[UserWithGreeting | UserWithGreetingDataclass]
) -> None:
    user_dto = factory.factory(model, DTOConfig(Purpose.READ, include={"name", "greeting_hybrid_property"}))
    assert DTOInspect(user_dto).has_init_field("name")
    assert not DTOInspect(user_dto).has_init_field("greeting_hybrid_property")


@pytest.mark.parametrize("model", [UserWithGreeting, UserWithGreetingDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_column_property(factory: AnyFactory, model: type[UserWithGreeting | UserWithGreetingDataclass]) -> None:
    user_dto = factory.factory(model, DTOConfig(Purpose.READ, include={"greeting_column_property"}))
    assert DTOInspect(user_dto).has_init_field("greeting_column_property")


@pytest.mark.parametrize("model", [SponsoredUser, SponsoredUserDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_self_reference(factory: AnyFactory, model: type[SponsoredUser | SponsoredUserDataclass]) -> None:
    user_dto = factory.factory(model, read_all_config)
    assert DTOInspect(user_dto).field_type("sponsor") == Optional[Self]  # ty: ignore[invalid-type-form]
    assert DTOInspect(user_dto).field_type("sponsored") == list[Self]  # ty: ignore[invalid-type-form]


@pytest.mark.parametrize("name", ["SomeTag", None])
def test_forward_refs_resolved(name: str, sqlalchemy_pydantic_factory: MappedPydanticFactory) -> None:
    tag_dto = sqlalchemy_pydantic_factory.factory(Tag, read_all_config, name=name)
    tag_dto.model_validate(
        {
            "id": uuid4(),
            "name": "tag 1",
            "groups": [
                {
                    "id": uuid4(),
                    "tag_id": uuid4(),
                    "tag": {
                        "id": uuid4(),
                        "name": "group tag",
                        "groups": [
                            {
                                "id": uuid4(),
                                "name": "another group",
                                "tag_id": uuid4(),
                                "color_id": uuid4(),
                                "color": {"id": uuid4(), "name": "red", "fruits": []},
                                "tag": {"id": uuid4(), "name": "group tag", "groups": []},
                                "users": [],
                            }
                        ],
                    },
                    "name": "group 1",
                    "color_id": uuid4(),
                    "color": {
                        "id": uuid4(),
                        "name": "red",
                        "fruits": [
                            {
                                "id": uuid4(),
                                "name": "Banana",
                                "color_id": uuid4(),
                                "sweetness": 2.0,
                                "color": {"id": uuid4(), "name": "Yellow", "fruits": []},
                            }
                        ],
                    },
                    "users": [],
                }
            ],
        }
    )


@pytest.mark.parametrize("model", [Fruit, FruitDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_from_include_empty_raises(factory: AnyFactory, model: type[Fruit | FruitDataclass]) -> None:
    """Test that from_include(None) produces a DTO with no fields."""
    with pytest.raises(EmptyDTOError):
        factory.factory(model, DTOConfig.from_include(None), if_no_fields="raise")


@pytest.mark.parametrize(
    ("include_spec", "expected_fields"),
    [
        pytest.param("all", {"name", "color_id", "sweetness", "id", "color"}, id="all"),
        pytest.param(["name", "sweetness"], {"name", "sweetness"}, id="list"),
        pytest.param({"name", "sweetness"}, {"name", "sweetness"}, id="set"),
    ],
)
@pytest.mark.parametrize("model", [Fruit, FruitDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_from_include_spec(
    factory: AnyFactory,
    model: type[Fruit | FruitDataclass],
    include_spec: Any,
    expected_fields: set[str],
) -> None:
    """Test that from_include() accepts 'all' and field-name collections."""
    dto = factory.factory(model, DTOConfig.from_include(include_spec))
    assert set(DTOInspect(dto).annotations()) == expected_fields


@pytest.mark.parametrize("factory", factory_iterator())
def test_from_include_with_custom_purpose(factory: AnyFactory) -> None:
    """Test that from_include() honors the purpose: read-only fields are dropped from write DTOs."""
    dto = factory.factory(Book, DTOConfig.from_include(["title", "isbn"], purpose=Purpose.WRITE))
    assert set(DTOInspect(dto).annotations()) == {"title"}


@pytest.mark.parametrize("model", [Fruit, FruitDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_named_exclude_with_include_all(factory: AnyFactory, model: type[Fruit | FruitDataclass]) -> None:
    """Test that excluded fields are dropped from the DTO even when include='all'."""
    dto = factory.factory(model, DTOConfig(Purpose.READ, include="all", exclude={"color", "color_id"}))
    assert set(DTOInspect(dto).annotations()) == {"name", "sweetness", "id"}


@pytest.mark.parametrize(
    "include_spec",
    [pytest.param("scalars", id="direct-string"), pytest.param([SCALARS], id="constant-list")],
)
@pytest.mark.parametrize("model", [Fruit, FruitDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_scalars_include_allows_exclude(
    factory: AnyFactory, model: type[Fruit | FruitDataclass], include_spec: FieldSpec
) -> None:
    """Test that a group-bearing include coexists with exclude and is not clobbered to 'all'."""
    dto = factory.factory(model, DTOConfig(Purpose.READ, include=include_spec, exclude=["name"]))
    assert set(DTOInspect(dto).annotations()) == {"id", "color_id", "sweetness"}


@pytest.mark.parametrize("model", [Fruit, FruitDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_plain_include_with_exclude(factory: AnyFactory, model: type[Fruit | FruitDataclass]) -> None:
    """Test that a plain field-name include and exclude combine (exclude wins) and warn on overlap."""
    msg = "both explicitly included and excluded"
    with pytest.warns(UserWarning, match=msg):
        dto = factory.factory(model, DTOConfig(Purpose.READ, include=["name", "sweetness"], exclude=["sweetness"]))
    assert set(DTOInspect(dto).annotations()) == {"name"}
    with pytest.warns(UserWarning, match=msg):
        StrawchemyConfig(dialect="postgresql", include=["name", "sweetness"], exclude=["sweetness"])


@pytest.mark.parametrize("model", [Fruit, FruitDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_bare_exclude_still_implies_all(factory: AnyFactory, model: type[Fruit | FruitDataclass]) -> None:
    """Test that a bare exclude (no include) still implies include='all'."""
    dto = factory.factory(model, DTOConfig(Purpose.READ, exclude=["name"]))
    assert set(DTOInspect(dto).annotations()) == {"id", "color_id", "sweetness", "color"}


@pytest.mark.parametrize("model", [Fruit, FruitDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_mixed_group_and_name_include_allows_exclude(factory: AnyFactory, model: type[Fruit | FruitDataclass]) -> None:
    """Test that a group selector mixed with a field name coexists with exclude."""
    dto = factory.factory(model, DTOConfig(Purpose.READ, include=[SCALARS, "color"], exclude=["name"]))
    assert set(DTOInspect(dto).annotations()) == {"id", "color_id", "sweetness", "color"}


@pytest.mark.parametrize("model", [Fruit, FruitDataclass])
@pytest.mark.parametrize("factory", factory_iterator())
def test_global_group_include_allows_global_exclude(factory: AnyFactory, model: type[Fruit | FruitDataclass]) -> None:
    """Test that group-bearing global_include/global_exclude shape nested DTOs."""
    dto = factory.factory(
        model, DTOConfig(Purpose.READ, include=[SCALARS, "color"], global_include=[SCALARS], global_exclude=["name"])
    )
    annotations = DTOInspect(dto).annotations()
    # Root fields follow `include`; global rules don't apply at the root.
    assert set(annotations) == {"id", "name", "color_id", "sweetness", "color"}
    # Nested DTO follows global rules: scalars only, minus the global exclude.
    color_dto = next((arg for arg in get_args(annotations["color"]) if arg is not type(None)), annotations["color"])
    assert set(DTOInspect(color_dto).annotations()) == {"id"}


@pytest.mark.parametrize(
    "include_spec",
    [
        pytest.param("all", id="all-string"),
        pytest.param(ALL, id="bare-constant"),
        pytest.param([ALL], id="constant-list"),
        pytest.param([SCALARS, RELATIONSHIPS], id="both-groups"),
    ],
)
@pytest.mark.parametrize("factory", factory_iterator())
def test_include_all_equivalents(factory: AnyFactory, include_spec: Any) -> None:
    """Test that [ALL] and [SCALARS, RELATIONSHIPS] are equivalent to include='all'."""
    dto = factory.factory(Fruit, DTOConfig(Purpose.READ, include=include_spec))
    assert set(DTOInspect(dto).annotations()) == {"id", "name", "color_id", "sweetness", "color"}


@pytest.mark.parametrize(
    "include_spec",
    [
        pytest.param(("scalars", ()), id="include-scalars-string"),
        pytest.param((SCALARS, ()), id="include-scalars-bare-constant"),
        pytest.param(([SCALARS], ()), id="include-scalars-list"),
        pytest.param((None, RELATIONSHIPS), id="bare-exclude-relationships-bare-constant"),
        pytest.param(([ALL], [RELATIONSHIPS]), id="exclude-relationships-constant-list"),
        pytest.param(("all", "relationships"), id="exclude-relationships-string"),
        pytest.param((None, "relationships"), id="bare-exclude-relationships-string"),
        pytest.param((None, [RELATIONSHIPS]), id="bare-exclude-relationships-constant-list"),
    ],
)
@pytest.mark.parametrize("factory", factory_iterator())
def test_include_scalars(factory: AnyFactory, include_spec: tuple[FieldSpec | None, FieldSpec]) -> None:
    """Test that scalars group selectors keep scalar fields and drop relationships."""
    include, exclude = include_spec
    dto = factory.factory(Fruit, DTOConfig(Purpose.READ, include=include, exclude=exclude))
    assert set(DTOInspect(dto).annotations()) == {"id", "name", "color_id", "sweetness"}


@pytest.mark.parametrize(
    "include_spec",
    [
        pytest.param(("relationships", ()), id="include-relationships-string"),
        pytest.param((RELATIONSHIPS, ()), id="include-relationships-bare-constant"),
        pytest.param(([RELATIONSHIPS], ()), id="include-relationships-list"),
        pytest.param((None, SCALARS), id="bare-exclude-scalars-bare-constant"),
        pytest.param(([ALL], [SCALARS]), id="exclude-scalars-constant-list"),
        pytest.param(("all", "scalars"), id="exclude-scalars-string"),
        pytest.param((None, "scalars"), id="bare-exclude-scalars-string"),
        pytest.param((None, [SCALARS]), id="bare-exclude-scalars-constant-list"),
    ],
)
@pytest.mark.parametrize("factory", factory_iterator())
def test_include_relationships(factory: AnyFactory, include_spec: tuple[FieldSpec | None, FieldSpec]) -> None:
    """Test that relationships group selectors keep only relationships and drop scalars."""
    include, exclude = include_spec
    dto = factory.factory(Fruit, DTOConfig(Purpose.READ, include=include, exclude=exclude))
    assert set(DTOInspect(dto).annotations()) == {"color"}


@pytest.mark.parametrize(
    "include_spec",
    [
        pytest.param(["scalars"], id="scalars-list"),
        pytest.param(["relationships"], id="relationships-list"),
        pytest.param({"all"}, id="all-set"),
    ],
)
@pytest.mark.parametrize("factory", factory_iterator())
def test_group_string_in_iterable_is_field_name(factory: AnyFactory, include_spec: Any) -> None:
    """Test that group string literals inside iterables are treated as field names, not group selectors."""
    with pytest.raises(EmptyDTOError):
        factory.factory(Fruit, DTOConfig(Purpose.READ, include=include_spec), if_no_fields="raise")


@pytest.mark.parametrize("factory", factory_iterator())
def test_exclude_relationships_plus_named_scalar(factory: AnyFactory) -> None:
    """Test that exclude=[RELATIONSHIPS, 'sweetness'] drops relationships and the named scalar."""
    dto = factory.factory(Fruit, DTOConfig(Purpose.READ, exclude=[RELATIONSHIPS, "sweetness"]))
    assert set(DTOInspect(dto).annotations()) == {"id", "name", "color_id"}


@pytest.mark.parametrize("factory", factory_iterator())
def test_include_plain_names_with_relationship(factory: AnyFactory) -> None:
    """Test that plain field-name includes select scalars and relations by name, without groups."""
    dto = factory.factory(Fruit, DTOConfig(Purpose.READ, include=frozenset(["name", "color"])))
    assert set(DTOInspect(dto).annotations()) == {"name", "color"}
