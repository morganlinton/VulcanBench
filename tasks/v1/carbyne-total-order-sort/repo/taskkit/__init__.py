"""Task helpers. A task is a dict with 'priority' (int) and 'due' (a
datetime.date, or None meaning no due date)."""


def titles(tasks):
    return [t["title"] for t in tasks]
