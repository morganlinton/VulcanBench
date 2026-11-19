from __future__ import annotations

import json
import re
from datetime import date, datetime, time, timedelta
from decimal import Decimal
from statistics import mean, pstdev, pvariance, stdev, variance
from typing import TYPE_CHECKING, Any, Literal, TypeAlias
from uuid import UUID

from pydantic import TypeAdapter
from sqlalchemy import inspect

from tests.integration.types import postgres as pg_types

if TYPE_CHECKING:
    from collections.abc import Iterable

    from sqlalchemy.orm import DeclarativeBase


__all__ = ("from_graphql_representation", "graphql_input", "python_type", "to_graphql_representation")


_TimeDeltaType = TypeAdapter(timedelta)
AnyTypesModule: TypeAlias = pg_types  # ty: ignore[invalid-type-form]


def to_graphql_representation(value: Any, mode: Literal["input", "output"]) -> Any:
    """Convert Python values to their GraphQL string representation.

    This function transforms various Python data types into their appropriate
    GraphQL representation, with different handling based on whether the value
    is being used as input to a GraphQL query or as output from a GraphQL query.

    Args:
        value: The Python value to convert to a GraphQL representation.
            Supported types include:
            - Basic types (str, int, float, bool)
            - Date/time types (datetime, date, time)
            - Decimal and UUID
            - Dictionaries
        mode: Determines the conversion format:
            - "input": For values used in GraphQL query filters/arguments
            - "output": For values expected in GraphQL query results

    Returns:
        The GraphQL representation of the input value:
        - datetime/date/time: ISO format string
        - Decimal/UUID: String representation
        - Boolean (input mode): "true"/"false" strings
        - Dictionaries (input mode): JSON with GraphQL-style unquoted keys
        - Strings (input mode): Double-quoted

    Examples:
        >>> _to_graphql_representation(True, "input")
        'true'
        >>> _to_graphql_representation({"key": "value"}, "input")
        '{key: "value"}'
        >>> _to_graphql_representation(datetime(2023, 1, 1), "output")
        '2023-01-01T00:00:00'
    """
    expected = value

    if isinstance(value, (datetime, date, time)):
        expected = value.isoformat()
    elif isinstance(value, timedelta):
        expected = _TimeDeltaType.dump_python(value, mode="json")
    elif isinstance(value, (Decimal, UUID)):
        expected = str(value)

    if mode == "input":
        if value is True:
            expected = "true"
        elif value is False:
            expected = "false"
        elif isinstance(value, dict):
            expected = re.sub(r'"([^"]+)":', r"\g<1>:", json.dumps(value))
        if isinstance(value, (str, datetime, date, time)):
            expected = f'"{expected}"'

    return expected


def graphql_input(value: Any) -> str:
    if isinstance(value, list):
        return f"[{', '.join(to_graphql_representation(v, 'input') for v in value)}]"
    return to_graphql_representation(value, "input")


def from_graphql_representation(value: Any, type_: type[Any]) -> Any:
    if type_ in {date, datetime, time}:
        return type_.fromisoformat(value)
    if type_ is Decimal:
        return Decimal(value)
    return value


def python_type(model: type[DeclarativeBase], col_name: str) -> type[Any]:
    return inspect(model).mapper.columns[col_name].type.python_type


def compute_aggregation(
    graphql_aggregation: Literal["max", "min", "sum", "avg", "stddevSamp", "stddevPop", "varSamp", "varPop"],
    iterable: Iterable[int | float],
) -> float | Decimal:
    if graphql_aggregation == "max":
        value = max(iterable)
    elif graphql_aggregation == "min":
        value = min(iterable)
    elif graphql_aggregation == "sum":
        value = sum(iterable)
    elif graphql_aggregation == "avg":
        value = mean(iterable)
    elif graphql_aggregation == "stddevPop":
        value = pstdev(iterable)
    elif graphql_aggregation == "varPop":
        value = pvariance(iterable)
    elif graphql_aggregation == "stddevSamp":
        value = stdev(iterable)
    elif graphql_aggregation == "varSamp":
        value = variance(iterable)
    return value
