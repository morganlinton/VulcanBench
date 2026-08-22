"""Hidden pass-to-pass guards: undirected behavior and existing directed
results that were already correct must not regress."""

import networkx as nx
from networkx.algorithms.connectivity import minimum_st_node_cut


def test_undirected_complete_graph_connectivity():
    assert nx.node_connectivity(nx.complete_graph(5)) == 4
    assert len(nx.minimum_node_cut(nx.complete_graph(5))) == 4


def test_undirected_petersen_connectivity():
    G = nx.petersen_graph()
    assert nx.node_connectivity(G) == 3
    cut = nx.minimum_node_cut(G)
    assert len(cut) == 3
    H = G.copy()
    H.remove_nodes_from(cut)
    assert not nx.is_connected(H)


def test_directed_cycle_connectivity():
    # A directed cycle is strongly connected with node connectivity 1; this
    # was already reported correctly.
    G = nx.cycle_graph(6, create_using=nx.DiGraph)
    assert nx.node_connectivity(G) == 1


def test_undirected_st_cut_adjacent_nodes_empty():
    # For undirected graphs adjacent nodes cannot be separated: empty set.
    G = nx.path_graph(4)
    assert minimum_st_node_cut(G, 0, 1) == set()
    # Non-adjacent endpoints of a path are separated by one interior node.
    cut = minimum_st_node_cut(G, 0, 3)
    assert len(cut) == 1 and cut < {1, 2}


def test_disconnected_undirected_graph():
    G = nx.Graph([(0, 1), (2, 3)])
    assert nx.node_connectivity(G) == 0
