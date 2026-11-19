from __future__ import annotations

import pytest
from sqlalchemy.dialects.postgresql import dialect

pytest.importorskip("geoalchemy2", reason="geoalchemy2 is not installed")

from strawchemy.schema.filters.geo import GeoComparison, GeoFilter
from tests.unit.models import GeoModel

pytestmark = [pytest.mark.geo, pytest.mark.extras]

_DIALECT = dialect()


@pytest.mark.parametrize(
    ("is_null", "expected"),
    [
        pytest.param(True, "geos_fields.point IS NULL", id="true"),
        pytest.param(False, "geos_fields.point IS NOT NULL", id="false"),
    ],
)
def test_geo_filter_is_null(is_null: bool, expected: str) -> None:
    """Test that an explicit is_null value compiles to the matching NULL check."""
    expressions = GeoFilter(comparison=GeoComparison(is_null=is_null)).to_expressions(_DIALECT, GeoModel.point)

    assert len(expressions) == 1
    assert str(expressions[0]) == expected


def test_geo_filter_is_null_unset() -> None:
    """Test that an omitted is_null produces no expression."""
    expressions = GeoFilter(comparison=GeoComparison()).to_expressions(_DIALECT, GeoModel.point)

    assert not expressions
