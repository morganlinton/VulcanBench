"""Finds the tables a SELECT reads twice for the same rows, which the transpiler must never generate.

A read is one FROM target: a table or its alias, a LATERAL, a CTE or a derived table, at any depth (subqueries,
EXISTS). Two reads are duplicates when they have the same base table, or the same body, and the same correlation:
the column equalities tying the target to another FROM, in its ON clause or WHERE, or else those of its subquery.
Alias names never count; columns are compared by the base table they belong to.
"""

from __future__ import annotations

import warnings
from dataclasses import dataclass, replace
from itertools import combinations
from math import prod
from typing import TYPE_CHECKING, Any, TypeAlias, cast

from sqlalchemy import Select, Table
from sqlalchemy.exc import CompileError, InvalidRequestError, SAWarning
from sqlalchemy.sql import column as core_column
from sqlalchemy.sql import operators
from sqlalchemy.sql import table as core_table
from sqlalchemy.sql.elements import BinaryExpression, BooleanClauseList, ColumnClause, Over, True_
from sqlalchemy.sql.functions import FunctionElement
from sqlalchemy.sql.selectable import CTE, AliasedReturnsRows, FromClause, FromGrouping, Join, Lateral
from sqlalchemy.sql.visitors import iterate, replacement_traverse

if TYPE_CHECKING:
    from collections.abc import Callable, Iterator

    from sqlalchemy import Dialect
    from sqlalchemy.sql import ClauseElement

__all__ = ("ALLOWED", "Read", "assert_no_duplicate_reads", "duplicate_reads")


_ColumnNode: TypeAlias = "tuple[int, str, str]"
"""A column: the key of its FROM, the base table label and the column name."""
_SmallPage: TypeAlias = "tuple[tuple[object, ...], str]"
"""A LATERAL of allowed duplicate 7: the group it forms with its siblings, and its normalized body."""

# Module state because ``plan_sql`` and ``QueryTracker`` run the check without the test's markers at hand; the
# ``allow_duplicate_reads`` fixture sets it for one test only.
_allowed_reason: str | None = None
"""Why the current test may read the same rows twice; ``None`` checks every statement."""
_allowed_dialects: tuple[str, ...] | None = None
"""The dialects ``_allowed_reason`` applies to; ``None`` for every dialect."""
_ADDED_KEY_PREFIXES = ("outer ", "via ")
"""Mark the correlations a read takes from its SELECT's WHERE or from its subquery rather than from its ON clause."""
_DERIVED = "sub"
"""Stands for the alias of a LATERAL, CTE or derived table in normalized text."""
_PAGE_PREFIXES = ("LIMIT ", "OFFSET ")
"""Start the filters that bound a page of rows rather than select other rows."""
_SMALL_PAGES_ROWS = 16
"""The most rows the pages of sibling LATERALs may multiply to and still be read separately."""
_AGGREGATES = frozenset(
    {
        "count",
        "min",
        "max",
        "sum",
        "avg",
        "stddev_samp",
        "stddev_pop",
        "var_samp",
        "var_pop",
        "array_agg",
        "json_agg",
        "bool_or",
        "bool_and",
        "string_agg",
        "group_concat",
    }
)


@dataclass(frozen=True)
class _Path:
    """Where a SELECT sits in the statement."""

    lineage: tuple[int, ...] = ()
    kinds: tuple[str, ...] = ()
    aliases: tuple[str, ...] = ()
    enclosing: frozenset[int] = frozenset()
    """Keys of the FROM targets of the enclosing SELECTs, which a subquery correlates to instead of reading."""
    filters: frozenset[str] = frozenset()
    """What restricts the rows of the SELECT reading this one as a derived table."""
    small_pages: tuple[tuple[int, _SmallPage], ...] = ()
    """The LATERALs of allowed duplicate 7 among the FROM targets of the enclosing SELECTs, by key."""
    small_page: _SmallPage | None = None
    """The LATERAL of allowed duplicate 7 this SELECT lies under."""

    def extend(
        self,
        select: Select[Any],
        kind: str,
        alias: str,
        targets: set[int],
        filters: frozenset[str],
        small_pages: dict[int, _SmallPage],
    ) -> _Path:
        lineage, kinds, aliases = (*self.lineage, id(select)), (*self.kinds, kind), (*self.aliases, alias)
        return _Path(
            lineage, kinds, aliases, self.enclosing | targets, filters, tuple(small_pages.items()), self.small_page
        )


@dataclass(frozen=True)
class Read:
    """One FROM target of a statement."""

    base: str
    """Base table name, or the normalized body of a LATERAL, CTE or derived table."""
    keys: frozenset[str]
    """Normalized column equalities tying the target to another FROM, leaving out those of a column to itself."""
    extras: frozenset[str]
    """The other conjuncts of the ON clause."""
    is_copy: bool
    """Tied to another read of the same table on identical columns, so it reads the same rows again."""
    outer: bool
    """Reached by an outer join."""
    joined: bool
    """Reached by a JOIN rather than being the first FROM of its SELECT."""
    lineage: tuple[int, ...]
    """Ids of the SELECTs holding the read, outermost first."""
    kinds: tuple[str, ...]
    """How each SELECT of ``lineage`` is reached: ``query``, ``from`` (derived table, LATERAL, CTE) or ``where``."""
    aliases: tuple[str, ...]
    """The alias name of the derived table holding each SELECT of ``lineage``, empty for the others."""
    dialect: str
    name: str = ""
    """The alias name of the target, empty for a bare table."""
    grouped: bool = False
    """Read by a subquery body selecting aggregate functions or grouping its rows."""
    grouped_by: bool = False
    """Read by a subquery body with a GROUP BY."""
    filters: frozenset[str] = frozenset()
    """What restricts the rows of the read's SELECT and of the SELECTs reading it as a derived table: DISTINCT ON, LIMIT,
    OFFSET and the normalized WHERE conjuncts that do not correlate it to an enclosing SELECT."""
    cte: int | None = None
    """Identifies the body of a CTE target, which every alias of that CTE shares."""
    cte_join: str = ""
    """The ON clause of a CTE target, its own columns read from one placeholder and every other FROM told apart."""
    small_page: _SmallPage | None = None
    """The LATERAL of allowed duplicate 7 the read is, lies under or is correlated to."""

    @property
    def derived(self) -> bool:
        """Tells whether the read is of a derived table rather than a base table."""
        return self.base.startswith("(")

    @property
    def select(self) -> int:
        """Identifies the SELECT the read belongs to."""
        return self.lineage[-1]

    @property
    def in_exists(self) -> bool:
        """Tells whether the read is inside a WHERE subquery."""
        return self.kinds[-1] == "where"

    @property
    def in_subquery(self) -> bool:
        """Tells whether the read is inside any subquery."""
        return self.kinds[-1] != "query"


def _under(first: Read, second: Read, alias: str) -> bool:
    """Tells whether exactly one of the reads lies under the derived table named ``alias``."""
    return (alias in first.aliases) != (alias in second.aliases)


def _describe(read: Read) -> str:
    return f"derived table {read.base[:60]!r}" if read.derived else f"table {read.base!r}"


def _duplicate(first: Read, second: Read) -> bool:
    if first.base != second.base:
        return False
    # Two joins of one CTE read the rows its body computes once; they repeat a read only with the same ON clause.
    if first.cte is not None and first.cte == second.cte and first.cte_join and second.cte_join:
        return first.cte_join == second.cte_join
    pairs = ((first, second), (second, first))
    # An EXISTS reading a copy of an enclosing read's row: whatever restricts either, the copy is that row again.
    if any(
        copy.is_copy and copy.in_exists and not other.is_copy and other.select in copy.lineage[:-1]
        for copy, other in pairs
    ):
        return True
    # Pages of one table's rows read those rows again: only what selects other rows tells two subqueries apart.
    if first.in_subquery and second.in_subquery and _row_filters(first) != _row_filters(second):
        return False
    on_clause_keys = {key for key in first.keys if not key.startswith(_ADDED_KEY_PREFIXES)}
    if first.keys == second.keys and (on_clause_keys or first.extras == second.extras):
        return True
    return any(
        copy.is_copy and not other.is_copy and not other.keys and other.select in copy.lineage for copy, other in pairs
    )


def _row_filters(read: Read) -> frozenset[str]:
    return frozenset(entry for entry in read.filters if not entry.startswith(_PAGE_PREFIXES))


def _walk(select: Select[Any], dialect: Dialect, path: _Path, kind: str, alias: str, reads: list[Read]) -> None:
    targets: list[tuple[FromClause, ClauseElement | None, bool]] = []
    for from_ in select.get_final_froms():
        _flatten(from_, None, False, targets)
    targets = [entry for entry in targets if _key(entry[0]) not in path.enclosing]
    keys_of_targets = {_key(target) for target, _, _ in targets}
    where = _conjuncts(select.whereclause)
    grouped = kind != "query" and _is_grouped(select)
    filters = (path.filters if kind == "from" else frozenset()) | {
        *(_normalized(conjunct, dialect) for conjunct in where if not _correlates(conjunct, keys_of_targets)),
        *_page(select, dialect),
    }
    # A SELECT restricts the rows of a derived table it reads alone, such as an emulated DISTINCT ON's rank; next to
    # other FROM clauses it restricts their combination, not the derived table's rows.
    inherited = filters if len(targets) == 1 else frozenset()
    small_pages = {**dict(path.small_pages), **_small_pages(select, targets, keys_of_targets, dialect)}
    inner = path.extend(select, kind, alias, keys_of_targets, inherited, small_pages)
    joined = {id(target) for target, onclause, _ in targets if onclause is not None}
    correlated = {
        id(target): _correlation(target, where, keys_of_targets, dialect, in_where=True)[0] for target, _, _ in targets
    }
    subquery_keys = frozenset(f"via {left} = {right}" for keys in correlated.values() for left, right in keys)
    equal_columns = _equal_columns([*where, *(c for _, on, _ in targets if on is not None for c in _conjuncts(on))])
    for target, onclause, outer in targets:
        keys, extras = _correlation(
            target,
            _conjuncts(onclause) if onclause is not None else where,
            keys_of_targets,
            dialect,
            in_where=onclause is None,
        )
        cte = id(target.element) if isinstance(target, CTE) else None
        walked = cte is not None and any(read.cte == cte for read in reads)
        copies = {key for key in keys if key[0] == key[1]}
        read_keys = frozenset(f"{left} = {right}" for left, right in keys - copies)
        is_copy = bool(copies) and keys == copies
        small_page = path.small_page or next(
            (small_pages[key] for key in (_key(target), *_referenced(target, onclause)) if key in small_pages), None
        )
        # Prefixed so that they never equal ON clause keys, which normalization could otherwise make equal; with these
        # keys alone, ``_duplicate`` still compares the extras, so they only ever tell two reads apart.
        if onclause is not None:
            read_keys |= {f"outer {left} = {right}" for left, right in correlated[id(target)]}
        elif not read_keys and kind != "query":
            read_keys = subquery_keys
            is_copy = _tied_to_outer_copy(target, equal_columns, keys_of_targets)
        reads.append(
            Read(
                base=_base(target, dialect),
                keys=read_keys,
                extras=frozenset(extras),
                is_copy=is_copy,
                outer=outer,
                joined=id(target) in joined,
                lineage=inner.lineage,
                kinds=inner.kinds,
                aliases=inner.aliases,
                dialect=dialect.name,
                name=_alias_name(target),
                grouped=grouped,
                grouped_by=grouped and bool(select._group_by_clauses),  # noqa: SLF001
                filters=filters,
                cte=cte,
                cte_join="" if cte is None or onclause is None else _cte_join(target, onclause, dialect),
                small_page=small_page,
            )
        )
        if not walked and isinstance(body := _body(target), Select):
            _walk(body, dialect, replace(inner, small_page=small_page), "from", _alias_name(target), reads)
    for subselect in _subselects(select):
        _walk(subselect, dialect, inner, "where", "", reads)


def _small_pages(
    select: Select[Any],
    targets: list[tuple[FromClause, ClauseElement | None, bool]],
    own: set[int],
    dialect: Dialect,
) -> dict[int, _SmallPage]:
    """Maps the LATERALs of ``select`` that allowed duplicate 7 covers to their sibling group and body, by key.

    Siblings select rows of the same tables correlated to the same FROM clauses of ``select``; a group of two or more is
    covered when every sibling has a LIMIT and their ``offset + limit`` multiply to at most ``_SMALL_PAGES_ROWS``.
    """
    groups: dict[tuple[object, ...], list[tuple[int, str, int | None]]] = {}
    for target, onclause, _ in targets:
        if isinstance(target, Lateral) and isinstance(body := _body(target), Select) and not _aggregates(body):
            correlation = _referenced_keys(body.whereclause) | _referenced_keys(onclause)
            parents = frozenset(key for key in correlation if key in own and key != _key(target))
            tables = frozenset(_table_label(from_) for from_ in body.get_final_froms())
            group = groups.setdefault((id(select), tables, parents), [])
            group.append((_key(target), _base(target, dialect), _rows_bound(body)))
    covered: dict[int, _SmallPage] = {}
    for group, siblings in groups.items():
        bounds = [bound for _, _, bound in siblings]
        if len(siblings) > 1 and None not in bounds and prod(cast("list[int]", bounds)) <= _SMALL_PAGES_ROWS:
            covered.update((key, (group, base)) for key, base, _ in siblings)
    return covered


def _aggregates(select: Select[Any]) -> bool:
    """Tells whether ``select`` computes aggregates or groups its rows rather than selecting a page of them."""
    return _is_grouped(select) or bool(select._group_by_clauses)  # noqa: SLF001


def _rows_bound(select: Select[Any]) -> int | None:
    """Returns ``offset + limit`` of ``select``, ``None`` without a LIMIT."""
    limit = select._limit  # noqa: SLF001
    return None if limit is None else limit + (select._offset or 0)  # noqa: SLF001


def _referenced(target: FromClause, onclause: ClauseElement | None) -> set[int]:
    """Returns the keys of the FROM clauses whose columns the ON clause of ``target`` or its body reads."""
    return _referenced_keys(onclause) | _referenced_keys(_body(target))


def _referenced_keys(element: ClauseElement | None) -> set[int]:
    if element is None:
        return set()
    return {_key(node.table) for node in iterate(element) if isinstance(node, ColumnClause) and node.table is not None}


def _equal_columns(conjuncts: list[ClauseElement]) -> dict[_ColumnNode, set[_ColumnNode]]:
    """Groups the columns that ``conjuncts`` make equal, transitively."""
    groups: dict[_ColumnNode, set[_ColumnNode]] = {}
    for conjunct in conjuncts:
        if (sides := _column_sides(conjunct)) is None:
            continue
        left, right = ((_key(side.table), _table_label(side.table), side.name) for side in sides)
        merged = groups.get(left, {left}) | groups.get(right, {right})
        for column in merged:
            groups[column] = merged
    return groups


def _tied_to_outer_copy(target: FromClause, equal_columns: dict[_ColumnNode, set[_ColumnNode]], own: set[int]) -> bool:
    """Tells whether a column of ``target`` equals, through any chain, the same column of an enclosing read of its table."""
    key = _key(target)
    return any(
        other_key not in own and (other_label, other_name) == (label, name)
        for (table, label, name), group in equal_columns.items()
        if table == key
        for other_key, other_label, other_name in group
    )


def _flatten(
    from_: FromClause,
    onclause: ClauseElement | None,
    outer: bool,
    out: list[tuple[FromClause, ClauseElement | None, bool]],
) -> None:
    if isinstance(from_, Join):
        _flatten(from_.left, onclause, outer, out)
        _flatten(from_.right, from_.onclause, from_.isouter, out)
    elif isinstance(from_, FromGrouping):
        _flatten(from_.element, onclause, outer, out)
    else:
        out.append((from_, onclause, outer))


def _subselects(element: Select[Any]) -> Iterator[Select[Any]]:
    """Yields the SELECTs nested in the expressions of ``element``, not those reached through a FROM."""
    roots = [element.whereclause, *element._having_criteria, *element._raw_columns, *element._order_by_clauses]  # noqa: SLF001
    stack: list[Any] = [root for root in roots if root is not None and not isinstance(root, FromClause)]
    while stack:
        for child in stack.pop().get_children():
            if isinstance(child, Select):
                yield child
            elif not isinstance(child, FromClause):
                stack.append(child)


def _page(select: Select[Any], dialect: Dialect) -> list[str]:
    clauses = [
        *(("DISTINCT ON", column) for column in select._distinct_on),  # noqa: SLF001
        ("LIMIT", select._limit_clause),  # noqa: SLF001
        ("OFFSET", select._offset_clause),  # noqa: SLF001
    ]
    return [f"{name} {_normalized(clause, dialect)}" for name, clause in clauses if clause is not None]


def _is_grouped(select: Select[Any]) -> bool:
    """Tells whether ``select`` selects an aggregate function outside a window; a GROUP BY alone deduplicates rows."""
    stack: list[Any] = list(select._raw_columns)  # noqa: SLF001
    while stack:
        element = stack.pop()
        if isinstance(element, FunctionElement) and getattr(element, "name", "").lower() in _AGGREGATES:
            return True
        if isinstance(element, (Over, FromClause)):
            continue
        stack.extend(element.get_children())
    return False


def _correlates(conjunct: ClauseElement, own: set[int]) -> bool:
    """Tells whether ``conjunct`` ties a FROM of the SELECT to one of an enclosing SELECT."""
    sides = _column_sides(conjunct)
    if sides is None:
        return False
    inside = [_key(side.table) in own for side in sides]
    return any(inside) and not all(inside)


def _conjuncts(expression: ClauseElement | None) -> list[ClauseElement]:
    if expression is None or isinstance(expression, True_):
        return []
    if isinstance(expression, BooleanClauseList) and expression.operator is operators.and_:
        return [conjunct for clause in expression.clauses for conjunct in _conjuncts(clause)]
    return [expression]


def _correlation(
    target: FromClause, conjuncts: list[ClauseElement], own: set[int], dialect: Dialect, *, in_where: bool
) -> tuple[set[tuple[str, str]], list[str]]:
    """Splits ``conjuncts`` into the column equalities tying ``target`` to another FROM and the rest.

    In a WHERE only the equalities reaching outside the SELECT's own targets correlate, and the rest filters the rows.
    """
    name = _key(target)
    keys: set[tuple[str, str]] = set()
    extras: list[str] = []
    for conjunct in conjuncts:
        sides = _column_sides(conjunct)
        if sides is not None:
            names = {_key(side.table) for side in sides}
            other = names - {name}
            if name in names and other and not (in_where and other & own):
                left, right = sorted(_column_text(side) for side in sides)
                keys.add((left, right))
                continue
        if not in_where:
            extras.append(_normalized(conjunct, dialect))
    return keys, extras


def _column_sides(conjunct: ClauseElement) -> tuple[ColumnClause[Any], ColumnClause[Any]] | None:
    if not (isinstance(conjunct, BinaryExpression) and conjunct.operator is operators.eq):
        return None
    left, right = conjunct.left, conjunct.right
    if all(isinstance(side, ColumnClause) and side.table is not None for side in (left, right)):
        return left, right  # ty: ignore[invalid-return-type]
    return None


def _key(table: object) -> int:
    """Identifies a FROM target across the annotated copies the ORM makes of it."""
    while (element := getattr(table, "_Annotated__element", None)) is not None:
        table = element
    return id(table)


def _body(target: ClauseElement) -> ClauseElement:
    """Returns what a LATERAL, CTE or derived table selects, below its aliases."""
    while isinstance(target, AliasedReturnsRows):
        target = target.element
    return target


def _alias_name(target: object) -> str:
    name = getattr(target, "name", None)
    return name if type(name) is str else ""


def _column_text(column: ColumnClause[Any]) -> str:
    """Names ``column`` by its base table; a column a derived table exports is named by the table column it proxies."""
    label = _table_label(column.table)
    if label == _DERIVED and len(origins := list(column.base_columns)) == 1:
        origin = origins[0]
        if isinstance(origin, ColumnClause) and isinstance(origin.table, Table):
            return f"{origin.table.name}.{origin.name}"
    return f"{label}.{column.name}"


def _table_label(table: object) -> str:
    while isinstance(table, AliasedReturnsRows):
        if not isinstance(table.element, (Table, AliasedReturnsRows)):
            return _DERIVED
        table = table.element
    return table.name if isinstance(table, Table) else _DERIVED


def _base(target: FromClause, dialect: Dialect) -> str:
    label = _table_label(target)
    if label != _DERIVED:
        return label
    body = _body(target)
    # Normalizing names every derived table ``sub``, so the bases of those the body reads tell two wrappers apart.
    wrapped: list[tuple[FromClause, ClauseElement | None, bool]] = []
    for from_ in body.get_final_froms() if isinstance(body, Select) else ():
        _flatten(from_, None, False, wrapped)
    inner = "".join(f" {_base(from_, dialect)}" for from_, _, _ in wrapped if _table_label(from_) == _DERIVED)
    return f"({_normalized(body, dialect)}{inner})"


def _cte_join(target: FromClause, onclause: ClauseElement, dialect: Dialect) -> str:
    """Compiles ``onclause`` with the columns of ``target`` read from one placeholder, other FROM clauses named by identity."""
    own = _key(target)

    def replace(candidate: ClauseElement, **_: object) -> ClauseElement | None:
        if isinstance(candidate, ColumnClause) and candidate.table is not None:
            key = _key(candidate.table)
            return core_column(candidate.name, _selectable=core_table(_DERIVED if key == own else f"from_{key}"))
        return None

    return _compiled(replacement_traverse(cast("Any", onclause), {}, cast("Any", replace)), dialect)


def _normalized(element: ClauseElement, dialect: Dialect) -> str:
    """Compiles ``element`` with every column of an alias replaced by one of its base table."""

    def replace(candidate: ClauseElement, **_: object) -> ClauseElement | None:
        if isinstance(candidate, ColumnClause) and candidate.table is not None:
            return core_column(candidate.name, _selectable=core_table(_table_label(candidate.table)))
        return None

    try:
        return _compiled(replacement_traverse(cast("Any", element), {}, cast("Any", replace)), dialect)
    except InvalidRequestError:
        # An ORM join in a subquery whose rewritten ON clause names none of its FROM clauses cannot pick the one it starts
        # from among several, so the element keeps its alias names.
        return _compiled(element, dialect)


def _compiled(rewritten: ClauseElement, dialect: Dialect) -> str:
    try:
        with warnings.catch_warnings():
            # A bind without a value yet, such as a selectinload's primary keys, would render as a literal NULL.
            warnings.simplefilter("error", SAWarning)
            return str(rewritten.compile(dialect=dialect, compile_kwargs={"literal_binds": True}))
    except (CompileError, SAWarning):
        return str(rewritten.compile(dialect=dialect))


def hooked_to_one_relation(first: Read, second: Read) -> bool:
    """Allowed 1: one relation joined twice on its key, inside and outside a page or not, one adding hook criteria."""
    return (
        first.joined
        and second.joined
        and (first.select in second.lineage or second.select in first.lineage)
        and bool(first.keys)
        and (first.extras < second.extras or second.extras < first.extras)
    )


def filtered_or_aggregated_and_selected(first: Read, second: Read) -> bool:
    """Allowed 2: a to-many relation read by an EXISTS or a grouped body, and again as rows, such as its selection."""
    grouped, rows = (first, second) if first.grouped else (second, first)
    if grouped.grouped and not rows.grouped and not grouped.is_copy and (grouped.keys or grouped.grouped_by):
        return True
    exists, joined = (first, second) if first.in_exists else (second, first)
    return (
        exists.in_exists
        and not exists.is_copy
        and joined.joined
        and not joined.in_exists
        and joined.select in exists.lineage[:-1]
    )


def user_statement_primary_key_join(first: Read, second: Read) -> bool:
    """Allowed 3: a non-trivial user statement stays a primary key subquery named ``user_statement``."""
    return _under(first, second, "user_statement")


def dml_derived_table(first: Read, second: Read) -> bool:
    """Allowed 4: MySQL reads the DML target through the derived table named ``dml_matched``."""
    return first.dialect == "mysql" and _under(first, second, "dml_matched")


def custom_filter_subquery(first: Read, second: Read) -> bool:
    """Allowed 6: a custom filter's statement, user SQL read from the alias named ``custom_filter``."""
    return "custom_filter" in {first.name, second.name}


def small_page_sibling_laterals(first: Read, second: Read) -> bool:
    """Allowed 7: sibling LATERALs of one relation with small pages, each an index-backed top N, and the reads under them."""
    return (
        first.small_page is not None
        and second.small_page is not None
        and first.small_page[0] == second.small_page[0]
        and first.small_page[1] != second.small_page[1]
    )


ALLOWED: tuple[Callable[[Read, Read], bool], ...] = (
    hooked_to_one_relation,
    filtered_or_aggregated_and_selected,
    user_statement_primary_key_join,
    dml_derived_table,
    custom_filter_subquery,
    small_page_sibling_laterals,
)


def duplicate_reads(statement: Select, dialect: Dialect) -> list[str]:
    """Returns one message per pair of reads of the same rows that no allowed duplicate covers."""
    reads: list[Read] = []
    _walk(statement, dialect, _Path(), "query", "", reads)
    return [
        f"{_describe(first)} is read twice with correlation {sorted(first.keys) or 'none'}"
        for first, second in combinations(reads, 2)
        if _duplicate(first, second) and not any(allowed(first, second) for allowed in ALLOWED)
    ]


def assert_no_duplicate_reads(statement: Select, dialect: Dialect) -> None:
    """Raises ``AssertionError`` listing the duplicate reads of ``statement``, unless the test allows them."""
    allowed = _allowed_reason is not None and (_allowed_dialects is None or dialect.name in _allowed_dialects)
    if not allowed and (messages := duplicate_reads(statement, dialect)):
        raise AssertionError("\n".join(messages))
