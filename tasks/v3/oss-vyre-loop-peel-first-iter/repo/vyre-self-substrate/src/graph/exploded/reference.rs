use vyre_primitives::graph::exploded::{canonicalize_csr_within_rows, try_build_cpu_reference};

/// Build an exploded supergraph and return its CSR `(row_ptr, col_idx)`.
/// Inputs match the underlying primitive's contract; the wrapper bumps
/// the dataflow-fixpoint observability counter so dispatch-time IFDS
/// graph builds are visible in dashboards.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_build_ifds_csr(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_edges: &[(u32, u32, u32)],
    inter_edges: &[(u32, u32, u32, u32)],
    flow_gen: &[(u32, u32, u32)],
    flow_kill: &[(u32, u32, u32)],
) -> (Vec<u32>, Vec<u32>) {
    try_reference_build_ifds_csr(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_edges,
        inter_edges,
        flow_gen,
        flow_kill,
    )
    .unwrap_or_else(|err| panic!("exploded IFDS self-substrate reference rejected input. {err}"))
}

/// Fallible exploded-supergraph CPU reference wrapper.
///
/// This is the substrate boundary API for hostile-input tests and dispatch
/// adapters: the primitive owns graph construction, while self-substrate only
/// adds observability and typed error propagation.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_reference_build_ifds_csr(
    num_procs: u32,
    blocks_per_proc: u32,
    facts_per_proc: u32,
    intra_edges: &[(u32, u32, u32)],
    inter_edges: &[(u32, u32, u32, u32)],
    flow_gen: &[(u32, u32, u32)],
    flow_kill: &[(u32, u32, u32)],
) -> Result<(Vec<u32>, Vec<u32>), String> {
    use crate::observability::{bump, graph_dispatch_calls};
    bump(&graph_dispatch_calls);
    try_build_cpu_reference(
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_edges,
        inter_edges,
        flow_gen,
        flow_kill,
    )
}

/// Sort each row's column indices in ascending order. Pure CPU helper
/// used by parity tests to compare CSRs whose row contents may have
/// been emitted in different orders by parallel kernels.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_canonicalize_csr_within_rows(
    row_ptr: &[u32],
    col_idx: &[u32],
) -> (Vec<u32>, Vec<u32>) {
    canonicalize_csr_within_rows(row_ptr, col_idx)
}
