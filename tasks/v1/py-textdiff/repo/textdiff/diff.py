"""Compute a line-level diff between two sequences of lines.

NOTE: diff is not implemented — see issue.md. It must produce a minimal edit
script (longest common subsequence of kept lines) made of ("keep"|"del"|"ins",
line) operations, consumed by ``textdiff/patch.py``.
"""


def diff(a, b):
    """Return an edit script transforming list-of-lines ``a`` into ``b``.

    Each op is a tuple: ("keep", line) for a line common to both, ("del", line)
    for a line only in ``a``, ("ins", line) for a line only in ``b``. The kept
    lines must form a longest common subsequence of ``a`` and ``b``.
    """
    raise NotImplementedError
