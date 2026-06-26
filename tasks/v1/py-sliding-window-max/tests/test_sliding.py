"""Correctness plus a hard performance gate.

The naive O(n*k) solution (max of each window slice) passes correctness but is
too slow on the large input, so only an O(n) approach (monotonic deque) clears
test_large_input_is_efficient.
"""

import random
import time

import pytest

from windows import max_sliding_window


@pytest.mark.parametrize(
    "nums,k,want",
    [
        ([1, 3, -1, -3, 5, 3, 6, 7], 3, [3, 3, 5, 5, 6, 7]),
        ([4, 4, 4, 4], 2, [4, 4, 4]),
        ([9], 1, [9]),
        ([1, 2, 3, 4, 5], 5, [5]),
        ([5, 4, 3, 2, 1], 1, [5, 4, 3, 2, 1]),
        ([-1, -2, -3], 2, [-1, -2]),
        ([2, 1, 2, 1, 2], 2, [2, 2, 2, 2]),
    ],
)
def test_correctness(nums, k, want):
    assert max_sliding_window(nums, k) == want


@pytest.mark.parametrize("k", [0, -1, 4])
def test_invalid_k_raises(k):
    with pytest.raises(ValueError):
        max_sliding_window([1, 2, 3], k)


def test_large_input_is_efficient():
    rng = random.Random(12345)
    n, k = 200000, 4000
    nums = [rng.randint(-(10**6), 10**6) for _ in range(n)]
    start = time.perf_counter()
    got = max_sliding_window(nums, k)
    elapsed = time.perf_counter() - start
    assert len(got) == n - k + 1
    # Spot-check correctness against a direct max so a fast-but-wrong answer fails.
    for i in (0, 1, n // 2, n - k):
        assert got[i] == max(nums[i : i + k])
    assert elapsed < 4.0, f"too slow ({elapsed:.2f}s); needs O(n), not O(n*k)"


def test_module_imports():
    assert callable(max_sliding_window)
