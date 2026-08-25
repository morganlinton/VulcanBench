use std::collections::HashSet;

use vyre_primitives::graph::reachable::{reachable as reachable_cpu, UnknownNode};
use vyre_primitives::graph::toposort::{toposort as toposort_cpu, ToposortError};

/// Topologically sort `(node_count, edges)`. Edges encode "from
/// depends on to", so `to` is emitted before `from`. Bumps the
/// topological-sort substrate counter so graph ordering traffic is
/// visible independently from dataflow fixpoint traffic.
///
/// # Errors
///
/// Forwards `ToposortError::Cycle` when the graph has a cycle and
/// `ToposortError::UnknownNode` when an edge references an
/// out-of-range node id.
#[cfg(test)]
pub fn reference_topo_order(
    node_count: u32,
    edges: &[(u32, u32)],
) -> Result<Vec<u32>, ToposortError> {
    use crate::observability::{bump, toposort_calls};
    bump(&toposort_calls);
    toposort_cpu(node_count, edges)
}

/// Compute the set of nodes reachable from `sources` over `edges`.
/// Bumps the topological-sort substrate counter because this is the
/// reachability half of the same graph-ordering wrapper.
///
/// # Errors
///
/// Returns `UnknownNode` when an edge names a node id outside
/// `0..node_count`.
#[cfg(test)]
pub fn reference_reachable_set(
    node_count: u32,
    edges: &[(u32, u32)],
    sources: &[u32],
) -> Result<HashSet<u32>, UnknownNode> {
    use crate::observability::{bump, toposort_calls};
    bump(&toposort_calls);
    reachable_cpu(node_count, edges, sources)
}

/// Convenience: returns true iff every node in `targets` is in the
/// reachable set of `sources`. Useful for "would running pass set
/// S leave every required predecessor satisfied?" queries.
#[cfg(test)]
pub fn reference_all_reachable(
    node_count: u32,
    edges: &[(u32, u32)],
    sources: &[u32],
    targets: &[u32],
) -> Result<bool, UnknownNode> {
    let reach = reference_reachable_set(node_count, edges, sources)?;
    Ok(targets.iter().all(|t| reach.contains(t)))
}
