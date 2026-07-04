"""Hidden test for oss-networkx-vf2pp-subgraph.

The VF2++ isomorphism module must gain subgraph-isomorphism and monomorphism
support: `vf2pp_subgraph_is_isomorphic`, `vf2pp_all_subgraph_isomorphisms`,
`vf2pp_monomorphism`, `vf2pp_is_monomorphic`, `vf2pp_all_monomorphisms`. Subgraph
isomorphism is node-induced (the mapped host subgraph must have exactly the
pattern's edges); monomorphism is an edge-preserving injection (extra host edges
allowed). Existing full-graph isomorphism must be unaffected. Expected behavior
captured from the upstream feature (networkx/networkx PR #8506).

Run with PYTHONPATH=. so the workspace networkx is under test.
"""
from __future__ import annotations

import networkx as nx
from networkx.algorithms.isomorphism import vf2pp as v

K4 = nx.complete_graph(4)
TRI = nx.cycle_graph(3)
P3 = nx.path_graph(3)
P4 = nx.path_graph(4)


def test_subgraph_isomorphism_positive_and_count():
    assert v.vf2pp_subgraph_is_isomorphic(K4, TRI) is True
    # 4 triangles in K4, each with 3! = 6 orderings -> 24 induced subgraph isomorphisms.
    assert len(list(v.vf2pp_all_subgraph_isomorphisms(K4, TRI))) == 24


def test_induced_semantics_reject_non_induced():
    # Any 3 nodes of K4 induce a triangle, never an induced path P3,
    # so there are zero *induced* subgraph isomorphisms of P3 into K4.
    assert v.vf2pp_subgraph_is_isomorphic(K4, P3) is False
    assert len(list(v.vf2pp_all_subgraph_isomorphisms(K4, P3))) == 0
    # P4 does not fit in K4 as an induced subgraph either.
    assert v.vf2pp_subgraph_is_isomorphic(K4, P4) is False


def test_monomorphism_allows_extra_host_edges():
    # P3 is not an *induced* subgraph of a triangle, but a monomorphism
    # (edge-preserving injection) exists.
    assert v.vf2pp_is_monomorphic(TRI, P3) is True
    assert v.vf2pp_subgraph_is_isomorphic(TRI, P3) is False
    # P3 monomorphisms into K4: 24 (every ordered triple of distinct nodes).
    assert len(list(v.vf2pp_all_monomorphisms(K4, P3))) == 24


def test_monomorphism_mapping_is_edge_preserving():
    m = v.vf2pp_monomorphism(K4, P3)
    assert m is not None
    # keys are host nodes, values pattern nodes (or vice versa); every P3 edge maps
    # to an existing K4 edge.
    inv = {pat: host for host, pat in m.items()}
    for a, b in P3.edges():
        assert K4.has_edge(inv[a], inv[b])


def test_full_graph_isomorphism_unaffected():
    # The pre-existing full-graph VF2++ isomorphism must still work.
    assert v.vf2pp_is_isomorphic(nx.cycle_graph(5), nx.cycle_graph(5)) is True
    assert v.vf2pp_is_isomorphic(nx.cycle_graph(5), nx.path_graph(5)) is False
