# Group betweenness centrality returns wrong results in several cases

The workspace at `/app` is NetworkX, a pure-Python graph library. Install the
test dependencies and run its suite the usual way:

```
python -m pip install numpy scipy pandas pytest
python -m pytest networkx/algorithms/centrality/tests/test_group.py
```

The suite passes as given. But users relying on `group_betweenness_centrality`
(in `networkx.algorithms.centrality`) report that it returns wrong numbers, and
sometimes raises, on a range of inputs that the current tests do not cover:

- On an undirected, connected graph it can return values that disagree with the
  definition (the sum of pair-dependencies routed through the group), even for
  small graphs where the answer can be worked out by hand.
- On a directed graph that is not strongly connected, the counts of shortest
  paths through the group come out wrong, and some such graphs raise a
  `KeyError` from inside the computation instead of returning a result.
- When a pair of endpoints has no path between them (a shortest-path count of
  zero), the per-pair adjustment for the group is skipped in a way that
  corrupts the running total rather than contributing nothing.
- With `endpoints=True` the normalization is off, so the returned values are
  scaled incorrectly relative to the `endpoints=False` case.
- The result depends on the order in which the group's nodes are passed in: the
  same graph and the same group as a reordered list can give different numbers,
  which should never happen.

Fix `group_betweenness_centrality` so it is correct for directed and undirected
graphs, connected or not, with and without endpoints, and so its output depends
only on the set of group nodes, not their order. The definition and the public
signature do not change. Do not special-case the examples above; find and fix
the underlying causes so that arbitrary graphs give correct, order-independent
results.

Grading rebuilds the library from your workspace in a clean environment, runs a
held-out set of correctness tests for group betweenness, and then runs the
module's full existing centrality test suite as a regression wall. A fix that
makes the held-out tests pass but breaks any existing test does not count. Only
changes under `networkx/` are graded.
