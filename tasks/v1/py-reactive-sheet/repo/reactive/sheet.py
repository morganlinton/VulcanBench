from reactive.graph import DependencyGraph


class Sheet:
    """A spreadsheet of cells; each is a constant or a formula over other cells."""

    def __init__(self):
        self._graph = DependencyGraph()
        self._values = {}
        self._formulas = {}  # node -> (deps, fn)

    def set_value(self, name, value):
        """Set `name` to a constant, then recompute its dependents."""
        raise NotImplementedError

    def set_formula(self, name, deps, fn):
        """Set `name` to fn(values_of_deps); recompute it and its dependents.

        Raises CycleError if this introduces a dependency cycle."""
        raise NotImplementedError

    def get(self, name):
        """Return the current value of `name` (KeyError if unset)."""
        raise NotImplementedError
