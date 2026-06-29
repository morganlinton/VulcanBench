"""Sequence helpers that preserve input order."""


def compact(items):
    return [x for x in items if x]
