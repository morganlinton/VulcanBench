"""List transforms. Each returns a NEW list and never mutates its input."""


def dedupe(items):
    seen = set()
    out = []
    for x in items:
        if x not in seen:
            seen.add(x)
            out.append(x)
    return out


def without(items, value):
    return [x for x in items if x != value]
