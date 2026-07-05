"""Hidden test for oss-networkx-leiden-communities.

A native NetworkX implementation of the Leiden community-detection algorithm
must be added (`leiden_communities` and `leiden_partitions`). Grading is
semantic rather than exact-partition-matching (a seeded run's exact partition
depends on incidental RNG consumption order, which correct implementations
legitimately differ on). What is graded, on two graphs:

- partition QUALITY under the constant Potts model at resolution 0.5, with
  thresholds far above degenerate outputs (singletons score 0, classic
  label-propagation/greedy-modularity score deeply negative) yet below the
  across-seed floor of the reference implementation;
- Leiden's defining GUARANTEE: every returned community induces a connected
  subgraph (the property that separates Leiden from Louvain);
- resolution semantics: lower resolution coarsens the partition;
- seed determinism: the same seed reproduces the same partition;
- the `leiden_partitions` generator contract: its final level equals the
  top-level `leiden_communities` result.

Expected behavior captured from the upstream feature (networkx/networkx PR
#8509). Run with PYTHONPATH=. so the workspace networkx is under test.
"""
from __future__ import annotations

import networkx as nx


def _canon(communities):
    return sorted(sorted(n for n in c) for c in communities)


def _cpm(G, communities, resolution):
    """Constant Potts Model score: sum_c (e_c - resolution * n_c*(n_c-1)/2)."""
    score = 0.0
    for c in communities:
        cs = set(c)
        e_c = G.subgraph(cs).number_of_edges()
        n = len(cs)
        score += e_c - resolution * n * (n - 1) / 2
    return score


def test_leiden_partition_quality():
    # Karate club: reference implementation scores 2.0-8.5 across seeds at
    # resolution 0.5; singletons score 0; label propagation scores -54.
    G = nx.karate_club_graph()
    comms = nx.community.leiden_communities(G, resolution=0.5, seed=7)
    assert _cpm(G, comms, 0.5) >= 1.5

    # Les Miserables: reference scores 55-66 at resolution 0.5; label
    # propagation scores -91.
    H = nx.les_miserables_graph()
    comms_h = nx.community.leiden_communities(H, resolution=0.5, seed=7)
    assert _cpm(H, comms_h, 0.5) >= 30.0


def test_leiden_communities_are_connected():
    # Leiden's defining guarantee over Louvain: communities are connected.
    for G in (nx.karate_club_graph(), nx.les_miserables_graph()):
        for res in (1.0, 0.5):
            comms = nx.community.leiden_communities(G, resolution=res, seed=7)
            for c in comms:
                if len(c) > 1:
                    assert nx.is_connected(G.subgraph(c)), (
                        f"community {sorted(c)} is disconnected at resolution {res}"
                    )


def test_leiden_honors_resolution():
    # Lower resolution -> fewer, larger communities.
    G = nx.karate_club_graph()
    n_high = len(nx.community.leiden_communities(G, resolution=1.0, seed=7))
    n_low = len(nx.community.leiden_communities(G, resolution=0.5, seed=7))
    assert n_low < n_high


def test_leiden_deterministic_for_seed():
    G = nx.karate_club_graph()
    a = nx.community.leiden_communities(G, seed=7)
    b = nx.community.leiden_communities(G, seed=7)
    assert _canon(a) == _canon(b)


def test_leiden_partitions_generator():
    G = nx.karate_club_graph()
    parts = list(nx.community.leiden_partitions(G, seed=7))
    assert len(parts) >= 1
    # The final refinement level matches the communities returned by the top-level call.
    assert _canon(parts[-1]) == _canon(nx.community.leiden_communities(G, seed=7))


def test_leiden_is_a_valid_partition():
    # Whatever partition is produced, it must cover every node exactly once.
    G = nx.karate_club_graph()
    comms = nx.community.leiden_communities(G, seed=7)
    covered = sorted(n for c in comms for n in c)
    assert covered == sorted(G.nodes())


def test_existing_modularity_communities_unaffected():
    # A pre-existing community function must keep working.
    G = nx.karate_club_graph()
    comms = list(nx.community.greedy_modularity_communities(G))
    covered = sorted(n for c in comms for n in c)
    assert covered == sorted(G.nodes())
