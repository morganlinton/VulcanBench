from __future__ import annotations

import random
from typing import TYPE_CHECKING

import pytest
from sqlalchemy import Column, Integer, MetaData, Table, Text, and_, create_engine, event, insert, select

from strawchemy.exceptions import FilterValueError
from strawchemy.schema.filters import TextComparison

if TYPE_CHECKING:
    from collections.abc import Callable, Iterator

    from sqlalchemy import Connection

_TABLE = Table("like_filter", MetaData(), Column("id", Integer, primary_key=True), Column("value", Text))
_RANDOM = random.Random(361)  # noqa: S311  # seeded test data, not cryptography
# Upper and lower ASCII letters, which SQLite's own LIKE folds, a newline, which LIKE wildcards match, and the escape.
_VALUES = sorted({"".join(_RANDOM.choice("abAB\n%_\\") for _ in range(_RANDOM.randint(0, 8))) for _ in range(80)})
_PATTERNS = sorted(
    pattern
    for pattern in {"".join(_RANDOM.choice("abA%_\\") for _ in range(_RANDOM.randint(0, 6))) for _ in range(4000)}
    if (len(pattern) - len(pattern.rstrip("\\"))) % 2 == 0
)
_ALL_IDS = set(range(1, len(_VALUES) + 1))


def _connection(*, case_sensitive_like: bool) -> Iterator[Connection]:
    engine = create_engine("sqlite://")
    if case_sensitive_like:
        event.listen(
            engine, "connect", lambda dbapi_connection, _: dbapi_connection.execute("PRAGMA case_sensitive_like = ON")
        )
    with engine.connect() as connection:
        _TABLE.create(connection)
        connection.execute(insert(_TABLE), [{"value": value} for value in _VALUES])
        yield connection
    engine.dispose()


def _like_mismatches(connection: Connection, operator: str, negated_operator: str) -> list[str]:
    mismatches: list[str] = []
    for pattern in _PATTERNS:
        native = set(connection.scalars(select(_TABLE.c.id).where(_TABLE.c.value.like(pattern, escape="\\"))))
        for name, expected in ((operator, native), (negated_operator, _ALL_IDS - native)):
            comparison = TextComparison(**{name: pattern})
            where = and_(*comparison.to_expressions(connection.dialect, _TABLE.c.value))
            if set(connection.scalars(select(_TABLE.c.id).where(where))) != expected:
                mismatches.append(f"{name}: {pattern!r}")
    return mismatches


@pytest.fixture(scope="module")
def connection() -> Iterator[Connection]:
    yield from _connection(case_sensitive_like=False)


@pytest.fixture(scope="module")
def case_sensitive_connection() -> Iterator[Connection]:
    yield from _connection(case_sensitive_like=True)


def test_sqlite_ilike_matches_native_like(connection: Connection) -> None:
    """Test that ilike and nilike on SQLite select the same ASCII rows as SQLite's own case-insensitive LIKE."""
    assert not _like_mismatches(connection, "ilike", "nilike")


def test_sqlite_like_matches_native_case_sensitive_like(case_sensitive_connection: Connection) -> None:
    """Test that like and nlike on SQLite select the same rows as SQLite's own LIKE once made case-sensitive."""
    assert not _like_mismatches(case_sensitive_connection, "like", "nlike")


@pytest.mark.parametrize(
    ("operator", "matches"),
    [
        pytest.param("startswith", str.startswith, id="startswith"),
        pytest.param("endswith", str.endswith, id="endswith"),
        pytest.param("contains", str.__contains__, id="contains"),
        pytest.param("istartswith", lambda value, part: value.lower().startswith(part.lower()), id="istartswith"),
        pytest.param("iendswith", lambda value, part: value.lower().endswith(part.lower()), id="iendswith"),
        pytest.param("icontains", lambda value, part: part.lower() in value.lower(), id="icontains"),
    ],
)
def test_sqlite_literal_operators_ignore_wildcards(
    operator: str, matches: Callable[[str, str], bool], connection: Connection
) -> None:
    """Test that the literal text operators on SQLite read wildcards and the escape as plain characters."""
    mismatches: list[str] = []
    for part in _PATTERNS:
        comparison = TextComparison(**{operator: part})
        where = and_(*comparison.to_expressions(connection.dialect, _TABLE.c.value))
        expected = {i for i, value in enumerate(_VALUES, start=1) if matches(value, part)}
        if set(connection.scalars(select(_TABLE.c.id).where(where))) != expected:
            mismatches.append(repr(part))

    assert not mismatches


@pytest.mark.parametrize("operator", ["like", "nlike", "ilike", "nilike"])
@pytest.mark.parametrize("pattern", ["\\", "a\\", "a\\\\\\"])
def test_like_pattern_ending_with_escape_raises(operator: str, pattern: str, connection: Connection) -> None:
    """Test that a LIKE pattern ending with a lone escape character raises a filter value error."""
    with pytest.raises(FilterValueError, match="must not end with an escape character"):
        TextComparison(**{operator: pattern}).to_expressions(connection.dialect, _TABLE.c.value)
