"""Hidden test for oss-networkx-leiden-communities.

A native NetworkX implementation of the Leiden community-detection algorithm
must be added (`leiden_communities` and `leiden_partitions`), reproducing the
reference partition exactly for a fixed graph and seed, and honoring the
resolution parameter. Expected behavior captured from the upstream feature
(networkx/networkx PR #8509).

Run with PYTHONPATH=. so the workspace networkx is under test.
"""
from __future__ import annotations

import networkx as nx

KARATE_SEED7 = [
    [0, 1, 2, 3, 7, 13],
    [4, 10],
    [5, 6, 16],
    [8, 23, 25, 30, 31, 32, 33],
    [9], [11], [12], [14], [15], [17], [18], [19], [20], [21], [22],
    [24, 27], [26, 29], [28],
]
KARATE_RES05 = [
    [0, 1, 2, 3, 7, 13],
    [4, 5, 6, 10, 16],
    [8, 15, 23, 26, 29, 30, 32, 33],
    [9], [11], [12], [14], [17], [18], [19], [20], [21], [22],
    [24, 25, 27, 28, 31],
]


def _canon(communities):
    return sorted(sorted(int(n) for n in c) for c in communities)


def test_leiden_reproduces_reference_partition():
    G = nx.karate_club_graph()
    assert _canon(nx.community.leiden_communities(G, seed=7)) == KARATE_SEED7


def test_leiden_honors_resolution():
    G = nx.karate_club_graph()
    assert _canon(nx.community.leiden_communities(G, resolution=0.5, seed=7)) == KARATE_RES05


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
