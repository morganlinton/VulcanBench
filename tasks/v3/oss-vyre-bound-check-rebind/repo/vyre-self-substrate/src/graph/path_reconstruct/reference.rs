#[cfg(test)]
pub(crate) use vyre_primitives::graph::path_reconstruct::cpu_ref as path_reconstruct_cpu;

/// Reconstruct the path from `target` to its root, writing the
/// `(target, parent, ..., root)` sequence into `scratch`.
#[must_use]
#[cfg(test)]
pub fn reference_reconstruct_path(
    parent: &[u32],
    target: u32,
    max_depth: u32,
    scratch: &mut Vec<u32>,
) -> u32 {
    use crate::observability::{bump, graph_dispatch_calls};
    bump(&graph_dispatch_calls);
    path_reconstruct_cpu(parent, target, max_depth, scratch)
}

/// Convenience wrapper returning the reconstructed path truncated to length.
#[must_use]
#[cfg(test)]
pub fn reference_path_to_root(parent: &[u32], target: u32, max_depth: u32) -> Vec<u32> {
    let mut scratch = Vec::with_capacity(max_depth as usize);
    let len = reference_reconstruct_path(parent, target, max_depth, &mut scratch);
    scratch.truncate(len as usize);
    scratch
}
