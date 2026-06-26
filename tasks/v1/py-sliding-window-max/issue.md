# Implement an efficient sliding-window maximum

`windows.max_sliding_window(nums, k)` should return a list containing the
maximum of every contiguous window of length `k` in `nums`, left to right.

Requirements:

- The result has `len(nums) - k + 1` entries; entry `i` is `max(nums[i:i+k])`.
- `k <= 0` or `k > len(nums)` raises `ValueError`.
- It must run in **O(n)** time. The test suite includes a large input (200k
  elements) with a time budget, so an O(n*k) approach that recomputes the max of
  each window will be too slow. Use a monotonic deque (or equivalent) that
  amortizes to a constant amount of work per element.

The function lives in `windows/sliding.py`.
