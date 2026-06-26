# Implement a reactive spreadsheet engine

The `reactive` package should provide a `Sheet` whose cells are either constants
or formulas over other cells, with automatic recomputation. The work is split
across two files and both must be implemented:

- `reactive/graph.py` — `DependencyGraph` tracks dependencies between cells.
- `reactive/sheet.py` — `Sheet` holds cell values and formulas and drives
  recomputation through the graph.

## `Sheet` API

- `set_value(name, value)`: set `name` to a constant. Any cell that transitively
  depends on `name` must be recomputed.
- `set_formula(name, deps, fn)`: make `name` a formula. `deps` is the list of
  cell names it reads; its value is `fn(d)` where `d` is a dict mapping each
  dependency name to its current value. Setting the formula recomputes `name`
  and everything that transitively depends on it.
- `get(name)`: return the current value of `name`, or raise `KeyError` if the
  cell has never been set.

## Required behavior

- **Transitive propagation.** Changing a cell recomputes *all* downstream cells,
  not just direct dependents.
- **Topological order.** A cell is recomputed only after every cell it depends on
  has its up-to-date value. (If `d` depends on `b` and `c`, and both depend on
  `a`, then changing `a` recomputes `b` and `c` before `d`.)
- **Rebinding clears old dependencies.** Re-calling `set_formula` for a cell
  replaces its dependencies; the cell must no longer react to its previous deps.
- **Cycle detection.** If a `set_formula` call would create a dependency cycle
  (including a self-reference), raise `reactive.CycleError`.

## `DependencyGraph` contract

`Sheet` collaborates with the graph; implement these so they cooperate:

- `set_dependencies(node, deps)`: replace `node`'s direct dependencies.
- `dependents(node)`: the set of nodes that directly depend on `node`.
- `affected_order(start)`: `start` plus all transitive dependents, ordered so
  each node comes after the dependencies of it that are in the set; raise
  `CycleError` on a cycle among those nodes.
