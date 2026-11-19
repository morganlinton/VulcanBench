from __future__ import annotations

from typing import Any

import pytest
import strawberry
from strawberry import LazyType
from strawberry.types import get_object_definition

from strawchemy.utils.registry import _TypeReference


@strawberry.type
class GroupNode:
    id: int


@strawberry.type(name="TagNode")
class RenamedTag:
    id: int


class NotStrawberry:
    pass


_REF_HOLDER = get_object_definition(GroupNode, strict=True).fields[0]
_NOT_THE_MEMBER = object()


@pytest.mark.parametrize(
    ("target", "target_name", "member", "expected"),
    [
        pytest.param(GroupNode, "GroupNode", GroupNode, True, id="identity-match"),
        pytest.param(_NOT_THE_MEMBER, None, GroupNode, False, id="no-target-name"),
        pytest.param(_NOT_THE_MEMBER, "TagNode", LazyType("TagNode", "some.module"), True, id="lazy-name-match"),
        pytest.param(_NOT_THE_MEMBER, "TagNode", LazyType("Other", "some.module"), False, id="lazy-name-mismatch"),
        pytest.param(_NOT_THE_MEMBER, "TagNode", RenamedTag, True, id="definition-name-match"),
        pytest.param(_NOT_THE_MEMBER, "TagNode", GroupNode, False, id="definition-name-mismatch"),
        pytest.param(_NOT_THE_MEMBER, "NotStrawberry", NotStrawberry, False, id="no-object-definition"),
    ],
)
def test_type_reference_matches_target(target: Any, target_name: str | None, member: Any, expected: bool) -> None:
    reference = _TypeReference(ref_holder=_REF_HOLDER, target=target, target_name=target_name)
    assert reference._matches_target(member) is expected  # noqa: SLF001
