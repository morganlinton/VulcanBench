"""Interval helpers. An interval is a (start, end) tuple with start <= end."""


def length(interval):
    return interval[1] - interval[0]
