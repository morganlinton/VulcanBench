from __future__ import annotations

from typing import Annotated, TypeVar, Union, cast
from unittest.mock import Mock

import strawberry
from graphql import GraphQLResolveInfo
from strawberry.types import get_object_definition

from strawchemy import Strawchemy, StrawchemyConfig
from strawchemy.dto.backend.strawberry import StrawberrryDTOBackend
from tests.unit.models import Fruit, Tomato

T = TypeVar("T")


class TestIsTypeOf:
    @classmethod
    def _make(cls, model: type[T]) -> T:
        """Build a bare instance without running the SQLAlchemy constructor.

        is_type_of only performs an isinstance check, so attribute population is
        unnecessary.
        """
        return model.__new__(model)

    @property
    def _resolve_info(self) -> GraphQLResolveInfo:
        """A stand-in info object.

        The generated ``is_type_of`` ignores its info arg (pure isinstance check), so no
        real GraphQL resolution context is needed.
        """
        return cast("GraphQLResolveInfo", Mock(spec=GraphQLResolveInfo))

    def test_backend_receives_flag(self) -> None:
        sc = Strawchemy(StrawchemyConfig("postgresql", auto_is_type_of=False))
        backend = sc.type_factory.backend
        assert isinstance(backend, StrawberrryDTOBackend)
        assert backend.auto_is_type_of is False

        sc_on = Strawchemy(StrawchemyConfig("postgresql"))
        backend_on = sc_on.type_factory.backend
        assert isinstance(backend_on, StrawberrryDTOBackend)
        assert backend_on.auto_is_type_of is True

    def test_generated_matches_model(self, strawchemy: Strawchemy) -> None:
        @strawchemy.type(Fruit, include="all", override=True)
        class FruitType: ...

        obj_def = get_object_definition(FruitType, strict=True)
        assert obj_def.is_type_of is not None
        assert obj_def.is_type_of(self._make(Fruit), self._resolve_info) is True

    def test_generated_rejects_other_model(self, strawchemy: Strawchemy) -> None:
        @strawchemy.type(Fruit, include="all", override=True)
        class FruitType: ...

        obj_def = get_object_definition(FruitType, strict=True)
        assert obj_def.is_type_of is not None
        assert obj_def.is_type_of(self._make(Tomato), self._resolve_info) is False

    def test_generated_accepts_strawberry_instance(self, strawchemy: Strawchemy) -> None:
        @strawchemy.type(Fruit, include="all", override=True)
        class FruitType: ...

        obj_def = get_object_definition(FruitType, strict=True)
        # An instance of the strawberry type itself also resolves (the `cls` arm).
        assert obj_def.is_type_of is not None
        assert obj_def.is_type_of(FruitType.__new__(FruitType), self._resolve_info) is True

    def test_user_defined_preserved(self, strawchemy: Strawchemy) -> None:
        sentinel = object()

        @strawchemy.type(Fruit, include="all", override=True)
        class FruitType:
            @classmethod
            def is_type_of(cls, obj, info):  # noqa: ANN001, ANN206, ARG003
                return sentinel  # type: ignore[return-value]

        obj_def = get_object_definition(FruitType, strict=True)
        assert obj_def.is_type_of is not None
        # The user's implementation is used verbatim, not the generated isinstance one.
        assert obj_def.is_type_of(self._make(Fruit), self._resolve_info) is sentinel

    def test_flag_off_generates_no_is_type_of(self) -> None:
        sc = Strawchemy(StrawchemyConfig("postgresql", auto_is_type_of=False))

        @sc.type(Fruit, include="all", override=True)
        class FruitType: ...

        obj_def = get_object_definition(FruitType, strict=True)
        assert obj_def.is_type_of is None

    def test_input_type_gets_no_is_type_of(self, strawchemy: Strawchemy) -> None:
        @strawchemy.create_input(Fruit, include="all")
        class FruitInput: ...

        obj_def = get_object_definition(FruitInput, strict=True)
        assert obj_def.is_type_of is None

    def test_union_resolves_via_generated_is_type_of(self, strawchemy: Strawchemy) -> None:
        # include only a scalar field to avoid relationship forward references.
        @strawchemy.type(Fruit, include=["name"], override=True)
        class FruitType: ...

        @strawchemy.type(Tomato, include=["name"], override=True)
        class TomatoType: ...

        FruitOrTomato = Annotated[Union[FruitType, TomatoType], strawberry.union("FruitOrTomato")]  # noqa: N806
        _fruit = self._make(Fruit)

        @strawberry.type
        class Query:
            # graphql_type bypasses annotation string resolution caused by
            # `from __future__ import annotations` when the type is defined locally.
            @strawberry.field(graphql_type=FruitOrTomato)
            def thing(self) -> FruitOrTomato:  # type: ignore[return-value]
                return _fruit  # ty: ignore[invalid-return-type]

        schema = strawberry.Schema(query=Query)
        result = schema.execute_sync("{ thing { __typename } }")

        assert result.errors is None
        assert result.data == {"thing": {"__typename": "FruitType"}}
