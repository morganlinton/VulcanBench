"""Hidden test for oss-more-itertools-interleave-empty.

interleave_evenly must handle an empty list of iterables by yielding nothing,
rather than raising. Behavior captured from upstream fix (more-itertools PR #1193).
"""
from more_itertools import interleave_evenly


def test_interleave_evenly_empty():
    assert list(interleave_evenly([])) == []


def test_interleave_evenly_basic():
    # pass_to_pass no-regression anchor (unchanged by the fix).
    assert list(interleave_evenly([[1, 2, 3], [4, 5]])) == [1, 4, 2, 3, 5]
