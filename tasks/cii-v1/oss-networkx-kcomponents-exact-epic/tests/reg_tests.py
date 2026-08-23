"""Hidden pass-to-pass guards: everything k_components already gets right.

These pin exact decompositions the pre-change code produces correctly, the
documented public contract, and the untouched approximation module. Any
regression here zeroes the functional score.
"""

from __future__ import annotations

import pytest

import networkx as nx
from networkx.algorithms import approximation


def canonical(result):
    return {k: sorted(sorted(c) for c in comps) for k, comps in result.items()}


def test_petersen_decomposition():
    G = nx.petersen_graph()
    assert canonical(nx.k_components(G)) == {3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]]}


def test_pinned_random_graph_decompositions():
    for (n, p, seed), expected in [
    ((12, 0.5, 3), {3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]]}),
    ((14, 0.45, 7), {5: [[0, 1, 2, 4, 8, 9, 10, 11, 12, 13]], 4: [[0, 1, 2, 3, 4, 8, 9, 10, 11, 12, 13]], 3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]]}),
    ((16, 0.4, 11), {5: [[1, 4, 5, 6, 7, 8, 10, 11, 12, 13]], 4: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]], 3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]]}),
    ((18, 0.3, 5), {4: [[1, 2, 3, 4, 5, 6, 8, 9, 11, 13, 14, 15]], 3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 17]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]]}),
    ((20, 0.5, 9), {8: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15, 16, 17, 18, 19]], 7: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]], 6: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]], 5: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]], 4: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]], 3: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]], 2: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]], 1: [[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]]}),
    ]:
        G = nx.gnp_random_graph(n, p, seed=seed)
        assert canonical(nx.k_components(G)) == expected, (n, p, seed)


def test_output_contract_no_nesting_and_connectivity():
    for n, p, seed in [(12, 0.5, 3), (14, 0.45, 7), (16, 0.4, 11), (18, 0.3, 5)]:
        G = nx.gnp_random_graph(n, p, seed=seed)
        result = nx.k_components(G)
        assert sorted(result) == list(range(1, max(result) + 1))
        for k, comps in result.items():
            for comp in comps:
                assert not any(set(comp) < set(other) for other in comps)
                if len(comp) > k:
                    assert nx.node_connectivity(G.subgraph(comp)) >= k


def test_directed_graph_rejected():
    with pytest.raises(nx.NetworkXNotImplemented):
        nx.k_components(nx.DiGraph([(0, 1), (1, 2)]))


def test_approximation_module_unchanged():
    # The approximation algorithm is a different procedure and must keep its
    # documented behavior on the karate club graph.
    G = nx.karate_club_graph()
    result = approximation.k_components(G)
    assert sorted(sorted(c) for c in result[1]) == [sorted(G)]
    for k, comps in result.items():
        for comp in comps:
            assert len(comp) >= k
