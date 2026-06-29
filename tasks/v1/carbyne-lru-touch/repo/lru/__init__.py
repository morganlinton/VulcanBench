"""A fixed-capacity LRU cache built on OrderedDict. put() inserts/updates and
evicts the least-recently-used entry when over capacity."""

from collections import OrderedDict


class LRU:
    def __init__(self, capacity):
        self.capacity = capacity
        self._d = OrderedDict()

    def put(self, key, value):
        if key in self._d:
            self._d.move_to_end(key)
        self._d[key] = value
        if len(self._d) > self.capacity:
            self._d.popitem(last=False)
