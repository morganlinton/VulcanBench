class UndoJournal:
    """A stack of transaction frames. Each frame collects the *inverse* operations
    needed to undo the mutations made since the matching ``push``.

    ``Store`` records an inverse (a zero-arg callable) for every mutation. The
    journal owns the nesting semantics: ``rollback`` undoes the current frame and
    ``commit`` folds it into the parent so a later parent rollback still undoes it.
    """

    def __init__(self):
        self._frames = []  # list of frames; each frame is a list of undo callables

    def in_transaction(self):
        """True if at least one frame is open."""
        raise NotImplementedError

    def push(self):
        """Open a new (nested) frame."""
        raise NotImplementedError

    def record(self, undo):
        """Record an inverse operation into the current frame. If no frame is open
        (an autocommit mutation), the mutation is durable and nothing is recorded."""
        raise NotImplementedError

    def rollback(self):
        """Undo the current frame: run its inverses in reverse order, then pop it."""
        raise NotImplementedError

    def commit(self):
        """Close the current frame. Its inverses are folded into the parent frame
        (preserving order) so a later parent rollback also undoes them. At the top
        level there is no parent, so the changes become durable."""
        raise NotImplementedError
