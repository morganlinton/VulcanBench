"""Hidden fail-to-pass tests: node connectivity and node cuts on digraphs,
self-loops and parallel edges. All behavior asserted via public APIs."""

import networkx as nx
from networkx.algorithms.connectivity import minimum_st_node_cut


def test_directed_triangle_minimum_node_cut():
    # The directed triangle is strongly connected with node connectivity 1:
    # removing any one node breaks the cycle. The minimum node cut must be a
    # real cut of size 1, not the empty set.
    G = nx.DiGraph([(0, 1), (1, 2), (2, 0)])
    cut = nx.minimum_node_cut(G)
    assert len(cut) == 1
    H = G.copy()
    H.remove_nodes_from(cut)
    assert not nx.is_strongly_connected(H)


def test_directed_st_cut_ignores_reverse_edge():
    # An edge from t to s is not an s-t path: s can still be separated from t.
    G = nx.DiGraph([(0, 1), (1, 2), (2, 0)])
    assert minimum_st_node_cut(G, 0, 2) == {1}
    # Only a direct s-to-t edge makes the pair inseparable.
    assert minimum_st_node_cut(G, 0, 1) == set()


def test_weakly_connected_digraph_connectivity_zero():
    # Weakly but not strongly connected: node connectivity is 0 and the
    # minimum node cut is empty (the graph is already not strongly connected).
    G = nx.DiGraph([(0, 1), (1, 2), (2, 0), (3, 0)])
    assert nx.is_weakly_connected(G)
    assert not nx.is_strongly_connected(G)
    assert nx.node_connectivity(G) == 0
    assert nx.minimum_node_cut(G) == set()


def test_digraph_connectivity_considers_both_orders():
    # Node 1 cannot reach node 0, so the graph is not strongly connected and
    # its node connectivity is 0 — but only the ordered pair (1, 0) shows it.
    G = nx.DiGraph([(0, 3), (1, 2), (2, 1), (3, 0), (3, 1), (3, 2)])
    assert nx.is_weakly_connected(G)
    assert not nx.is_strongly_connected(G)
    assert nx.node_connectivity(G) == 0


def test_strongly_connected_digraph_both_orders_cut():
    # Strongly connected; removing node 4 disconnects it, so both the node
    # connectivity and the minimum node cut size are 1, not 2.
    G = nx.DiGraph(
        [(0, 1), (0, 2), (0, 3), (1, 0), (1, 2), (2, 0), (2, 5), (3, 4),
         (3, 5), (4, 0), (4, 1), (4, 2), (4, 3), (4, 5), (5, 3), (5, 4)]
    )
    assert nx.is_strongly_connected(G)
    assert nx.node_connectivity(G) == 1
    cut = nx.minimum_node_cut(G)
    assert len(cut) == 1
    H = G.copy()
    H.remove_nodes_from(cut)
    assert not nx.is_strongly_connected(H)


def test_self_loops_do_not_inflate_connectivity():
    # K5 has node connectivity 4; self-loops must change neither the number
    # nor the minimum node cut size, on the graph and on its directed version.
    G = nx.complete_graph(5)
    G.add_edges_from((u, u) for u in G)
    D = G.to_directed()
    assert nx.node_connectivity(G) == 4
    assert nx.node_connectivity(D) == 4
    assert len(nx.minimum_node_cut(G)) == 4
    assert len(nx.minimum_node_cut(D)) == 4


def test_parallel_edges_do_not_inflate_connectivity():
    # Parallel edges add no node connectivity: doubled K5 still has 4.
    G = nx.complete_graph(5, nx.MultiGraph)
    G.add_edges_from(list(G.edges()))
    D = G.to_directed()
    assert nx.node_connectivity(G) == 4
    assert nx.node_connectivity(D) == 4
