"""Hidden fail-to-pass tests: a bare `None` annotation as a discriminated-union
member's tag. Models are defined inside each test so the schema-build failure
at base fails each test individually. Public API only; no message text."""

from __future__ import annotations

from typing import Annotated, Literal, Union

from pydantic import BaseModel, Field


def _build_model():
    class A(BaseModel):
        field: Literal["A"] = "A"

    class B(BaseModel):
        field: None = None

    class Model(BaseModel):
        a_or_b: Annotated[Union[A, B], Field(discriminator="field")]

    return A, B, Model


def test_none_member_builds_and_validates():
    A, B, Model = _build_model()
    m = Model.model_validate({"a_or_b": {"field": None}})
    assert isinstance(m.a_or_b, B)


def test_literal_member_still_routes():
    A, B, Model = _build_model()
    m = Model.model_validate({"a_or_b": {"field": "A"}})
    assert isinstance(m.a_or_b, A)


def test_json_schema_maps_null():
    A, B, Model = _build_model()
    disc = Model.model_json_schema()["properties"]["a_or_b"]["discriminator"]
    assert disc["propertyName"] == "field"
    assert disc["mapping"]["A"].endswith("/A")
    assert disc["mapping"]["null"].endswith("/B")
