Add a native implementation of the Leiden community-detection algorithm.

NetworkX exposes `networkx.community.leiden_communities` and
`networkx.community.leiden_partitions`, but they currently only dispatch to an
external backend and raise `NotImplementedError` when run on the default
backend. Implement the algorithm natively so both functions work without a
backend.

The Leiden algorithm improves on Louvain by guaranteeing well-connected
communities. A correct implementation performs, per level: a fast local-moving
phase that greedily moves nodes between communities to improve the quality
function, a refinement phase that may split communities into well-connected
sub-communities, and an aggregation phase that collapses each refined community
into a super-node before repeating on the aggregated graph, until no further
improvement is possible.

The quality function optimized is the Constant Potts Model (CPM), the standard
objective for Leiden:

    Q = sum over communities c of [ e_c - resolution * n_c * (n_c - 1) / 2 ]

where `e_c` is the total weight of edges inside community `c` and `n_c` its
number of nodes (for weighted graphs, `e_c` uses the `weight` attribute). Note
this is CPM, not modularity: at `resolution=1` on sparse graphs it legitimately
produces many small communities.

Requirements:

- `leiden_communities(G, weight="weight", resolution=1, ..., seed=None)` returns
  the final list of communities (sets of nodes).
- `leiden_partitions(G, ...)` yields the successive partitions produced at each
  level of the algorithm, with the final level equal to what
  `leiden_communities` returns.
- The `resolution` parameter is the CPM resolution: higher resolution yields
  more, smaller communities.
- Results must be deterministic given a fixed `seed` (the same seed reproduces
  the same partition; different correct implementations may produce different
  partitions of comparable CPM quality).
- Every returned community must induce a connected subgraph (the Leiden
  guarantee), and every node must appear in exactly one community.
- Partitions must be genuinely good under CPM at the requested resolution: for
  example, on `nx.karate_club_graph()` at `resolution=0.5` the reference lands
  around Q = 2 to 8.5 depending on seed (degenerate outputs like all-singletons
  score 0, and modularity-style giant communities score far below 0).

Existing community-detection functions must be unaffected.
