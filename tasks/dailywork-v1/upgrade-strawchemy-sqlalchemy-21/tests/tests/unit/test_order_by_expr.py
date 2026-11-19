from __future__ import annotations

from typing import Any

import pytest
from sqlalchemy import func
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column

from strawchemy.dto.strawberry import OrderByEnum, decompose_order_by
from strawchemy.exceptions import StrawchemyFieldError


class Base(DeclarativeBase): ...


class Widget(Base):
    __tablename__ = "widget"
    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column()


@pytest.mark.parametrize(
    ("expr", "expected"),
    [
        (Widget.name, ("name", OrderByEnum.ASC)),
        (Widget.name.asc(), ("name", OrderByEnum.ASC)),
        (Widget.name.desc(), ("name", OrderByEnum.DESC)),
        (Widget.name.asc().nulls_first(), ("name", OrderByEnum.ASC_NULLS_FIRST)),
        (Widget.name.asc().nulls_last(), ("name", OrderByEnum.ASC_NULLS_LAST)),
        (Widget.name.desc().nulls_first(), ("name", OrderByEnum.DESC_NULLS_FIRST)),
        (Widget.name.desc().nulls_last(), ("name", OrderByEnum.DESC_NULLS_LAST)),
    ],
)
def test_decompose_order_by(expr: Any, expected: tuple[str, OrderByEnum]) -> None:
    result = decompose_order_by(expr)
    assert (result.key, result.order) == expected


def test_decompose_order_by_rejects_unsupported() -> None:
    with pytest.raises(StrawchemyFieldError):
        decompose_order_by(Widget.name.distinct())


def test_decompose_order_by_rejects_non_column() -> None:
    """An ordering expression with no resolvable column key is rejected."""
    with pytest.raises(StrawchemyFieldError, match="Could not resolve a column"):
        decompose_order_by(func.now().asc())
