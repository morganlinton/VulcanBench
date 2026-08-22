# node_connectivity, minimum_node_cut and minimum_st_node_cut return wrong results on directed graphs

`node_connectivity`, `minimum_node_cut` and `minimum_st_node_cut` return wrong
results on directed graphs. `node_connectivity` is also wrong on graphs with
self-loops or parallel edges, and `minimum_node_cut` on graphs with
self-loops. `minimum_node_cut` can return a set that is not a cut at all.

As an illustration:

```python
>>> import networkx as nx

>>> # a directed triangle has node connectivity 1
>>> nx.minimum_node_cut(nx.DiGraph([(0, 1), (1, 2), (2, 0)]))
set()

>>> # weakly but not strongly connected, so the node connectivity is 0
>>> nx.node_connectivity(nx.DiGraph([(0, 3), (1, 2), (2, 1), (3, 0), (3, 1), (3, 2)]))
1

>>> # K5 has node connectivity 4, self-loops should not change that
>>> G = nx.complete_graph(5)
>>> G.add_edges_from((u, u) for u in G)
>>> nx.node_connectivity(G)
6
```

The affected functions implement algorithm 11 of Esfahanian's *Connectivity
Algorithms* (cited in their docstrings). That algorithm is stated for
undirected graphs only; the directed handling in our implementation was
derived informally and does not follow from the paper.

Fix the directed, self-loop and parallel-edge behavior of these functions.
Undirected results on simple graphs are correct today and must not change.
