"""Cursor pagination must walk every record exactly once, in order.

The implementation spans two files: ``pagination/cursor.py`` (encode/decode and
the strict "is this record after the cursor?" comparison) and
``pagination/repository.py`` (ordering, slicing, and when to stop). A correct
fix touches both: the cursor comparison must be strict and carry the tie-breaker,
and the repository must not advertise a next page once the data is exhausted.
"""

import pytest

from pagination import Repository, encode_cursor, is_after


def make_records(n, ts=None):
    # distinct ids; ts defaults to id so order is unambiguous
    return [{"id": i, "ts": (ts(i) if ts else i)} for i in range(1, n + 1)]


def collect_all(repo, limit):
    seen = []
    cursor = None
    for _ in range(1000):  # guard: a buggy cursor can paginate forever
        page = repo.page(after=cursor, limit=limit)
        seen.extend(r["id"] for r in page.items)
        if page.next_cursor is None:
            return seen
        cursor = page.next_cursor
    raise AssertionError("pagination did not terminate")


def test_first_page_basic():
    # passes pre-fix: the first page (no cursor) is already correct
    repo = Repository(make_records(25))
    page = repo.page(after=None, limit=5)
    assert [r["id"] for r in page.items] == [1, 2, 3, 4, 5]


def test_is_after_is_strict():
    rec = {"id": 7, "ts": 100}
    # a record is not "after" its own cursor position
    assert is_after(rec, encode_cursor(rec)) is False
    assert is_after({"id": 8, "ts": 100}, encode_cursor(rec)) is True


def test_last_page_has_no_next_cursor():
    repo = Repository(make_records(5))
    page = repo.page(after=None, limit=5)  # exactly one full page
    assert [r["id"] for r in page.items] == [1, 2, 3, 4, 5]
    assert page.next_cursor is None


def test_full_walk_no_dupes_no_gaps():
    repo = Repository(make_records(25))
    seen = collect_all(repo, limit=10)
    assert seen == list(range(1, 26))  # every id once, in order


def test_pages_are_contiguous_no_boundary_repeat():
    repo = Repository(make_records(20))
    p1 = repo.page(after=None, limit=10)
    p2 = repo.page(after=p1.next_cursor, limit=10)
    assert [r["id"] for r in p1.items] == list(range(1, 11))
    assert [r["id"] for r in p2.items] == list(range(11, 21))  # no repeat of id 10


def test_tie_break_on_id_within_same_timestamp():
    # all records share a timestamp; ordering and cursoring must fall back to id
    repo = Repository([{"id": i, "ts": 42} for i in (5, 1, 3, 2, 4)])
    seen = collect_all(repo, limit=2)
    assert seen == [1, 2, 3, 4, 5]


def test_limit_larger_than_dataset():
    repo = Repository(make_records(3))
    page = repo.page(after=None, limit=100)
    assert [r["id"] for r in page.items] == [1, 2, 3]
    assert page.next_cursor is None


def test_empty_repository():
    repo = Repository([])
    page = repo.page(after=None, limit=10)
    assert page.items == []
    assert page.next_cursor is None
