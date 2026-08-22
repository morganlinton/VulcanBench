"""Hidden pass-to-pass guards: ordinary discriminated unions unchanged."""

from __future__ import annotations

from typing import Annotated, Literal, Union

import pytest
from pydantic import BaseModel, Field, ValidationError


class Cat(BaseModel):
    kind: Literal["cat"] = "cat"
    meow: str = "m"


class Dog(BaseModel):
    kind: Literal["dog"] = "dog"
    bark: str = "b"


class Pet(BaseModel):
    pet: Annotated[Union[Cat, Dog], Field(discriminator="kind")]


def test_literal_discriminated_union_routes():
    assert isinstance(Pet.model_validate({"pet": {"kind": "cat"}}).pet, Cat)
    assert isinstance(Pet.model_validate({"pet": {"kind": "dog"}}).pet, Dog)


def test_unknown_tag_rejected():
    with pytest.raises(ValidationError):
        Pet.model_validate({"pet": {"kind": "fish"}})


def test_json_schema_for_literal_union():
    disc = Pet.model_json_schema()["properties"]["pet"]["discriminator"]
    assert disc["propertyName"] == "kind"
    assert set(disc["mapping"]) == {"cat", "dog"}


def test_plain_model_validation_works():
    class Simple(BaseModel):
        n: int

    assert Simple.model_validate({"n": 3}).n == 3
    with pytest.raises(ValidationError):
        Simple.model_validate({"n": "not-an-int"})


# Literal[None] tags already worked at base: kept as a guard.
def test_optional_literal_none_union_tag():
    # Literal[None]-style spelling through Optional: the tag value None is
    # accepted anywhere a Literal tag is.
    class C(BaseModel):
        kind: Literal["c"] = "c"

    class D(BaseModel):
        kind: Literal[None] = None

    class M(BaseModel):
        item: Annotated[Union[C, D], Field(discriminator="kind")]

    assert isinstance(M.model_validate({"item": {"kind": None}}).item, D)
    assert isinstance(M.model_validate({"item": {"kind": "c"}}).item, C)
