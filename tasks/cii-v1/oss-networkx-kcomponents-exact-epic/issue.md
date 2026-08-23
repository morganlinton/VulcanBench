# `k_components` returns incorrect decompositions

Differential fuzzing of the exact `nx.k_components` algorithm against an
independent implementation shows it returns a wrong decomposition for
roughly 1 in 40 connected G(n, p) graphs (n = 12–20, p = 0.2–0.6). The
failures are silent — the output looks plausible — and take several forms:
a genuine k-component can be missed entirely, reported as a strict subset
of itself, replaced by fragments of lower connectivity, or accompanied by
a non-maximal component nested inside another at the same level.

One deterministic reproduction: take two icosahedra (each 5-connected) and
join them through three connector nodes plus a fourth attachment node so
the combined graph is 4-connected:

```python
A = nx.icosahedral_graph()
B = nx.relabel_nodes(nx.icosahedral_graph(), {i: i + 12 for i in range(12)})
G = nx.union(A, B)
G.add_edges_from([(24, 0), (24, 1), (24, 2), (24, 12), (24, 13)])
G.add_edges_from([(25, 3), (25, 4), (25, 5), (25, 14), (25, 15)])
G.add_edges_from([(26, 6), (26, 7), (26, 8), (26, 16), (26, 17)])
G.add_edges_from([(27, 9), (27, 10), (27, 20), (27, 21)])
```

The correct answer reports both icosahedra intact at level 5 and the whole
graph as the single component at levels 1–4. The current code loses one of
the two 5-components.

Fix the exact algorithm so that `nx.k_components` returns the exact
Moody–White decomposition: for every level k, exactly the maximal
k-connected subgraphs, none missing, none truncated, none replaced by
lower-connectivity fragments, and no component nested inside another at
the same level. Results on graphs the current code already handles
correctly must not change, and the approximation variant
(`networkx.algorithms.approximation.k_components`) must keep its documented
behavior. There may be more than one root cause.
