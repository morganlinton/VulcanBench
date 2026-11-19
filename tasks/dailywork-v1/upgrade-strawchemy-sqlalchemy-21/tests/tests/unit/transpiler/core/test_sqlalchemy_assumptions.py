"""Spike on the SQLAlchemy behaviour the transpiler core relies on.

Verdict (a): plain ``aliased(mapper, page)`` does NOT work when the page subquery exports two aliases of the same
table. Columns are matched to the subquery by their base table column, so both entities resolve to the first exported
pair (the root's) and the second entity silently reads the root's columns. What works is adapting the column
expressions themselves: ``ClauseAdapter(page).traverse(alias.<attr>.__clause_element__())`` returns the column that
``page`` exports for that very alias, for the root and for the to-one self relation alike. No re-join is needed, so
``materialize`` must rewrite the projection with a ``ClauseAdapter`` over columns and must not build a per-alias
``aliased(mapper, page)`` entity when one table is exported twice. An entity over a subquery is only safe when the
exported columns carry the attribute names (a nested ``select`` relabelling one alias's columns), which adds a
subquery level.

Verdict (b): yes. ``aliased(Model, Model.__table__)`` is an alias whose columns are the table's own columns, so it
renders ``color.name = :name_1`` and can sit in the WHERE of an UPDATE or DELETE on ``Model`` without a second FROM.
"""

from __future__ import annotations

from typing import Any

from inline_snapshot import snapshot
from sqlalchemy import Subquery, delete, select, update
from sqlalchemy.dialects import postgresql
from sqlalchemy.orm import aliased
from sqlalchemy.sql.util import ClauseAdapter

from tests.unit.models import Color, SponsoredUser
from tests.utils import format_sql


def _two_alias_page() -> tuple[Any, Any, Subquery]:
    root = aliased(SponsoredUser)
    sponsor = aliased(SponsoredUser)
    page = (
        select(root.id, root.name, sponsor.id.label(None), sponsor.name.label(None))
        .join(sponsor, root.sponsor_id == sponsor.id)
        .subquery()
    )
    return root, sponsor, page


def test_aliased_on_subquery_with_two_aliases_of_one_table() -> None:
    """A page exporting two aliases of one table: ClauseAdapter finds each alias's own column, aliased() does not."""
    root, sponsor, page = _two_alias_page()
    exported = list(page.c)
    adapter = ClauseAdapter(page)

    root_name = adapter.traverse(root.name.__clause_element__())
    sponsor_name = adapter.traverse(sponsor.name.__clause_element__())

    assert root_name is exported[1]
    assert sponsor_name is exported[3]
    assert root_name is not sponsor_name

    # Plain entities over the subquery both resolve to the root's exported columns.
    root_entity = aliased(SponsoredUser, page)
    sponsor_entity = aliased(SponsoredUser, page)
    sql = format_sql(str(select(root_entity.name, sponsor_entity.name)))
    assert sql.splitlines() == snapshot(
        [
            "SELECT anon_1.name,",
            "       anon_1.name AS name__1",
            "  FROM (",
            "        SELECT sponsored_user_1.id AS id,",
            "               sponsored_user_1.name AS name,",
            "               sponsored_user_2.id AS id_1,",
            "               sponsored_user_2.name AS name_1",
            "          FROM sponsored_user AS sponsored_user_1",
            "          JOIN sponsored_user AS sponsored_user_2",
            "            ON sponsored_user_1.sponsor_id = sponsored_user_2.id",
            "       ) AS anon_1",
        ]
    )


def test_entity_over_relabelled_nested_subquery_finds_its_own_columns() -> None:
    """A nested select relabelling one alias's columns to attribute names yields an entity on the right columns."""
    _, _, page = _two_alias_page()
    exported = list(page.c)
    sponsor_view = select(exported[2].label("id"), exported[3].label("name")).subquery()

    sponsor_entity = aliased(SponsoredUser, sponsor_view, adapt_on_names=True)

    sql = format_sql(str(select(sponsor_entity.name)))
    assert sql.splitlines() == snapshot(
        [
            "SELECT anon_1.name",
            "  FROM (",
            "        SELECT anon_2.id_1 AS id,",
            "               anon_2.name_1 AS name",
            "          FROM (",
            "                SELECT sponsored_user_1.id AS id,",
            "                       sponsored_user_1.name AS name,",
            "                       sponsored_user_2.id AS id_1,",
            "                       sponsored_user_2.name AS name_1",
            "                  FROM sponsored_user AS sponsored_user_1",
            "                  JOIN sponsored_user AS sponsored_user_2",
            "                    ON sponsored_user_1.sponsor_id = sponsored_user_2.id",
            "               ) AS anon_2",
            "       ) AS anon_1",
        ]
    )


def test_table_alias_targets_table_columns() -> None:
    """``aliased(Model, Model.__table__)`` compiles to the table's own columns, with no second FROM in DML."""
    alias = aliased(Color, Color.__table__)
    dialect = postgresql.psycopg2.dialect()

    assert str(alias.name == "x") == "color.name = :name_1"
    update_sql = str(update(Color).where(alias.name == "x").values(name="y").compile(dialect=dialect))
    delete_sql = str(delete(Color).where(alias.name == "x").compile(dialect=dialect))

    assert update_sql == "UPDATE color SET name=%(name)s WHERE color.name = %(name_1)s"
    assert delete_sql == "DELETE FROM color WHERE color.name = %(name_1)s"
