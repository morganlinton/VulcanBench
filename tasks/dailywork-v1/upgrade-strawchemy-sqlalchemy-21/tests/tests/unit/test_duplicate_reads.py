"""Tests of the duplicate read checker on hand-written statements."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest
from sqlalchemy import bindparam, func, select, true
from sqlalchemy.dialects import mysql, postgresql, sqlite
from sqlalchemy.orm import aliased

from tests.duplicate_reads import assert_no_duplicate_reads, duplicate_reads
from tests.unit.models import Color, Department, Fruit, Group, Tag, User, UserDepartmentJoinTable

if TYPE_CHECKING:
    from sqlalchemy import Dialect, Select
    from sqlalchemy.sql.selectable import CTE, LateralFromClause

DIALECT = postgresql.psycopg2.dialect()


def _color_count_lateral(color: type[Color], name: str) -> LateralFromClause:
    fruit = aliased(Fruit)
    return select(func.count().label("n")).select_from(fruit).where(fruit.color_id == color.id).lateral(name)


def test_flags_table_joined_twice_on_same_key() -> None:
    """A table joined twice on the same key is reported once."""
    group, first, second = aliased(Group, name="group"), aliased(Color, name="color_1"), aliased(Color, name="color_2")
    statement = (
        select(group.id, first.name, second.name)
        .select_from(group)
        .outerjoin(first, first.id == group.color_id)
        .outerjoin(second, second.id == group.color_id)
    )

    assert len(duplicate_reads(statement, DIALECT)) == 1


@pytest.mark.allow_duplicate_reads(reason="the test checks that the marker turns the check off")
def test_marker_turns_check_off() -> None:
    """In a test marked ``allow_duplicate_reads``, a statement reading a table twice passes the check."""
    group, first, second = aliased(Group, name="group"), aliased(Color, name="color_1"), aliased(Color, name="color_2")
    statement = (
        select(group.id, first.name, second.name)
        .select_from(group)
        .outerjoin(first, first.id == group.color_id)
        .outerjoin(second, second.id == group.color_id)
    )

    assert_no_duplicate_reads(statement, DIALECT)


def test_bind_without_value_is_not_rendered() -> None:
    """A statement holding a bind with no value yet, such as a selectinload's keys, is checked without a warning."""
    fruit = aliased(Fruit, name="fruit")
    statement = select(fruit.id).where(fruit.color_id.in_(bindparam("primary_keys", expanding=True)))

    assert duplicate_reads(statement, DIALECT) == []


def test_allows_hooked_relation_duplicate() -> None:
    """A relation joined for the filter and again with hook criteria, on the same key, is allowed."""
    group, plain, hooked = aliased(Group, name="group"), aliased(Color, name="color_1"), aliased(Color, name="color_2")
    statement = (
        select(group.id, hooked.id, hooked.name)
        .select_from(group)
        .join(plain, plain.id == group.color_id)
        .outerjoin(hooked, (hooked.id == group.color_id) & (hooked.name != "hidden"))
        .where(plain.name == "red")
    )

    assert duplicate_reads(statement, DIALECT) == []


def test_allows_hooked_relation_duplicate_across_page() -> None:
    """A relation joined for the filter inside a page and again with hook criteria outside it is allowed."""
    group, plain, hooked = aliased(Group, name="group"), aliased(Color, name="color_1"), aliased(Color, name="color_2")
    page = (
        select(group.id, group.color_id)
        .select_from(group)
        .join(plain, plain.id == group.color_id)
        .where(plain.name == "red")
        .limit(2)
        .subquery("group")
    )
    statement = (
        select(page.c.id, hooked.name)
        .select_from(page)
        .outerjoin(hooked, (hooked.id == page.c.color_id) & (hooked.name != "hidden"))
    )

    assert duplicate_reads(statement, DIALECT) == []


def test_flags_relation_joined_inside_and_outside_page_without_hook() -> None:
    """A relation joined inside a page and again outside it with the same ON clause is flagged."""
    group, inner, outer = aliased(Group, name="group"), aliased(Color, name="color_1"), aliased(Color, name="color_2")
    page = (
        select(group.id, group.color_id)
        .select_from(group)
        .join(inner, inner.id == group.color_id)
        .where(inner.name == "red")
        .limit(2)
        .subquery("group")
    )
    statement = select(page.c.id, outer.name).select_from(page).outerjoin(outer, outer.id == page.c.color_id)

    assert len(duplicate_reads(statement, DIALECT)) == 1


def test_allows_exists_plus_selection() -> None:
    """A to-many relation filtered in an EXISTS and selected by a join is allowed."""
    color, selected, tested = (
        aliased(Color, name="color"),
        aliased(Fruit, name="fruit_1"),
        aliased(Fruit, name="fruit_2"),
    )
    statement = (
        select(color.id, selected.name)
        .select_from(color)
        .outerjoin(selected, color.id == selected.color_id)
        .where(select(1).where(tested.color_id == color.id, tested.sweetness > 5).exists())
    )

    assert duplicate_reads(statement, DIALECT) == []


def test_flags_exists_root_copy() -> None:
    """An EXISTS starting from a copy of the root matched on its primary key is reported."""
    color, copy, fruit = aliased(Color, name="color"), aliased(Color, name="color_2"), aliased(Fruit, name="fruit_2")
    statement = (
        select(color.id)
        .select_from(color)
        .where(
            select(1)
            .select_from(copy)
            .join(fruit, fruit.color_id == copy.id)
            .where(copy.id == color.id, fruit.sweetness > 5)
            .exists()
        )
    )

    assert len(duplicate_reads(statement, DIALECT)) == 1


def test_flags_exists_copy_of_a_joined_relation() -> None:
    """An EXISTS reading a copy of a joined relation, matched on its key, is reported though the relation is correlated."""
    group, color = aliased(Group, name="group"), aliased(Color, name="color_1")
    copy, fruit = aliased(Color, name="color_2"), aliased(Fruit, name="fruit_1")
    statement = (
        select(group.id)
        .select_from(group)
        .join(color, color.id == group.color_id)
        .where(
            ~select(1)
            .select_from(copy)
            .outerjoin(fruit, fruit.color_id == copy.id)
            .where(copy.id == color.id, fruit.name == "kiwi")
            .exists()
        )
    )

    assert len(duplicate_reads(statement, DIALECT)) == 1


def test_flags_exists_copy_of_an_exists_relation() -> None:
    """An EXISTS reading a copy of the relation an enclosing EXISTS reads, matched on its key, is reported."""
    group, user, copy = aliased(Group, name="group"), aliased(User, name="user_1"), aliased(User, name="user_2")
    statement = select(group.id).where(
        select(1)
        .select_from(user)
        .where(
            user.group_id == group.id,
            ~select(1).select_from(copy).where(copy.id == user.id, copy.name == "x").exists(),
        )
        .exists()
    )

    assert len(duplicate_reads(statement, DIALECT)) == 1


def test_flags_exists_root_copy_with_outer_joined_relation() -> None:
    """The root copy of an EXISTS outer-joining a relation is reported: an OR splits at the EXISTS instead."""
    color, copy, fruit = aliased(Color, name="color"), aliased(Color, name="color_2"), aliased(Fruit, name="fruit_2")
    statement = (
        select(color.id)
        .select_from(color)
        .where(
            select(1)
            .select_from(copy)
            .outerjoin(fruit, fruit.color_id == copy.id)
            .where(copy.id == color.id, (fruit.sweetness > 5) | (copy.name == "red"))
            .exists()
        )
    )

    assert len(duplicate_reads(statement, DIALECT)) == 1


def test_flags_duplicate_lateral() -> None:
    """Two identical correlated LATERALs are reported."""
    color = aliased(Color, name="color")
    first, second = _color_count_lateral(color, "first"), _color_count_lateral(color, "second")
    statement = select(color.id).select_from(color).join(first, true()).join(second, true())

    assert duplicate_reads(statement, DIALECT)


def test_flags_duplicate_cte_read() -> None:
    """A CTE joined under two aliases on the same key is reported."""
    color = aliased(Color, name="color")
    cte = select(Fruit.color_id, func.count().label("n")).group_by(Fruit.color_id).cte("counts")
    again = cte.alias("counts_again")
    statement = (
        select(color.id)
        .select_from(color)
        .outerjoin(cte, cte.c.color_id == color.id)
        .outerjoin(again, again.c.color_id == color.id)
    )

    assert duplicate_reads(statement, DIALECT)


def test_flags_duplicate_derived_table() -> None:
    """Two identical derived tables joined on the same key are reported."""
    color = aliased(Color, name="color")
    first = select(Fruit.color_id).group_by(Fruit.color_id).subquery("first")
    second = select(Fruit.color_id).group_by(Fruit.color_id).subquery("second")
    statement = (
        select(color.id)
        .select_from(color)
        .outerjoin(first, first.c.color_id == color.id)
        .outerjoin(second, second.c.color_id == color.id)
    )

    assert duplicate_reads(statement, DIALECT)


def test_flags_duplicate_inside_nested_lateral() -> None:
    """A table joined twice on one key inside a LATERAL body is reported."""
    color = aliased(Color, name="color")
    fruit, first, second = aliased(Fruit, name="fruit"), aliased(Color, name="c1"), aliased(Color, name="c2")
    body = (
        select(fruit.id)
        .select_from(fruit)
        .join(first, first.id == fruit.color_id)
        .join(second, second.id == fruit.color_id)
        .where(fruit.color_id == color.id)
        .lateral("nested")
    )
    statement = select(color.id).select_from(color).join(body, true())

    assert len(duplicate_reads(statement, DIALECT)) == 1


def test_flags_root_reread_in_subquery_named_like_the_root() -> None:
    """A subquery reading the root table under the root's own alias name is still a second read."""
    outer, inner = aliased(Color, name="color"), aliased(Color, name="color")
    statement = select(outer.id).where(outer.id.in_(select(inner.id).where(inner.name == "red")))

    assert len(duplicate_reads(statement, DIALECT)) == 1


def _user_statement_join(name: str) -> Select[Any]:
    color, inner = aliased(Color, name="color"), aliased(Color, name="color_2")
    matched = select(inner.id).where(inner.name == "red").subquery(name)
    return select(color.id).select_from(color).join(matched, matched.c.id == color.id)


def test_allows_user_statement_subquery() -> None:
    """The primary key subquery named user_statement is the allowed duplicate of the user statement."""
    assert duplicate_reads(_user_statement_join("user_statement"), DIALECT) == []


def test_flags_primary_key_subquery_with_another_name() -> None:
    """A primary key subquery of the root under any other name is reported."""
    assert len(duplicate_reads(_user_statement_join("matched"), DIALECT)) == 1


def _dml_filter(name: str) -> Select[Any]:
    target, inner = aliased(Color, name="color"), aliased(Color, name="color_2")
    matched = select(inner.id).where(inner.name == "red").subquery(name)
    return select(target.id).where(select(1).select_from(matched).where(matched.c.id == target.id).exists())


@pytest.mark.parametrize(
    ("name", "dialect", "flagged"),
    [
        pytest.param("dml_matched", mysql.dialect(), False, id="mysql-dml_matched"),
        pytest.param("dml_matched", postgresql.psycopg2.dialect(), True, id="postgresql-dml_matched"),
        pytest.param("matched", mysql.dialect(), True, id="mysql-other-name"),
    ],
)
def test_dml_derived_table_needs_mysql_and_its_name(name: str, dialect: Dialect, flagged: bool) -> None:
    """The DML derived table is allowed on MySQL under its name only."""
    assert bool(duplicate_reads(_dml_filter(name), dialect)) is flagged


def test_flags_joins_with_disjoint_hook_criteria() -> None:
    """Two joins of one relation on its key whose extra criteria differ without one containing the other are reported."""
    group, first, second = aliased(Group, name="group"), aliased(Color, name="color_1"), aliased(Color, name="color_2")
    statement = (
        select(group.id)
        .select_from(group)
        .join(first, (first.id == group.color_id) & (first.name != "a"))
        .join(second, (second.id == group.color_id) & (second.name != "b"))
    )

    assert len(duplicate_reads(statement, DIALECT)) == 1


def test_flags_unkeyed_reads_of_one_table_in_one_select() -> None:
    """A table listed twice in FROM, with nothing tying the copies, is reported."""
    first, second = aliased(Color, name="color"), aliased(Color, name="color_2")
    statement = select(first.id).select_from(first, second)

    assert len(duplicate_reads(statement, DIALECT)) == 1


def test_flags_unkeyed_joins_with_identical_criteria() -> None:
    """Two joins of one table on the same non-key criteria are reported."""
    root, first, second = aliased(Group, name="group"), aliased(Color, name="color_1"), aliased(Color, name="color_2")
    statement = select(root.id).select_from(root).join(first, first.name == "red").join(second, second.name == "red")

    assert len(duplicate_reads(statement, DIALECT)) == 1


def test_allows_unkeyed_joins_with_different_criteria() -> None:
    """Two unkeyed joins of one table on different criteria read different rows."""
    root, first, second = aliased(Group, name="group"), aliased(Color, name="color_1"), aliased(Color, name="color_2")
    statement = select(root.id).select_from(root).join(first, first.name == "red").join(second, second.name == "blue")

    assert duplicate_reads(statement, DIALECT) == []


def _custom_filter_in(name: str) -> Select[Any]:
    color, inner = aliased(Color, name="color"), aliased(Color, name=name)
    return select(color.id).where(color.id.in_(select(inner.id).where(inner.name == "red")))


def test_allows_custom_filter_subquery() -> None:
    """A custom filter's subquery over the alias named custom_filter is the allowed duplicate of user SQL."""
    assert duplicate_reads(_custom_filter_in("custom_filter"), DIALECT) == []


def test_flags_custom_filter_subquery_with_another_name() -> None:
    """The same subquery over an alias of any other name is reported."""
    assert len(duplicate_reads(_custom_filter_in("matched"), DIALECT)) == 1


def test_allows_aggregate_lateral_plus_selection() -> None:
    """A to-many relation aggregated in a LATERAL and selected by a join, on the same key, is allowed."""
    color, selected = aliased(Color, name="color"), aliased(Fruit, name="fruit_1")
    counts = _color_count_lateral(color, "counts")
    statement = (
        select(color.id, selected.name, counts.c.n)
        .select_from(color)
        .join(counts, true())
        .outerjoin(selected, color.id == selected.color_id)
    )

    assert duplicate_reads(statement, DIALECT) == []


def test_flags_identical_aggregate_laterals_next_to_selection() -> None:
    """Two identical aggregate LATERALs are reported even next to a selection join of their relation."""
    color, selected = aliased(Color, name="color"), aliased(Fruit, name="fruit_1")
    first, second = _color_count_lateral(color, "first"), _color_count_lateral(color, "second")
    statement = (
        select(color.id, selected.name)
        .select_from(color)
        .join(first, true())
        .join(second, true())
        .outerjoin(selected, color.id == selected.color_id)
    )

    assert duplicate_reads(statement, DIALECT)


def _two_exists(first_name: str, second_name: str) -> Select[Any]:
    color, first, second = aliased(Color, name="color"), aliased(Fruit, name="fruit_1"), aliased(Fruit, name="fruit_2")
    return (
        select(color.id)
        .select_from(color)
        .where(select(1).where(first.color_id == color.id, first.name == first_name).exists())
        .where(select(1).where(second.color_id == color.id, second.name == second_name).exists())
    )


def test_allows_exists_on_one_correlation_with_different_predicates() -> None:
    """Two EXISTS on the same correlation testing different predicates need two related rows, which is allowed."""
    assert duplicate_reads(_two_exists("a", "b"), DIALECT) == []


def test_flags_identical_exists() -> None:
    """Two EXISTS on the same correlation testing the same predicates are reported."""
    assert len(duplicate_reads(_two_exists("a", "a"), DIALECT)) == 1


def test_flags_root_copy_inside_aggregate_lateral() -> None:
    """An aggregate LATERAL starting from a copy of the root matched on its primary key is reported."""
    color, copy, fruit = aliased(Color, name="color"), aliased(Color, name="color_2"), aliased(Fruit, name="fruit_2")
    counts = (
        select(func.count().label("n"))
        .select_from(copy)
        .join(fruit, copy.id == fruit.color_id)
        .where(copy.id == color.id)
        .lateral("counts")
    )
    statement = select(color.id, counts.c.n).select_from(color).join(counts, true())

    assert duplicate_reads(statement, DIALECT)


def test_flags_uncorrelated_root_read_inside_aggregate_lateral() -> None:
    """An aggregate LATERAL reading the root table again, tied to nothing outside it, is reported."""
    color, other, fruit = aliased(Color, name="color"), aliased(Color, name="color_3"), aliased(Fruit, name="fruit_3")
    counts = select(func.count().label("n")).select_from(other, fruit).where(other.id == fruit.color_id).lateral("c")
    statement = select(color.id, counts.c.n).select_from(color).join(counts, true())

    assert duplicate_reads(statement, DIALECT)


def _fruit_rows_lateral(color: type[Color], name: str, *, distinct: bool) -> LateralFromClause:
    fruit = aliased(Fruit)
    rows = select(fruit.id, fruit.name).select_from(fruit).where(fruit.color_id == color.id)
    if distinct:
        rows = rows.distinct(fruit.sweetness).order_by(fruit.sweetness)
    return rows.lateral(name)


def test_allows_laterals_on_one_correlation_with_different_rows() -> None:
    """Two row LATERALs on the same correlation, one keeping a DISTINCT ON subset, read different rows."""
    color = aliased(Color, name="color")
    first, second = _fruit_rows_lateral(color, "a", distinct=True), _fruit_rows_lateral(color, "b", distinct=False)
    statement = (
        select(color.id, first.c.name, second.c.name).select_from(color).join(first, true()).join(second, true())
    )

    assert duplicate_reads(statement, DIALECT) == []


def test_allows_deduplicated_rows_cte_next_to_aggregate_cte() -> None:
    """A rows CTE grouping by its own columns, joined to an aggregate CTE of the same table, is allowed."""
    color, color_1 = aliased(Color, name="color"), aliased(Color, name="color_1")
    counted, ranked = aliased(Fruit, name="fruit_2"), aliased(Fruit, name="fruit_1")
    counts = select(func.count().label("n"), counted.color_id).group_by(counted.color_id).cte("counts")
    rows = (
        select(ranked.id, ranked.color_id, counts.c.n)
        .select_from(ranked)
        .outerjoin(color_1, color_1.id == ranked.color_id)
        .outerjoin(counts, color_1.id == counts.c.color_id)
        .group_by(ranked.id, ranked.color_id, counts.c.n)
        .cte("rows")
    )
    statement = select(color.id, rows.c.id).select_from(color).outerjoin(rows, color.id == rows.c.color_id)

    assert duplicate_reads(statement, DIALECT) == []


def test_allows_rows_ctes_where_one_keeps_a_ranked_subset() -> None:
    """Two rows CTEs of one table differ when one keeps only the first row of each rank, as emulated DISTINCT ON does."""
    color, ranked, plain = aliased(Color, name="color"), aliased(Fruit, name="fruit_1"), aliased(Fruit, name="fruit_2")
    rank = func.row_number().over(partition_by=ranked.sweetness).label("position")
    inner = select(ranked.id, ranked.color_id, rank).subquery("ranked")
    first = select(inner.c.id, inner.c.color_id).where(inner.c.position == 1).cte("first")
    second = select(plain.id, plain.color_id).cte("second")
    statement = (
        select(color.id, first.c.id, second.c.id)
        .select_from(color)
        .outerjoin(first, color.id == first.c.color_id)
        .outerjoin(second, color.id == second.c.color_id)
    )

    assert duplicate_reads(statement, DIALECT) == []


def _counts_cte(name: str) -> CTE:
    fruit = aliased(Fruit)
    return select(func.count().label("n"), fruit.color_id).group_by(fruit.color_id).cte(name)


def test_flags_aggregate_cte_inside_and_outside_page() -> None:
    """An aggregate CTE joined inside a LIMIT page and again outside it is reported: the page restricts colors only."""
    color, inner, outer = aliased(Color, name="color"), _counts_cte("inner"), _counts_cte("outer")
    page = (
        select(color.id, inner.c.n)
        .select_from(color)
        .outerjoin(inner, color.id == inner.c.color_id)
        .order_by(inner.c.n)
        .limit(2)
        .subquery("color")
    )
    statement = select(page.c.id, outer.c.n).select_from(page).outerjoin(outer, page.c.id == outer.c.color_id)

    assert duplicate_reads(statement, sqlite.dialect())


@pytest.mark.parametrize("limit", [2, None])
def test_flags_aggregate_lateral_inside_and_outside_page(limit: int | None) -> None:
    """An aggregate LATERAL correlated to the root inside the page and to the page outside it is reported."""
    color = aliased(Color, name="color")
    inner = _color_count_lateral(color, "inner")
    page = select(color.id, inner.c.n).select_from(color).join(inner, true()).limit(limit).subquery("color")
    fruit = aliased(Fruit)
    outer = select(func.count().label("n")).select_from(fruit).where(fruit.color_id == page.c.id).lateral("outer")
    statement = select(page.c.id, outer.c.n).select_from(page).join(outer, true())

    assert duplicate_reads(statement, DIALECT)


def _limited_rows_lateral(color: type[Color], name: str, *, distinct: bool) -> LateralFromClause:
    fruit = aliased(Fruit)
    rows = select(fruit.id, fruit.name).select_from(fruit).where(fruit.color_id == color.id)
    rows = rows.distinct(fruit.sweetness).order_by(fruit.sweetness) if distinct else rows.order_by(fruit.id).limit(2)
    return rows.lateral(name)


@pytest.mark.parametrize("distinct", [False, True], ids=["limit", "distinct-on"])
def test_flags_identical_restricted_laterals(distinct: bool) -> None:
    """Two LATERALs with the same LIMIT, or the same DISTINCT ON, on one correlation are reported."""
    color = aliased(Color, name="color")
    first = _limited_rows_lateral(color, "a", distinct=distinct)
    second = _limited_rows_lateral(color, "b", distinct=distinct)
    statement = (
        select(color.id, first.c.name, second.c.name).select_from(color).join(first, true()).join(second, true())
    )

    assert duplicate_reads(statement, DIALECT)


def _users_of_departments_of(user: type[User]) -> tuple[Select[Any], type[Department]]:
    """Returns ``user``'s departments, joined to the secondary table correlated to ``user``, and that table."""
    department = aliased(Department, name="department_1")
    link = UserDepartmentJoinTable.alias("user_department_join_table_1")
    rows = select(1).select_from(department).join(link, department.id == link.c.department_id)
    return rows.where(user.id == link.c.user_id), department


def test_allows_secondary_table_read_on_both_sides_of_a_relation() -> None:
    """An EXISTS from a user's departments to their users reads the secondary table once per side, not flagged."""
    user, other = aliased(User, name="user"), aliased(User, name="user_1")
    rows, department = _users_of_departments_of(user)
    other_link = UserDepartmentJoinTable.alias("user_department_join_table_2")
    rows = rows.join(other_link, department.id == other_link.c.department_id).join(
        other, other.id == other_link.c.user_id
    )
    statement = select(user.id).select_from(user).where(rows.where(other.name == "Alice").exists())

    assert duplicate_reads(statement, DIALECT) == []


@pytest.mark.parametrize("nested", ["exists", "lateral"])
def test_allows_root_table_read_by_a_nested_relation_subquery(nested: str) -> None:
    """A subquery of a user's departments reading the users of each department reads other rows, not flagged."""
    user, other = aliased(User, name="user"), aliased(User, name="user_1")
    rows, department = _users_of_departments_of(user)
    other_link = UserDepartmentJoinTable.alias("user_department_join_table_2")
    users = (
        select(func.count(other.id).label("n"))
        .select_from(other)
        .join(other_link, other.id == other_link.c.user_id)
        .where(department.id == other_link.c.department_id)
    )
    if nested == "exists":
        rows = rows.where(~users.where(other.name == "Alice").exists())
    else:
        lateral = users.lateral("anon_1")
        rows = rows.join(lateral, true()).where(lateral.c.n < 2)
    statement = select(user.id).select_from(user).where(rows.exists())

    assert duplicate_reads(statement, DIALECT) == []


def test_flags_secondary_table_joined_twice_on_the_same_correlation() -> None:
    """An EXISTS joining the secondary table twice, both correlated to the same user, is reported."""
    user = aliased(User, name="user")
    rows, department = _users_of_departments_of(user)
    again = UserDepartmentJoinTable.alias("user_department_join_table_2")
    rows = rows.join(again, department.id == again.c.department_id).where(user.id == again.c.user_id)
    statement = select(user.id).select_from(user).where(rows.exists())

    assert duplicate_reads(statement, DIALECT)


def test_flags_root_copy_correlated_through_its_joined_relation() -> None:
    """An EXISTS reading a root copy, tied to the root only through its joined relation's key, is reported."""
    color, copy, fruit = aliased(Color, name="color"), aliased(Color, name="color_1"), aliased(Fruit, name="fruit_2")
    rows = (
        select(1)
        .select_from(copy)
        .join(fruit, copy.id == fruit.color_id)
        .where(fruit.color_id == color.id, fruit.sweetness > 5)
    )
    statement = select(color.id).select_from(color).where(rows.exists())

    assert duplicate_reads(statement, DIALECT)


def test_allows_reads_correlated_only_in_where_with_different_on_criteria() -> None:
    """Two joins of one table tied to the root only in WHERE, each ON clause restricting other rows, are not flagged."""
    color, group = aliased(Color, name="color"), aliased(Group, name="group_1")
    sweet, sour = aliased(Fruit, name="fruit_1"), aliased(Fruit, name="fruit_2")
    rows = (
        select(1)
        .select_from(group)
        .join(sweet, sweet.sweetness > 5)
        .join(sour, sour.sweetness < 2)
        .where(group.color_id == color.id, sweet.color_id == color.id, sour.color_id == color.id)
    )
    statement = select(color.id).select_from(color).where(rows.exists())

    assert duplicate_reads(statement, DIALECT) == []


def test_allows_one_cte_joined_to_two_parents() -> None:
    """A CTE joined under two aliases, each to a different alias of the parent table, is one read."""
    group, color, other = aliased(Group, name="group"), aliased(Color, name="color"), aliased(Color, name="color_2")
    cte = select(Fruit.color_id, func.count().label("n")).group_by(Fruit.color_id).cte("counts")
    again = cte.alias("counts_again")
    statement = (
        select(group.id)
        .select_from(group)
        .join(color, color.id == group.color_id)
        .join(other, other.id == group.id)
        .outerjoin(cte, cte.c.color_id == color.id)
        .outerjoin(again, again.c.color_id == other.id)
    )

    assert duplicate_reads(statement, DIALECT) == []


def test_allows_one_cte_joined_twice_with_different_bounds() -> None:
    """A CTE joined under two aliases on the same key with different extra conditions is one read."""
    color = aliased(Color, name="color")
    ranked = select(Fruit.color_id, func.dense_rank().over(partition_by=Fruit.color_id).label("rank")).cte("ranked")
    again = ranked.alias("ranked_again")
    statement = (
        select(color.id)
        .select_from(color)
        .outerjoin(ranked, (ranked.c.color_id == color.id) & (ranked.c.rank <= 2))
        .outerjoin(again, (again.c.color_id == color.id) & (again.c.rank <= 3))
    )

    assert duplicate_reads(statement, DIALECT) == []


def test_allows_one_cte_joined_and_tested_in_exists() -> None:
    """A CTE outer-joined and read again in an EXISTS on the same correlation is one read: its body is computed once."""
    color = aliased(Color, name="color")
    counts = select(Fruit.color_id, func.count().label("n")).group_by(Fruit.color_id).cte("counts")
    again = counts.alias("counts_again")
    statement = (
        select(color.id, counts.c.n)
        .select_from(color)
        .outerjoin(counts, counts.c.color_id == color.id)
        .where(select(again.c.n).where(again.c.color_id == color.id).exists())
    )

    assert duplicate_reads(statement, DIALECT) == []


@pytest.mark.allow_duplicate_reads(
    reason="the test checks that the marker spares only its dialects", dialects=("mysql",)
)
def test_marker_scoped_to_other_dialect_keeps_check() -> None:
    """In a test whose marker names only mysql, a postgresql statement reading a table twice still fails the check."""
    group, first, second = aliased(Group, name="group"), aliased(Color, name="color_1"), aliased(Color, name="color_2")
    statement = (
        select(group.id, first.name, second.name)
        .select_from(group)
        .outerjoin(first, first.id == group.color_id)
        .outerjoin(second, second.id == group.color_id)
    )

    with pytest.raises(AssertionError):
        assert_no_duplicate_reads(statement, DIALECT)


def test_allows_one_cte_tested_by_two_exists_with_different_predicates() -> None:
    """Two EXISTS reading one CTE on the same correlation with different predicates test different rows."""
    color = aliased(Color, name="color")
    counts = select(Fruit.color_id, func.count().label("n")).group_by(Fruit.color_id).cte("counts")
    again = counts.alias("counts_again")
    statement = (
        select(color.id)
        .select_from(color)
        .where(select(counts.c.n).where(counts.c.color_id == color.id, counts.c.n > 1).exists())
        .where(select(again.c.n).where(again.c.color_id == color.id, again.c.n > 2).exists())
    )

    assert duplicate_reads(statement, DIALECT) == []


def test_checks_predicate_holding_a_join_with_two_froms_to_start_from() -> None:
    """A NOT EXISTS whose join could start from either of its two FROM clauses, in a user's predicate, is not flagged."""
    group, user = aliased(Group, name="group"), aliased(User, name="user_1")
    tag, department = aliased(Tag, name="tag_1"), aliased(Department, name="department_1")
    link = UserDepartmentJoinTable.alias("user_department_join_table_1")
    tagged = (
        select(1)
        .select_from(tag)
        .select_from(department)
        .join(link, department.id == link.c.department_id)
        .where(tag.id == user.tag_id, user.id == link.c.user_id, department.name.is_(None))
    )
    users = select(1).select_from(user).where(group.id == user.group_id, ~tagged.exists())
    statement = select(group.id).select_from(group).where(users.exists())

    assert duplicate_reads(statement, DIALECT) == []


def _users_page(
    group: type[Group], name: str, limit: int | None, offset: int | None, *, descending: bool
) -> LateralFromClause:
    user = aliased(User)
    rows = select(user.id, user.name).select_from(user).where(user.group_id == group.id)
    rows = rows.order_by(user.name.desc() if descending else user.name.asc()).limit(limit).offset(offset)
    return rows.lateral(name)


def _departments_count(page: LateralFromClause, name: str) -> LateralFromClause:
    link = UserDepartmentJoinTable.alias()
    return select(func.count().label("n")).select_from(link).where(link.c.user_id == page.c.id).lateral(name)


def _sibling_pages(
    first_limit: int | None,
    second_limit: int | None,
    *,
    first_offset: int | None = None,
    nested: bool = False,
    counted: bool = False,
) -> Select:
    """Selects two user pages per group, with each page's departments count if ``nested``, its user count if ``counted``."""
    group = aliased(Group, name="group")
    first = _users_page(group, "a", first_limit, first_offset, descending=False)
    second = _users_page(group, "b", second_limit, None, descending=True)
    statement = (
        select(group.id, first.c.name, second.c.name).select_from(group).join(first, true()).join(second, true())
    )
    if nested:
        first_count, second_count = _departments_count(first, "a_count"), _departments_count(second, "b_count")
        statement = statement.add_columns(first_count.c.n, second_count.c.n)
        statement = statement.join(first_count, true()).join(second_count, true())
    if counted:
        user = aliased(User)
        count = select(func.count().label("n")).select_from(user).where(user.group_id == group.id).lateral("count")
        statement = statement.add_columns(count.c.n).join(count, true())
    return statement


@pytest.mark.parametrize(("first_limit", "second_limit"), [(2, 2), (4, 4), (1, 16)])
def test_allows_small_page_sibling_laterals(first_limit: int, second_limit: int) -> None:
    """Two sibling LATERALs of one relation, with pages multiplying to at most 16 rows, are allowed duplicate 7."""
    assert duplicate_reads(_sibling_pages(first_limit, second_limit), DIALECT) == []


@pytest.mark.parametrize(
    ("first_limit", "first_offset", "second_limit"), [(5, None, 10), (4, None, 5), (1, None, 17), (2, 7, 2)]
)
def test_flags_large_page_sibling_laterals(first_limit: int, first_offset: int | None, second_limit: int) -> None:
    """Two sibling LATERALs of one relation whose ``offset + limit`` multiply to more than 16 rows are reported."""
    messages = duplicate_reads(_sibling_pages(first_limit, second_limit, first_offset=first_offset), DIALECT)

    assert any("table 'user'" in message for message in messages)


@pytest.mark.parametrize(("first_limit", "second_limit"), [(None, None), (2, None)], ids=["both", "one"])
def test_flags_unbounded_sibling_laterals(first_limit: int | None, second_limit: int | None) -> None:
    """Two sibling LATERALs of one relation, one of them or both without a LIMIT, are reported."""
    messages = duplicate_reads(_sibling_pages(first_limit, second_limit), DIALECT)

    assert any("table 'user'" in message for message in messages)


def test_allows_reads_nested_under_small_page_siblings() -> None:
    """An aggregate LATERAL correlated to each of two small-page sibling LATERALs is allowed with them."""
    assert duplicate_reads(_sibling_pages(2, 2, nested=True), DIALECT) == []


def test_flags_reads_nested_under_large_page_siblings() -> None:
    """An aggregate LATERAL correlated to each of two large-page sibling LATERALs is reported."""
    messages = duplicate_reads(_sibling_pages(5, 10, nested=True), DIALECT)

    assert any("user_department_join_table" in message for message in messages)


def test_allows_small_page_siblings_next_to_an_aggregate_of_their_relation() -> None:
    """An aggregate LATERAL of the relation next to two small-page sibling LATERALs leaves them allowed duplicate 7."""
    assert duplicate_reads(_sibling_pages(2, 2, counted=True), DIALECT) == []


def _ranked_wrapper(rows: Select[Any], name: str) -> LateralFromClause:
    """Wraps ``rows`` ranked by ``id``, as a shared LATERAL does, keeping the first two ranks."""
    ranked = rows.add_columns(func.row_number().over(order_by=rows.selected_columns[0]).label("rank_1")).subquery()
    return select(ranked.c.id, ranked.c.rank_1).where(ranked.c.rank_1 <= 2).lateral(name)


def test_tells_apart_ranked_wrappers_of_other_tables() -> None:
    """Two ranked LATERALs over different tables, whose wrappers select the same columns, are not reported."""
    group, user, color = aliased(Group, name="group"), aliased(User), aliased(Color)
    users = _ranked_wrapper(select(user.id).where(user.group_id == group.id), "users")
    colors = _ranked_wrapper(select(color.id).where(color.id == group.color_id), "colors")
    statement = select(group.id, users.c.id, colors.c.id).select_from(group).join(users, true()).join(colors, true())

    assert duplicate_reads(statement, DIALECT) == []
