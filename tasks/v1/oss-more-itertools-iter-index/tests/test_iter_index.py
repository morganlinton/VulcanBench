"""Negative start/stop bounds for iter_index (upstream test for the fix).

'A' occurs at indexes 0, 1, 4, 7 in 'AABCADEAF'. With negative start/stop, a
general iterable (the slow path) must produce the same result as a sequence
(the fast path), matching built-in list.index/str.index semantics.
"""

import pytest

import more_itertools as mi


@pytest.mark.parametrize("wrapper", [list, iter])
@pytest.mark.parametrize(
    "kwargs,expected",
    [
        ({"start": -3}, [7]),
        ({"start": -9}, [0, 1, 4, 7]),
        ({"stop": -2}, [0, 1, 4]),
        ({"start": -5, "stop": -1}, [4, 7]),
    ],
)
def test_negative_bounds(wrapper, kwargs, expected):
    assert list(mi.iter_index(wrapper("AABCADEAF"), "A", **kwargs)) == expected


def test_positive_bounds_still_work():
    # passes before the fix too (regression guard)
    assert list(mi.iter_index("AABCADEAF", "A", start=2)) == [4, 7]
    assert list(mi.iter_index(iter("AABCADEAF"), "A", stop=5)) == [0, 1, 4]
