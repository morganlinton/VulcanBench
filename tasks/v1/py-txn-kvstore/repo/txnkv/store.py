from txnkv.journal import UndoJournal


class Store:
    """An in-memory key/value store with nested transactions.

    Mutations made inside a transaction can be rolled back; ``begin`` opens a
    (possibly nested) transaction, ``commit`` keeps its changes, and ``rollback``
    discards them, restoring the exact prior state (including keys that did not
    previously exist). Mutations made with no open transaction are durable.
    """

    def __init__(self):
        self._data = {}
        self._journal = UndoJournal()

    def get(self, key):
        """Return the value for ``key`` or raise ``KeyError`` if absent."""
        raise NotImplementedError

    def set(self, key, value):
        """Set ``key`` to ``value``, recording how to undo it."""
        raise NotImplementedError

    def delete(self, key):
        """Delete ``key`` (``KeyError`` if absent), recording how to undo it."""
        raise NotImplementedError

    def keys(self):
        """Return the current set of keys."""
        raise NotImplementedError

    def begin(self):
        """Open a new transaction (transactions may nest)."""
        raise NotImplementedError

    def commit(self):
        """Commit the innermost open transaction. ``RuntimeError`` if none is open."""
        raise NotImplementedError

    def rollback(self):
        """Roll back the innermost open transaction. ``RuntimeError`` if none is open."""
        raise NotImplementedError
