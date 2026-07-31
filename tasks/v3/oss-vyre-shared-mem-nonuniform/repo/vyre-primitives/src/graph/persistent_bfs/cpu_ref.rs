#[cfg(any(test, feature = "cpu-parity"))]
use super::validate::validate_persistent_bfs_inputs;

/// CPU reference: run BFS up to `max_iters` steps, accumulating into a
/// running bitset.  Returns the final frontier and a sticky `changed`
/// flag (`1` if any step added new nodes, else `0`).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> (Vec<u32>, u32) {
    try_cpu_ref(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
        max_iters,
    )
    .expect(
        "Fix: reject malformed CSR/frontier via try_cpu_ref; parity wrappers must not pass hostile layouts",
    )
}

/// Fallible CPU reference for persistent BFS.
///
/// This is the primitive-owned entry point for parity wrappers that must reject
/// hostile CSR/frontier inputs without panicking.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    max_iters: u32,
) -> Result<(Vec<u32>, u32), String> {
    let mut out = Vec::new();
    let changed = try_cpu_ref_into(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
        max_iters,
        &mut out,
    )?;
    Ok((out, changed))
}

/// Caller-owned workspace for repeated persistent-BFS CPU oracle runs.
///
/// Conformance and CUDA parity sweeps call this oracle across large generated
/// graph corpora. Reusing the per-iteration frontier scratch avoids a heap
/// allocation per proof case while preserving the allocating compatibility API.
#[cfg(any(test, feature = "cpu-parity"))]
#[derive(Debug, Default, Clone)]
pub(crate) struct PersistentBfsCpuScratch {
    /// Temporary frontier produced by one CSR expansion step.
    pub step: Vec<u32>,
}

#[cfg(any(test, feature = "cpu-parity"))]
impl PersistentBfsCpuScratch {
    /// Create an empty reusable persistent-BFS workspace.
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

/// CPU reference into caller-owned output storage.
///
/// Runs BFS up to `max_iters` steps, accumulating into `frontier_out`. Returns
/// a sticky changed flag (`1` if any step added new nodes, else `0`).
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn cpu_ref_into(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    max_iters: u32,
    frontier_out: &mut Vec<u32>,
) -> u32 {
    let mut scratch = PersistentBfsCpuScratch::default();
    try_cpu_ref_into_with_scratch(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
        max_iters,
        frontier_out,
        &mut scratch,
    )
    .expect(
        "Fix: reject malformed CSR/frontier via try_cpu_ref_into; parity wrappers must not pass hostile layouts",
    )
}

/// Fallible CPU reference into caller-owned output storage.
///
/// On error, `frontier_out` is left unchanged. This lets integration tests and
/// dispatch wrappers treat malformed graph/frontier data as a typed finding
/// instead of a panic or partially clobbered oracle output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    max_iters: u32,
    frontier_out: &mut Vec<u32>,
) -> Result<u32, String> {
    let mut scratch = PersistentBfsCpuScratch::default();
    try_cpu_ref_into_with_scratch(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        allow_mask,
        max_iters,
        frontier_out,
        &mut scratch,
    )
}

/// Fallible CPU reference into caller-owned output and scratch storage.
///
/// On validation error, `frontier_out` and `scratch` are left unchanged. This
/// lets integration tests and dispatch wrappers treat malformed graph/frontier
/// data as a typed finding instead of a panic or partially clobbered oracle
/// state.
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn try_cpu_ref_into_with_scratch(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
    max_iters: u32,
    frontier_out: &mut Vec<u32>,
    scratch: &mut PersistentBfsCpuScratch,
) -> Result<u32, String> {
    let layout = validate_persistent_bfs_inputs(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
    )?;
    let words = layout.words;
    crate::graph::scratch::reserve_graph_items(
        frontier_out,
        words,
        "persistent BFS CPU oracle",
        "frontier output",
    )?;
    crate::graph::scratch::reserve_graph_items(
        &mut scratch.step,
        words,
        "persistent BFS CPU oracle",
        "per-iteration frontier scratch",
    )?;
    frontier_out.clear();
    frontier_out.extend_from_slice(frontier_in);
    frontier_out.resize(words, 0);
    scratch.step.clear();
    scratch.step.resize(words, 0);
    let mut changed = 0u32;

    for _ in 0..max_iters {
        crate::graph::csr_forward_traverse::cpu_ref_into(
            node_count,
            edge_offsets,
            edge_targets,
            edge_kind_mask,
            frontier_out,
            allow_mask,
            &mut scratch.step,
        );
        let mut step_changed = false;
        for w in 0..words {
            let old = frontier_out[w];
            frontier_out[w] |= scratch.step[w];
            if frontier_out[w] != old {
                step_changed = true;
            }
        }
        if step_changed {
            changed = 1;
        } else {
            break;
        }
    }
    Ok(changed)
}
