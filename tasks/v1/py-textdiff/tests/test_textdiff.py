"""Line diff/patch split across two files, joined by a round-trip property.

``textdiff/diff.py`` builds an LCS-based edit script; ``textdiff/patch.py``
applies it (validating against the source). The defining invariant spans both:
apply(diff(a, b), a) == b. The kept lines must be a *longest* common subsequence,
and apply must reject an edit script that does not match its source.
"""

import pytest

from textdiff import PatchError, apply, diff


def lcs_len(a, b):
    # independent reference LCS length for the minimality check
    n, m = len(a), len(b)
    dp = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(n - 1, -1, -1):
        for j in range(m - 1, -1, -1):
            dp[i][j] = dp[i + 1][j + 1] + 1 if a[i] == b[j] else max(dp[i + 1][j], dp[i][j + 1])
    return dp[0][0]


def is_subsequence(sub, seq):
    it = iter(seq)
    return all(x in it for x in sub)


CASES = [
    (["a", "b", "c"], ["a", "b", "c"]),
    (["a", "b", "c"], ["a", "x", "c"]),
    (["a", "b", "c"], ["a", "c"]),
    (["a", "c"], ["a", "b", "c"]),
    ([], ["a", "b"]),
    (["a", "b"], []),
    ([], []),
    (["x", "y", "z", "w"], ["y", "w", "q"]),
    (["the", "quick", "brown", "fox"], ["the", "lazy", "brown", "dog"]),
]


def test_callables_exist():
    # passes pre-fix: the API is importable before it is implemented
    assert callable(diff) and callable(apply)


@pytest.mark.parametrize("a,b", CASES)
def test_round_trip(a, b):
    assert apply(diff(a, b), a) == b


@pytest.mark.parametrize("a,b", CASES)
def test_kept_lines_are_a_longest_common_subsequence(a, b):
    kept = [line for kind, line in diff(a, b) if kind == "keep"]
    assert is_subsequence(kept, a) and is_subsequence(kept, b)
    assert len(kept) == lcs_len(a, b)  # longest, not just any common subsequence


def test_ops_use_only_known_kinds():
    for kind, _line in diff(["a", "b"], ["a", "c"]):
        assert kind in {"keep", "del", "ins"}


def test_insert_only_diff():
    ops = diff(["a", "b"], ["a", "INS", "b"])
    assert [k for k, _ in ops] == ["keep", "ins", "keep"]
    assert apply(ops, ["a", "b"]) == ["a", "INS", "b"]


def test_delete_only_diff():
    ops = diff(["a", "b", "c"], ["a", "c"])
    assert [k for k, _ in ops] == ["keep", "del", "keep"]


def test_apply_rejects_wrong_source():
    ops = diff(["a", "b", "c"], ["a", "x", "c"])
    with pytest.raises(PatchError):
        apply(ops, ["a", "DIFFERENT", "c"])  # source does not match the script


def test_apply_requires_consuming_all_of_source():
    # a script that keeps only "a" leaves "b" unconsumed -> error
    with pytest.raises(PatchError):
        apply([("keep", "a")], ["a", "b"])
