from __future__ import annotations

import pytest

from tests.utils import strip_loader_labels


@pytest.mark.parametrize(
    ("sql", "expected"),
    [
        pytest.param("SELECT fruit.name AS fruit_name", "SELECT fruit.name", id="plain"),
        pytest.param('SELECT "group".id AS group_id', 'SELECT "group".id', id="double-quoted"),
        pytest.param("SELECT `group`.id AS `group_id`", "SELECT `group`.id", id="backticks"),
        pytest.param(
            "SELECT fruit.name AS fruit_name, fruit.id AS fruit_id FROM fruit",
            "SELECT fruit.name, fruit.id FROM fruit",
            id="several",
        ),
        pytest.param("SELECT fruit.name AS name", "SELECT fruit.name AS name", id="user-label-kept"),
        pytest.param("SELECT fruit.name AS fruit_name_1", "SELECT fruit.name AS fruit_name_1", id="suffixed-kept"),
        pytest.param(
            "SELECT anon_1.fruit_name AS fruit_name",
            "SELECT anon_1.fruit_name AS fruit_name",
            id="other-table-kept",
        ),
    ],
)
def test_strip_loader_labels(sql: str, expected: str) -> None:
    """Test that only `<t>.<c> AS <t>_<c>` loader labels are removed."""
    assert strip_loader_labels(sql) == expected
