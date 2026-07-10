"""Hidden tests for oss-more-itertools-interleave-empty.

interleave_evenly must handle an empty list of iterables by yielding nothing,
rather than raising. Behavior captured from the upstream fix
(more-itertools PR #1193). At the unfixed base, consuming the result of
interleave_evenly([]) raises IndexError.
"""
import pytest

from more_itertools import interleave_evenly


# --- fail_to_pass: empty input must produce an empty iterator, not raise ---

def test_interleave_evenly_empty():
    assert list(interleave_evenly([])) == []


def test_interleave_evenly_empty_with_lengths():
    assert list(interleave_evenly([], lengths=[])) == []


def test_interleave_evenly_empty_stops_immediately():
    # At base this raises IndexError instead of a clean StopIteration.
    with pytest.raises(StopIteration):
        next(interleave_evenly([]))


# --- pass_to_pass: non-empty behavior is unchanged by the fix ---

def test_interleave_evenly_basic():
    assert list(interleave_evenly([[1, 2, 3], [4, 5]])) == [1, 4, 2, 3, 5]


def test_interleave_evenly_three():
    assert list(interleave_evenly([[1, 2], [3, 4], [5, 6]])) == [1, 3, 5, 2, 4, 6]
