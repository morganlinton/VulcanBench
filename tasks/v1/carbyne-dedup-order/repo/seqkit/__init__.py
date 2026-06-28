"""Sequence helpers that preserve input order."""


def compact(items):
    """Drop falsy values, preserving order."""
    return [x for x in items if x]
