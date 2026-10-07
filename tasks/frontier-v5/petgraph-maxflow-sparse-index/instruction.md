# Max flow panics on graphs that have had nodes or edges removed

The workspace at `/app` is petgraph, a Rust graph library. Build and test it
with `cargo test` (the toolchain and all dependencies are present; the crate
is configured to build offline).

The maximum-flow algorithms, Dinic's (`dinic`) and Ford-Fulkerson
(`ford_fulkerson`) in `petgraph::algo::maximum_flow`, are wrong on graphs
whose node or edge indices are not contiguous, which happens whenever a node
or edge has been removed from a `StableGraph`. On such a graph the algorithms
can panic with an out-of-bounds index, or return an incorrect flow, even
though the same graph rebuilt without the holes works fine.

The maximum flow value and the per-edge flows must be correct for any graph,
including a `StableGraph` from which nodes or edges have been removed, and the
algorithms must not panic. The public signatures do not change. Do not
special-case removed-index graphs; fix the underlying cause so any valid graph
is handled.

Grading rebuilds the crate from your `src/` in a clean environment, supplies a
held-out set of tests for both algorithms on graphs with removed nodes and
edges, and runs the existing max-flow tests as a regression wall. A fix that
passes the held-out tests but breaks an existing test does not count. Only
changes under `src/` are graded.
