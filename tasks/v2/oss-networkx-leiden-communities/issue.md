Add a native implementation of the Leiden community-detection algorithm.

NetworkX exposes `networkx.community.leiden_communities` and
`networkx.community.leiden_partitions`, but they currently only dispatch to an
external backend and raise `NotImplementedError` when run on the default
backend. Implement the algorithm natively so both functions work without a
backend.

The Leiden algorithm improves on Louvain by guaranteeing well-connected
communities. A correct implementation performs, per level: a fast local-moving
phase that greedily moves nodes between communities to improve a quality
function, a refinement phase that may split communities into well-connected
sub-communities, and an aggregation phase that collapses each refined community
into a super-node before repeating on the aggregated graph, until no further
improvement is possible.

Requirements:

- `leiden_communities(G, weight="weight", resolution=1, ..., seed=None)` returns
  the final list of communities (sets of nodes).
- `leiden_partitions(G, ...)` yields the successive partitions produced at each
  level of the algorithm.
- The `resolution` parameter controls community granularity (higher resolution
  yields more, smaller communities).
- Results must be deterministic given a fixed `seed`, and must exactly match the
  reference algorithm's output for a given graph, seed, and resolution.
- Every node must appear in exactly one community.

Existing community-detection functions must be unaffected.
