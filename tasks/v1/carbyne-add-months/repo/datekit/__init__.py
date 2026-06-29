"""Date helpers operating on datetime.date objects."""

from datetime import date, timedelta


def add_days(d, n):
    return d + timedelta(days=n)
