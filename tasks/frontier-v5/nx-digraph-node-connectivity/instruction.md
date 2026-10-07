# Node connectivity and node cuts are wrong on directed graphs

The workspace at `/app` is NetworkX, a pure-Python graph library. Install the
test dependencies and run the connectivity suite:

```
python -m pip install numpy scipy pytest
python -m pytest networkx/algorithms/connectivity/tests/test_connectivity.py \
                 networkx/algorithms/connectivity/tests/test_cuts.py
```

It passes as given. But `node_connectivity`, `minimum_node_cut` and
`minimum_st_node_cut` (in `networkx.algorithms.connectivity`) return wrong
answers on directed graphs, and on graphs with self-loops or parallel edges,
in ways the current tests do not cover. For example:

```python
>>> import networkx as nx
>>> # a directed triangle cannot be disconnected without removing a node
>>> nx.minimum_node_cut(nx.DiGraph([(0, 1), (1, 2), (2, 0)]))
set()                                    # wrong: returns a non-cut
>>> # weakly but not strongly connected: node connectivity is 0
>>> nx.node_connectivity(nx.DiGraph([(0, 3), (1, 2), (2, 1), (3, 0), (3, 1), (3, 2)]))
1                                        # wrong
>>> # K5 has node connectivity 4; self-loops must not change that
>>> G = nx.complete_graph(5); G.add_edges_from((u, u) for u in G)
>>> nx.node_connectivity(G)
6                                        # wrong
```

The problems include: a returned "cut" that does not actually disconnect the
graph; connectivity values that are too high for directed graphs that are only
weakly connected; sensitivity to self-loops and parallel edges, which should
never change node connectivity; and, for directed graphs, failing to take the
minimum over both orderings of a node pair, so some separations are missed.

Make all three functions correct for directed and undirected graphs, including
graphs with self-loops and parallel edges. A cut they return must be a valid
cut, and the connectivity value must match the minimum number of nodes whose
removal disconnects the graph (for digraphs, over ordered pairs). The public
signatures and the documented algorithm references do not change. Do not
special-case the examples above; fix the underlying causes.

Grading rebuilds the library from your workspace in a clean environment, runs a
held-out set of correctness tests for these functions, and then runs the full
existing connectivity test suite as a regression wall. A fix that passes the
held-out tests but breaks any existing test does not count. Only changes under
`networkx/` are graded.
