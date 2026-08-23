"""Hidden fail-to-pass tests: nx.k_components must return the exact
Moody-White decomposition.

Expected values are exact canonical decompositions, independently verified:
every expected component induces a subgraph of node connectivity >= k, no
component is nested in another at the same level, and each whole graph that
is k-connected appears intact at every level up to its connectivity. All
graphs are deterministic (explicit constructions or fixed-seed G(n, p)).
"""

from __future__ import annotations

import networkx as nx


def canonical(result):
    return {k: sorted(sorted(c) for c in comps) for k, comps in result.items()}


def test_two_icosahedra_joined_by_connectors():
    # Two icosahedra (each 5-connected) joined through three connector
    # nodes, plus a fourth attachment node; the whole graph is 4-connected.
    # Both icosahedra must be reported intact at level 5.
    A = nx.icosahedral_graph()
    B = nx.relabel_nodes(nx.icosahedral_graph(), {i: i + 12 for i in range(12)})
    G = nx.union(A, B)
    G.add_edges_from([(24, 0), (24, 1), (24, 2), (24, 12), (24, 13)])
    G.add_edges_from([(25, 3), (25, 4), (25, 5), (25, 14), (25, 15)])
    G.add_edges_from([(26, 6), (26, 7), (26, 8), (26, 16), (26, 17)])
    G.add_edges_from([(27, 9), (27, 10), (27, 20), (27, 21)])
    assert nx.node_connectivity(G) == 4
    result = canonical(nx.k_components(G))
    assert result[5] == [sorted(range(12)), sorted(range(12, 24))]
    for k in range(1, 5):
        assert result[k] == [sorted(range(28))]


def _assert_exact(cases):
    for (n, p, seed), expected in cases:
        G = nx.gnp_random_graph(n, p, seed=seed)
        assert canonical(nx.k_components(G)) == expected, (n, p, seed)


def test_exact_decomposition_sparse_graphs():
    _assert_exact([
    ((18, 0.2, 16), {3: [[2, 4, 5, 6, 7, 9, 12, 13, 14, 15]], 2: [[0, 2, 3, 4, 5, 6, 7, 9, 10, 12, 13, 14, 15, 17]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]]}),
    ((20, 0.2, 21), {3: [[0, 1, 4, 8, 11, 14, 15, 19]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]]}),
    ((12, 0.3, 38), {3: [[0, 3, 4, 5, 8, 10, 11]], 2: [[0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]]}),
    ((14, 0.3, 24), {3: [[0, 3, 4, 5, 6, 7, 9, 10, 11, 12]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]]}),
    ])


def test_exact_decomposition_medium_graphs():
    _assert_exact([
    ((12, 0.4, 6), {3: [[0, 2, 3, 5, 6, 8, 9, 10, 11]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]]}),
    ((14, 0.3, 19), {4: [[0, 5, 7, 8, 9, 10, 11, 12]], 3: [[0, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]]}),
    ((12, 0.6, 0), {5: [[0, 3, 4, 6, 7, 8, 9, 10, 11]], 4: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]], 3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]]}),
    ((12, 0.6, 29), {5: [[0, 1, 2, 4, 5, 7, 9, 10]], 4: [[0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11]], 3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]]}),
    ])


def test_exact_decomposition_dense_graphs():
    _assert_exact([
    ((14, 0.6, 15), {6: [[1, 2, 3, 4, 5, 6, 10, 11, 12, 13]], 5: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 4: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]]}),
    ((14, 0.6, 21), {6: [[0, 1, 2, 4, 5, 6, 7, 8, 10, 11, 12]], 5: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 4: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]]}),
    ((16, 0.6, 16), {8: [[0, 1, 2, 3, 4, 7, 8, 9, 10, 11, 12, 13, 14, 15]], 7: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]], 6: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]], 5: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]], 4: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]], 3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]]}),
    ((18, 0.5, 36), {6: [[1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 15, 17]], 5: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17]], 4: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17]], 3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]]}),
    ])
