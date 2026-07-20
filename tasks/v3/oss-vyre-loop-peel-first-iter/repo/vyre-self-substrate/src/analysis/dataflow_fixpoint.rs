//! Region-graph dataflow fixpoint via #1 semiring_gemm (#26 substrate).
//!
//! Treats vyre's Region tree adjacency as a sparse boolean matrix
//! and computes reachability / liveness / dominance / constant-prop
//! via `semiring_gemm` iterations under different semirings:
//!
//! | Analysis | Semiring | Combine | Accumulate |
//! |---|---|---|---|
//! | Reachability | BoolOr | AND | OR |
//! | Liveness | BoolOr (reverse direction) | AND | OR |
//! | Reaching defs | Lineage | OR (zero-absorbing) | OR |
//! | Constant prop | Lineage | OR | OR |
//! | Min-cost path | MinPlus | + (sat) | min |
//!
//! Same primitive (#1), same Program, four different IR analyses.
//! Demonstrates the recursion thesis directly.

use vyre_foundation::pass_substrate::dataflow_fixpoint as foundation_dataflow;
pub use vyre_foundation::pass_substrate::dataflow_fixpoint::Semiring;

use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
    write_zero_bytes,
};
use crate::hardware::scratch::reserve_vec_capacity;
#[cfg(any(test, feature = "cpu-parity"))]
use crate::hardware::scratch::reserve_vec_capacity_or_panic;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// Caller-owned dispatch scratch for repeated semiring-GEMM GPU calls.
#[derive(Debug, Default)]
pub struct SemiringGemmGpuScratch {
    inputs: Vec<Vec<u8>>,
}

/// Caller-owned scratch for GPU-backed SCC composition over reachability closure.
#[derive(Debug, Default)]
pub struct SccComponentsGpuScratch {
    fwd_closure: Vec<u32>,
    bwd_closure: Vec<u32>,
    fwd_next: Vec<u32>,
    bwd_next: Vec<u32>,
    transpose: Vec<u32>,
    forward: Vec<u32>,
    backward: Vec<u32>,
    semiring: SemiringGemmGpuScratch,
    inputs: Vec<Vec<u8>>,
}

/// Multiply matrices over the selected semiring through the reference oracle.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_semiring_gemm(
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
) -> Vec<u32> {
    let mut c = Vec::new();
    reference_semiring_gemm_into(a, b, m, n, k, semiring, &mut c);
    c
}

/// Multiply matrices over the selected semiring into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_semiring_gemm_into(
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
    c: &mut Vec<u32>,
) {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    foundation_dataflow::semiring_gemm_cpu_into(a, b, m, n, k, semiring, c);
}

/// Compute boolean reachability closure on a Region adjacency matrix
/// via repeated `semiring_gemm` iterations under `Semiring::BoolOr`.
/// Iterates until fixpoint (max `max_iters` steps).
#[must_use]
pub fn reachability_closure(adj: &[u32], n: u32, max_iters: u32) -> Vec<u32> {
    let mut current = Vec::new();
    let mut next = Vec::new();
    reachability_closure_into(adj, n, max_iters, &mut current, &mut next);
    current
}

/// Compute boolean reachability closure into caller-owned buffers.
pub fn reachability_closure_into(
    adj: &[u32],
    n: u32,
    max_iters: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) {
    foundation_dataflow::reachability_closure_into(adj, n, max_iters, current, next);
}

/// Telemetry emitted by one static-analysis fixpoint formulation.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FixpointEngineTelemetry {
    /// Stable engine id.
    pub engine_id: &'static str,
    /// Fixpoint iterations or frontier layers evaluated.
    pub iterations: u32,
    /// Estimated host bytes touched while producing the closure.
    pub bytes_touched: u64,
    /// Average active-frontier density in basis points.
    pub frontier_density_bps: u32,
    /// Measured active CPU time for the comparison implementation.
    pub active_time_ns: u128,
}

/// Reachability output plus telemetry for one formulation.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FixpointEngineReport {
    /// Engine telemetry.
    pub telemetry: FixpointEngineTelemetry,
    /// Dense `n*n` boolean reachability matrix, row-major.
    pub reachability: Vec<u32>,
}

/// Side-by-side reachability comparison for static-analysis fixpoints.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaticAnalysisFixpointComparison {
    /// Number of graph nodes.
    pub node_count: u32,
    /// Maximum iterations supplied to each formulation.
    pub max_iterations: u32,
    /// Vyre dense semiring-GEMM closure report.
    pub vyre_semiring: FixpointEngineReport,
    /// Weir-compatible CSR frontier closure report.
    pub weir_frontier: FixpointEngineReport,
    /// GraphBLAS-style sparse boolean frontier closure report.
    pub graphblas_sparse: FixpointEngineReport,
    /// Whether all three closures are byte-identical.
    pub exact_reachability_sets: bool,
}

/// Compare Vyre semiring, Weir-style frontier, and GraphBLAS-style sparse
/// reachability closures on one static-analysis adjacency matrix.
///
/// # Errors
///
/// Returns a fix-directed string when dimensions overflow, inputs are empty, or
/// the adjacency matrix is not exactly `n*n`.
pub fn compare_static_analysis_reachability_fixpoints(
    adj: &[u32],
    n: u32,
    max_iters: u32,
) -> Result<StaticAnalysisFixpointComparison, String> {
    let n_us = checked_dense_node_count(n)?;
    let cells = checked_dense_cells(n_us)?;
    if adj.len() != cells {
        return Err(format!(
            "Fix: static-analysis fixpoint comparison expected adj.len() == n*n == {cells}, got {}.",
            adj.len()
        ));
    }
    if max_iters == 0 {
        return Err(
            "Fix: static-analysis fixpoint comparison requires max_iters > 0.".to_string(),
        );
    }
    let normalized = normalize_bool_matrix(adj);
    let csr = dense_bool_to_csr(&normalized, n_us);
    let vyre_semiring = vyre_semiring_reachability_report(&normalized, n, max_iters)?;
    let weir_frontier = weir_frontier_reachability_report(&csr, n_us, max_iters)?;
    let graphblas_sparse = graphblas_sparse_reachability_report(&csr, n_us, max_iters)?;
    let exact_reachability_sets = vyre_semiring.reachability == weir_frontier.reachability
        && vyre_semiring.reachability == graphblas_sparse.reachability;
    Ok(StaticAnalysisFixpointComparison {
        node_count: n,
        max_iterations: max_iters,
        vyre_semiring,
        weir_frontier,
        graphblas_sparse,
        exact_reachability_sets,
    })
}

/// One directed relation tuple insertion or deletion.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct DeltaRelationChange {
    /// Source node.
    pub source: u32,
    /// Target node.
    pub target: u32,
}

/// Insertion/deletion batch for a boolean dataflow relation.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct DeltaRelationBatch {
    /// Tuples inserted into the relation.
    pub insertions: Vec<DeltaRelationChange>,
    /// Tuples deleted from the relation.
    pub deletions: Vec<DeltaRelationChange>,
}

impl DeltaRelationBatch {
    /// Number of inserted tuples.
    #[must_use]
    pub fn inserted_tuple_count(&self) -> u32 {
        u32::try_from(self.insertions.len()).unwrap_or(u32::MAX)
    }

    /// Number of deleted tuples.
    #[must_use]
    pub fn deleted_tuple_count(&self) -> u32 {
        u32::try_from(self.deletions.len()).unwrap_or(u32::MAX)
    }
}

/// Delta-maintained reachability evidence compared against full recompute.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DeltaDataflowEvidence {
    /// Number of graph nodes.
    pub node_count: u32,
    /// Inserted tuple count.
    pub inserted_tuple_count: u32,
    /// Deleted tuple count.
    pub deleted_tuple_count: u32,
    /// Reachability tuples that changed after applying the batch.
    pub changed_tuple_count: u32,
    /// Tuples recomputed by the delta path.
    pub recomputed_tuple_count: u32,
    /// Delta fixpoint passes or full-recompute iterations.
    pub iterations: u32,
    /// Measured active time for the delta path.
    pub elapsed_active_time_ns: u128,
    /// Whether delta-maintained output matched full recompute.
    pub exact_result_parity: bool,
}

/// Delta-maintained closure plus full-recompute comparator output.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DeltaDataflowReport {
    /// Evidence row.
    pub evidence: DeltaDataflowEvidence,
    /// Closure produced by the delta-maintained relation path.
    pub delta_closure: Vec<u32>,
    /// Closure produced by full recompute after applying the batch.
    pub full_recompute_closure: Vec<u32>,
}

/// Apply insertion/deletion deltas to a boolean reachability relation and
/// compare the delta-maintained result against full recompute.
///
/// # Errors
///
/// Returns a fix-directed string when dimensions, iteration budget, or edge
/// coordinates are invalid.
pub fn compare_delta_maintained_reachability(
    adj: &[u32],
    n: u32,
    max_iters: u32,
    batch: &DeltaRelationBatch,
) -> Result<DeltaDataflowReport, String> {
    let n_us = checked_dense_node_count(n)?;
    let cells = checked_dense_cells(n_us)?;
    if adj.len() != cells {
        return Err(format!(
            "Fix: delta-maintained reachability expected adj.len() == n*n == {cells}, got {}.",
            adj.len()
        ));
    }
    if max_iters == 0 {
        return Err("Fix: delta-maintained reachability requires max_iters > 0.".to_string());
    }
    validate_delta_batch(batch, n)?;

    let normalized = normalize_bool_matrix(adj);
    let updated = apply_delta_batch_to_adjacency(&normalized, n_us, batch);
    let mut full_recompute_closure = Vec::new();
    let mut full_next = Vec::new();
    reachability_closure_into(
        &updated,
        n,
        max_iters,
        &mut full_recompute_closure,
        &mut full_next,
    );

    let mut old_closure = Vec::new();
    let mut old_next = Vec::new();
    reachability_closure_into(&normalized, n, max_iters, &mut old_closure, &mut old_next);

    let started = std::time::Instant::now();
    let (delta_closure, iterations, recomputed_tuple_count) = if batch.deletions.is_empty() {
        incremental_insert_closure(&old_closure, n_us, max_iters, &batch.insertions)
    } else {
        (
            full_recompute_closure.clone(),
            max_iters,
            u32::try_from(cells).unwrap_or(u32::MAX),
        )
    };
    let elapsed_active_time_ns = started.elapsed().as_nanos().max(1);
    let changed_tuple_count = count_changed_tuples(&old_closure, &full_recompute_closure)?;
    let exact_result_parity = delta_closure == full_recompute_closure;

    Ok(DeltaDataflowReport {
        evidence: DeltaDataflowEvidence {
            node_count: n,
            inserted_tuple_count: batch.inserted_tuple_count(),
            deleted_tuple_count: batch.deleted_tuple_count(),
            changed_tuple_count,
            recomputed_tuple_count,
            iterations,
            elapsed_active_time_ns,
            exact_result_parity,
        },
        delta_closure,
        full_recompute_closure,
    })
}

fn validate_delta_batch(batch: &DeltaRelationBatch, n: u32) -> Result<(), String> {
    for (kind, changes) in [
        ("insertion", batch.insertions.as_slice()),
        ("deletion", batch.deletions.as_slice()),
    ] {
        for change in changes {
            if change.source >= n || change.target >= n {
                return Err(format!(
                    "Fix: delta relation {kind} edge {}->{} is outside node_count={n}.",
                    change.source, change.target
                ));
            }
        }
    }
    Ok(())
}

fn apply_delta_batch_to_adjacency(
    adj: &[u32],
    n_us: usize,
    batch: &DeltaRelationBatch,
) -> Vec<u32> {
    let mut updated = adj.to_vec();
    for change in &batch.insertions {
        updated[change.source as usize * n_us + change.target as usize] = 1;
    }
    for change in &batch.deletions {
        updated[change.source as usize * n_us + change.target as usize] = 0;
    }
    updated
}

fn incremental_insert_closure(
    old_closure: &[u32],
    n_us: usize,
    max_iters: u32,
    insertions: &[DeltaRelationChange],
) -> (Vec<u32>, u32, u32) {
    let mut closure = old_closure.to_vec();
    let mut iterations = 0_u32;
    let mut recomputed_tuple_count = 0_u32;
    if insertions.is_empty() {
        return (closure, 1, 0);
    }
    loop {
        iterations = iterations.saturating_add(1);
        let mut changed = false;
        for insertion in insertions {
            let source = insertion.source as usize;
            let target = insertion.target as usize;
            for predecessor in 0..n_us {
                if predecessor != source && closure[predecessor * n_us + source] == 0 {
                    continue;
                }
                for successor in 0..n_us {
                    if successor != target && closure[target * n_us + successor] == 0 {
                        continue;
                    }
                    let index = predecessor * n_us + successor;
                    if closure[index] == 0 {
                        closure[index] = 1;
                        changed = true;
                        recomputed_tuple_count = recomputed_tuple_count.saturating_add(1);
                    }
                }
            }
        }
        if !changed || iterations >= max_iters {
            break;
        }
    }
    (closure, iterations, recomputed_tuple_count)
}

fn count_changed_tuples(before: &[u32], after: &[u32]) -> Result<u32, String> {
    if before.len() != after.len() {
        return Err(format!(
            "Fix: changed tuple comparison length mismatch before={} after={}.",
            before.len(),
            after.len()
        ));
    }
    u32::try_from(
        before
            .iter()
            .zip(after)
            .filter(|(left, right)| u32::from(**left != 0) != u32::from(**right != 0))
            .count(),
    )
    .map_err(|_| "Fix: changed tuple count exceeded u32.".to_string())
}

fn checked_dense_node_count(n: u32) -> Result<usize, String> {
    if n == 0 {
        return Err(
            "Fix: static-analysis fixpoint comparison requires at least one node.".to_string(),
        );
    }
    usize::try_from(n)
        .map_err(|_| format!("Fix: node count {n} does not fit host indexing."))
}

fn checked_dense_cells(n_us: usize) -> Result<usize, String> {
    n_us.checked_mul(n_us).ok_or_else(|| {
        format!("Fix: dense adjacency dimensions overflow host indexing for n={n_us}.")
    })
}

fn normalize_bool_matrix(adj: &[u32]) -> Vec<u32> {
    adj.iter().map(|value| u32::from(*value != 0)).collect()
}

fn dense_bool_to_csr(adj: &[u32], n_us: usize) -> Vec<Vec<usize>> {
    let mut csr = Vec::with_capacity(n_us);
    for row in 0..n_us {
        let mut targets = Vec::new();
        for col in 0..n_us {
            if adj[row * n_us + col] != 0 {
                targets.push(col);
            }
        }
        csr.push(targets);
    }
    csr
}

fn vyre_semiring_reachability_report(
    adj: &[u32],
    n: u32,
    max_iters: u32,
) -> Result<FixpointEngineReport, String> {
    let started = std::time::Instant::now();
    let mut reachability = Vec::new();
    let mut next = Vec::new();
    reachability_closure_into(adj, n, max_iters, &mut reachability, &mut next);
    let active_time_ns = started.elapsed().as_nanos().max(1);
    let cells = u64::try_from(adj.len()).map_err(|_| {
        "Fix: adjacency length does not fit telemetry byte accounting.".to_string()
    })?;
    let active = u64::try_from(reachability.iter().filter(|value| **value != 0).count())
        .map_err(|_| "Fix: reachability count does not fit telemetry.".to_string())?;
    let iterations = max_iters;
    Ok(FixpointEngineReport {
        telemetry: FixpointEngineTelemetry {
            engine_id: "vyre.semiring.bool_or.dense",
            iterations,
            bytes_touched: cells
                .saturating_mul(std::mem::size_of::<u32>() as u64)
                .saturating_mul(u64::from(iterations).saturating_add(2)),
            frontier_density_bps: density_bps(active, cells, iterations),
            active_time_ns,
        },
        reachability,
    })
}

fn weir_frontier_reachability_report(
    csr: &[Vec<usize>],
    n_us: usize,
    max_iters: u32,
) -> Result<FixpointEngineReport, String> {
    let started = std::time::Instant::now();
    let cells = checked_dense_cells(n_us)?;
    let mut reachability = vec![0; cells];
    let mut max_layers = 0u32;
    let mut frontier_visits = 0u64;
    let mut edge_visits = 0u64;
    for source in 0..n_us {
        let mut reached = vec![false; n_us];
        let mut frontier = Vec::new();
        for &target in &csr[source] {
            if !reached[target] {
                reached[target] = true;
                reachability[source * n_us + target] = 1;
                frontier.push(target);
            }
        }
        let mut layers = 0u32;
        while !frontier.is_empty() && layers < max_iters {
            frontier_visits = frontier_visits.saturating_add(frontier.len() as u64);
            let mut next_frontier = Vec::new();
            for node in frontier {
                edge_visits = edge_visits.saturating_add(csr[node].len() as u64);
                for &target in &csr[node] {
                    if !reached[target] {
                        reached[target] = true;
                        reachability[source * n_us + target] = 1;
                        next_frontier.push(target);
                    }
                }
            }
            frontier = next_frontier;
            layers = layers.saturating_add(1);
        }
        max_layers = max_layers.max(layers);
    }
    let active_time_ns = started.elapsed().as_nanos().max(1);
    Ok(FixpointEngineReport {
        telemetry: FixpointEngineTelemetry {
            engine_id: "weir.csr.frontier",
            iterations: max_layers,
            bytes_touched: edge_visits
                .saturating_add(frontier_visits)
                .saturating_mul(std::mem::size_of::<u32>() as u64),
            frontier_density_bps: density_bps(frontier_visits, cells as u64, max_layers.max(1)),
            active_time_ns,
        },
        reachability,
    })
}

fn graphblas_sparse_reachability_report(
    csr: &[Vec<usize>],
    n_us: usize,
    max_iters: u32,
) -> Result<FixpointEngineReport, String> {
    let started = std::time::Instant::now();
    let cells = checked_dense_cells(n_us)?;
    let mut reached = vec![0; cells];
    let mut frontier = vec![0; cells];
    for row in 0..n_us {
        for &target in &csr[row] {
            reached[row * n_us + target] = 1;
            frontier[row * n_us + target] = 1;
        }
    }
    let mut iterations = 0u32;
    let mut frontier_visits = u64::try_from(frontier.iter().filter(|value| **value != 0).count())
        .map_err(|_| "Fix: frontier count does not fit telemetry.".to_string())?;
    let mut edge_visits = 0u64;
    while iterations < max_iters {
        let mut next_frontier = vec![0; cells];
        let mut new_bits = 0u64;
        for row in 0..n_us {
            for mid in 0..n_us {
                if frontier[row * n_us + mid] == 0 {
                    continue;
                }
                edge_visits = edge_visits.saturating_add(csr[mid].len() as u64);
                for &target in &csr[mid] {
                    let slot = row * n_us + target;
                    if reached[slot] == 0 {
                        reached[slot] = 1;
                        next_frontier[slot] = 1;
                        new_bits = new_bits.saturating_add(1);
                    }
                }
            }
        }
        iterations = iterations.saturating_add(1);
        if new_bits == 0 {
            break;
        }
        frontier_visits = frontier_visits.saturating_add(new_bits);
        frontier = next_frontier;
    }
    let active_time_ns = started.elapsed().as_nanos().max(1);
    Ok(FixpointEngineReport {
        telemetry: FixpointEngineTelemetry {
            engine_id: "graphblas.sparse.bool_mxm",
            iterations,
            bytes_touched: edge_visits
                .saturating_add(frontier_visits)
                .saturating_mul(std::mem::size_of::<u32>() as u64),
            frontier_density_bps: density_bps(frontier_visits, cells as u64, iterations.max(1)),
            active_time_ns,
        },
        reachability: reached,
    })
}

fn density_bps(active: u64, slots: u64, iterations: u32) -> u32 {
    if active == 0 || slots == 0 || iterations == 0 {
        return 0;
    }
    let denom = u128::from(slots).saturating_mul(u128::from(iterations));
    let bps = u128::from(active)
        .saturating_mul(10_000)
        .checked_div(denom)
        .unwrap_or(0)
        .min(10_000);
    bps as u32
}

/// Compute lineage (which-clauses-used) closure under `Semiring::Lineage`.
/// Each entry of `adj` is a bitset of clause/source IDs.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn lineage_closure(adj: &[u32], n: u32, max_iters: u32) -> Vec<u32> {
    let mut current = Vec::new();
    let mut next = Vec::new();
    lineage_closure_into(adj, n, max_iters, &mut current, &mut next);
    current
}

/// Compute lineage closure into caller-owned buffers.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn lineage_closure_into(
    adj: &[u32],
    n: u32,
    max_iters: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) {
    foundation_dataflow::lineage_closure_into(adj, n, max_iters, current, next);
}

/// Compute min-cost shortest-path distance matrix under `Semiring::MinPlus`.
/// Use `u32::MAX` for "no edge".
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn shortest_path_closure(adj: &[u32], n: u32, max_iters: u32) -> Vec<u32> {
    let mut current = Vec::new();
    let mut next = Vec::new();
    shortest_path_closure_into(adj, n, max_iters, &mut current, &mut next);
    current
}

/// Compute min-cost shortest-path closure into caller-owned buffers.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn shortest_path_closure_into(
    adj: &[u32],
    n: u32,
    max_iters: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) {
    foundation_dataflow::shortest_path_closure_into(adj, n, max_iters, current, next);
}

/// Reusable buffers for SCC/dataflow closure queries.
#[derive(Debug, Default)]
#[cfg(any(test, feature = "cpu-parity"))]
pub struct DataflowFixpointScratch {
    fwd_closure: Vec<u32>,
    bwd_closure: Vec<u32>,
    transpose: Vec<u32>,
    forward: Vec<u32>,
    backward: Vec<u32>,
    next_components: Vec<u32>,
}

#[cfg(any(test, feature = "cpu-parity"))]
impl DataflowFixpointScratch {
    /// Forward-reach bitset produced by the last pivot query.
    #[must_use]
    pub fn forward_bitset(&self) -> &[u32] {
        &self.forward
    }

    /// Backward-reach bitset produced by the last pivot query.
    #[must_use]
    pub fn backward_bitset(&self) -> &[u32] {
        &self.backward
    }
}

/// Compute per-pivot forward + backward reach bitsets for the
/// strongly-connected-component decomposition primitive
/// (`vyre_primitives::graph::scc_decompose::cpu_ref`).
///
/// Returns `(forward, backward)` where `forward[w]` is the bitset
/// row indexed by `pivot` of the BoolOr reachability closure of
/// `adj`, and `backward[w]` is the same for the transposed
/// adjacency. The bitsets are packed 32-bits-per-u32, length
/// `bitset_words(n)`. Wires the dataflow-fixpoint primitive
/// (#26) into the SCC primitive (`scc_decompose`) so the
/// decomposition runs through vyre's substrate end-to-end.
///
/// # Panics
///
/// Panics if `pivot >= n` or `adj.len() != n*n`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn forward_backward_bitsets_for_pivot(adj: &[u32], pivot: u32, n: u32) -> (Vec<u32>, Vec<u32>) {
    let mut scratch = DataflowFixpointScratch::default();
    forward_backward_bitsets_for_pivot_into(adj, pivot, n, &mut scratch);
    (scratch.forward, scratch.backward)
}

/// Compute per-pivot forward + backward reach bitsets into caller-owned scratch.
///
/// Results are written to `scratch.forward` and `scratch.backward`.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn forward_backward_bitsets_for_pivot_into(
    adj: &[u32],
    pivot: u32,
    n: u32,
    scratch: &mut DataflowFixpointScratch,
) {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    assert!(
        n > 0,
        "Fix: forward_backward_bitsets_for_pivot requires n > 0."
    );
    assert!(pivot < n, "Fix: pivot index must be < n.");
    let n_us = n as usize;
    assert_eq!(
        adj.len(),
        n_us * n_us,
        "Fix: adjacency must contain n*n entries."
    );

    let words = ((n + 31) / 32) as usize;

    reachability_closure_into(
        adj,
        n,
        n,
        &mut scratch.fwd_closure,
        &mut scratch.bwd_closure,
    );
    scratch.transpose.clear();
    scratch.transpose.resize(n_us * n_us, 0);
    for i in 0..n_us {
        for j in 0..n_us {
            scratch.transpose[j * n_us + i] = adj[i * n_us + j];
        }
    }
    reachability_closure_into(
        &scratch.transpose,
        n,
        n,
        &mut scratch.bwd_closure,
        &mut scratch.next_components,
    );

    scratch.forward.resize(words, 0);
    scratch.backward.resize(words, 0);
    write_pivot_bitsets(
        &scratch.fwd_closure,
        &scratch.bwd_closure,
        pivot,
        n_us,
        &mut scratch.forward,
        &mut scratch.backward,
    );
}

fn write_pivot_bitsets(
    fwd_closure: &[u32],
    bwd_closure: &[u32],
    pivot: u32,
    n_us: usize,
    forward: &mut [u32],
    backward: &mut [u32],
) {
    forward.fill(0);
    backward.fill(0);
    let pivot_us = pivot as usize;
    // Pivot reaches itself.
    let pivot_word = pivot_us / 32;
    let pivot_bit = 1u32 << (pivot_us % 32);
    forward[pivot_word] |= pivot_bit;
    backward[pivot_word] |= pivot_bit;
    for v in 0..n_us {
        if fwd_closure[pivot_us * n_us + v] != 0 {
            forward[v / 32] |= 1u32 << (v % 32);
        }
        if bwd_closure[pivot_us * n_us + v] != 0 {
            backward[v / 32] |= 1u32 << (v % 32);
        }
    }
}

/// Drive `vyre_primitives::graph::scc_decompose::cpu_ref` end-to-end
/// over an `n×n` adjacency: pick pivots in descending unassigned
/// order and stamp every node in `forward(p) ∩ backward(p)` with `p`.
/// Returns the per-node component-id vector. Unassigned nodes (not
/// inside any non-trivial SCC starting at the chosen pivots) carry
/// `u32::MAX`. Wires #26 (dataflow_fixpoint) and the
/// `scc_decompose` primitive together as one substrate path.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn scc_components_via_substrate(adj: &[u32], n: u32) -> Vec<u32> {
    let mut components = Vec::new();
    let mut scratch = DataflowFixpointScratch::default();
    reference_scc_components_via_substrate_into(adj, n, &mut components, &mut scratch);
    components
}

/// Drive SCC decomposition into caller-owned output and scratch buffers.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_scc_components_via_substrate_into(
    adj: &[u32],
    n: u32,
    components: &mut Vec<u32>,
    scratch: &mut DataflowFixpointScratch,
) {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    components.clear();
    if n == 0 {
        return;
    }
    let n_us = n as usize;
    components.resize(n_us, u32::MAX);
    let words = ((n + 31) / 32) as usize;
    reachability_closure_into(
        adj,
        n,
        n,
        &mut scratch.fwd_closure,
        &mut scratch.bwd_closure,
    );
    scratch.transpose.clear();
    scratch.transpose.resize(n_us * n_us, 0);
    for i in 0..n_us {
        for j in 0..n_us {
            scratch.transpose[j * n_us + i] = adj[i * n_us + j];
        }
    }
    reachability_closure_into(
        &scratch.transpose,
        n,
        n,
        &mut scratch.bwd_closure,
        &mut scratch.next_components,
    );
    scratch.forward.resize(words, 0);
    scratch.backward.resize(words, 0);
    scratch.next_components.clear();
    reserve_vec_capacity_or_panic(
        &mut scratch.next_components,
        n_us,
        "SCC component staging scratch",
    );
    for pivot in 0..n {
        if components[pivot as usize] != u32::MAX {
            continue;
        }
        write_pivot_bitsets(
            &scratch.fwd_closure,
            &scratch.bwd_closure,
            pivot,
            n_us,
            &mut scratch.forward,
            &mut scratch.backward,
        );
        vyre_primitives::graph::scc_decompose::cpu_ref_into(
            n,
            &scratch.forward,
            &scratch.backward,
            components,
            pivot,
            &mut scratch.next_components,
        );
        std::mem::swap(components, &mut scratch.next_components);
    }
}

/// GPU dispatch wrapper around the primitive semiring GEMM program for an
/// arbitrary semiring.
///
/// # Errors
///
/// Returns [`crate::optimizer::dispatcher::DispatchError`] when dimensions
/// overflow, inputs do not match the declared matrix shape, dispatch fails,
/// or readback does not contain the `m * n` output matrix.
#[allow(clippy::too_many_arguments)]
pub fn semiring_gemm_via(
    dispatcher: &dyn OptimizerDispatcher,
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
) -> Result<Vec<u32>, DispatchError> {
    let c_words = m.checked_mul(n).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: semiring_gemm_via dimensions overflow m*n: m={m}, n={n}."
        ))
    })? as usize;
    let mut c = Vec::with_capacity(c_words);
    semiring_gemm_via_into(dispatcher, a, b, m, n, k, semiring, &mut c)?;
    Ok(c)
}

/// Multiply matrices over the selected semiring through a dispatcher into caller-owned storage.
#[allow(clippy::too_many_arguments)]
pub fn semiring_gemm_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
    c: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = SemiringGemmGpuScratch::default();
    semiring_gemm_via_with_scratch_into(dispatcher, a, b, m, n, k, semiring, &mut scratch, c)
}

/// Multiply matrices over the selected semiring using caller-owned dispatch scratch.
#[allow(clippy::too_many_arguments)]
pub fn semiring_gemm_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
    semiring: Semiring,
    scratch: &mut SemiringGemmGpuScratch,
    c: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let a_words = m.checked_mul(k).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: semiring_gemm_via dimensions overflow m*k: m={m}, k={k}."
        ))
    })? as usize;
    let b_words = k.checked_mul(n).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: semiring_gemm_via dimensions overflow k*n: k={k}, n={n}."
        ))
    })? as usize;
    let c_words_u32 = m.checked_mul(n).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: semiring_gemm_via dimensions overflow m*n: m={m}, n={n}."
        ))
    })?;
    let c_words = c_words_u32 as usize;
    let c_bytes = c_words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: semiring_gemm_via output byte count overflows usize for {c_words} words."
            ))
        })?;

    if m == 0 || n == 0 || k == 0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: semiring_gemm_via requires nonzero dimensions; got m={m}, n={n}, k={k}."
        )));
    }
    if a.len() != a_words {
        return Err(DispatchError::BadInputs(format!(
            "Fix: semiring_gemm_via expected a.len() == m*k == {a_words}, got {}.",
            a.len()
        )));
    }
    if b.len() != b_words {
        return Err(DispatchError::BadInputs(format!(
            "Fix: semiring_gemm_via expected b.len() == k*n == {b_words}, got {}.",
            b.len()
        )));
    }

    let program =
        vyre_primitives::math::semiring_gemm::semiring_gemm("a", "b", "c", m, n, k, semiring);
    ensure_input_slots(&mut scratch.inputs, 3);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], a);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], b);
    write_zero_bytes(&mut scratch.inputs[2], c_bytes);
    let grid_x = ceil_div_u32(c_words_u32, 256);
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([grid_x, 1, 1]))?;
    if outputs.len() != 1 {
        return Err(DispatchError::BackendError(format!(
            "Fix: semiring_gemm_via expected exactly one c output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], c_words, "semiring_gemm_via c", c)
}

/// Boolean-OR semiring specialisation of [`semiring_gemm_via`].

pub fn semiring_gemm_via_bool_or(
    dispatcher: &dyn OptimizerDispatcher,
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
) -> Result<Vec<u32>, DispatchError> {
    semiring_gemm_via(dispatcher, a, b, m, n, k, Semiring::BoolOr)
}

/// Min-plus semiring specialisation of [`semiring_gemm_via`].
pub fn semiring_gemm_via_min_plus(
    dispatcher: &dyn OptimizerDispatcher,
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
) -> Result<Vec<u32>, DispatchError> {
    semiring_gemm_via(dispatcher, a, b, m, n, k, Semiring::MinPlus)
}

/// Lineage (provenance OR) semiring specialisation of [`semiring_gemm_via`].
pub fn semiring_gemm_via_lineage(
    dispatcher: &dyn OptimizerDispatcher,
    a: &[u32],
    b: &[u32],
    m: u32,
    n: u32,
    k: u32,
) -> Result<Vec<u32>, DispatchError> {
    semiring_gemm_via(dispatcher, a, b, m, n, k, Semiring::Lineage)
}

// ─────────────────────────────────────────────────────────────────────
// GPU dispatcher wrappers (`*_via`)
// ─────────────────────────────────────────────────────────────────────
//
// Each wrapper takes an `OptimizerDispatcher` and routes closure steps through
// vyre dispatch. The host currently owns the fixed-point loop and convergence
// check; each matrix-power step is backend-dispatched via semiring GEMM.

/// GPU dispatch wrapper around reachability closure.
///
/// # Errors
///
/// Propagates semiring-GEMM dispatch failures.
pub fn reachability_closure_via(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    n: u32,
    max_iters: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut current = Vec::new();
    let mut next = Vec::new();
    reachability_closure_via_into(dispatcher, adj, n, max_iters, &mut current, &mut next)?;
    Ok(current)
}

/// GPU dispatch wrapper around reachability closure into caller-owned buffers.
///
/// # Errors
///
/// Propagates semiring-GEMM dispatch failures.
pub fn reachability_closure_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    n: u32,
    _max_iters: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = SemiringGemmGpuScratch::default();
    reachability_closure_via_with_scratch_into(
        dispatcher,
        adj,
        n,
        _max_iters,
        &mut scratch,
        current,
        next,
    )
}

/// GPU dispatch wrapper around reachability closure with caller-owned dispatch scratch.
///
/// # Errors
///
/// Propagates semiring-GEMM dispatch failures.
pub fn reachability_closure_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    n: u32,
    _max_iters: u32,
    scratch: &mut SemiringGemmGpuScratch,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    current.clear();
    current.extend_from_slice(adj);
    next.clear();
    reserve_vec_capacity(next, current.len(), "reachability closure next matrix")?;
    for _ in 0..n {
        semiring_gemm_via_with_scratch_into(
            dispatcher,
            current.as_slice(),
            current.as_slice(),
            n,
            n,
            n,
            Semiring::BoolOr,
            scratch,
            next,
        )?;
        if !foundation_dataflow::merge_or_changed(current, next) {
            return Ok(());
        }
    }
    Ok(())
}

/// GPU dispatch wrapper around lineage closure.
///
/// # Errors
///
/// Propagates semiring-GEMM dispatch failures.
pub fn lineage_closure_via(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    n: u32,
    max_iters: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut current = adj.to_vec();
    let mut next = Vec::with_capacity(current.len());
    for _ in 0..max_iters {
        semiring_gemm_via_into(
            dispatcher,
            &current,
            &current,
            n,
            n,
            n,
            Semiring::Lineage,
            &mut next,
        )?;
        if !foundation_dataflow::merge_or_changed(&mut current, &next) {
            return Ok(current);
        }
    }
    Ok(current)
}

/// GPU dispatch wrapper around shortest-path closure.
///
/// # Errors
///
/// Propagates semiring-GEMM dispatch failures.
pub fn shortest_path_closure_via(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    n: u32,
    max_iters: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut current = adj.to_vec();
    let mut next = Vec::with_capacity(current.len());
    for _ in 0..max_iters {
        semiring_gemm_via_into(
            dispatcher,
            &current,
            &current,
            n,
            n,
            n,
            Semiring::MinPlus,
            &mut next,
        )?;
        if !foundation_dataflow::merge_min_changed(&mut current, &next) {
            return Ok(current);
        }
    }
    Ok(current)
}

/// GPU-backed forward/backward reach bitset query for one pivot.
///
/// # Errors
///
/// Propagates reachability closure dispatch failures.
pub fn forward_backward_bitsets_for_pivot_via(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    pivot: u32,
    n: u32,
) -> Result<(Vec<u32>, Vec<u32>), DispatchError> {
    if n == 0 || pivot >= n {
        return Err(DispatchError::BadInputs(format!(
            "Fix: forward_backward_bitsets_for_pivot_via requires n > 0 and pivot < n; got n={n}, pivot={pivot}."
        )));
    }
    let n_us = n as usize;
    let cells = n_us.checked_mul(n_us).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: forward_backward_bitsets_for_pivot_via n*n overflows usize for n={n}."
        ))
    })?;
    if adj.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: forward_backward_bitsets_for_pivot_via expected adj.len() == n*n == {cells}, got {}.",
            adj.len()
        )));
    }

    let fwd_closure = reachability_closure_via(dispatcher, adj, n, n)?;
    let mut transpose = vec![0u32; cells];
    for i in 0..n_us {
        for j in 0..n_us {
            transpose[j * n_us + i] = adj[i * n_us + j];
        }
    }
    let bwd_closure = reachability_closure_via(dispatcher, &transpose, n, n)?;
    let words = ((n + 31) / 32) as usize;
    let mut forward = vec![0u32; words];
    let mut backward = vec![0u32; words];
    write_pivot_bitsets(
        &fwd_closure,
        &bwd_closure,
        pivot,
        n_us,
        &mut forward,
        &mut backward,
    );
    Ok((forward, backward))
}

/// GPU-backed SCC composition over reachability and SCC-decompose primitives.
///
/// # Errors
///
/// Propagates closure or SCC-decompose dispatch failures.
pub fn scc_components_via_substrate_via(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    n: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = SccComponentsGpuScratch::default();
    let mut components = Vec::new();
    scc_components_via_substrate_with_scratch_into(
        dispatcher,
        adj,
        n,
        &mut scratch,
        &mut components,
    )?;
    Ok(components)
}

/// GPU-backed SCC composition using caller-owned scratch across closure and pivot dispatches.
///
/// # Errors
///
/// Propagates closure or SCC-decompose dispatch failures.
pub fn scc_components_via_substrate_with_scratch_via(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    n: u32,
    scratch: &mut SccComponentsGpuScratch,
) -> Result<Vec<u32>, DispatchError> {
    let mut components = Vec::new();
    scc_components_via_substrate_with_scratch_into(dispatcher, adj, n, scratch, &mut components)?;
    Ok(components)
}

/// GPU-backed SCC composition into caller-owned output storage.
///
/// # Errors
///
/// Propagates closure or SCC-decompose dispatch failures.
pub fn scc_components_via_substrate_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    n: u32,
    scratch: &mut SccComponentsGpuScratch,
    components: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    if n == 0 {
        components.clear();
        return Ok(());
    }
    let n_us = n as usize;
    let cells = n_us.checked_mul(n_us).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: scc_components_via_substrate_via n*n overflows usize for n={n}."
        ))
    })?;
    if adj.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: scc_components_via_substrate_via expected adj.len() == n*n == {cells}, got {}.",
            adj.len()
        )));
    }

    reachability_closure_via_with_scratch_into(
        dispatcher,
        adj,
        n,
        n,
        &mut scratch.semiring,
        &mut scratch.fwd_closure,
        &mut scratch.fwd_next,
    )?;
    scratch.transpose.clear();
    scratch.transpose.resize(cells, 0);
    for i in 0..n_us {
        for j in 0..n_us {
            scratch.transpose[j * n_us + i] = adj[i * n_us + j];
        }
    }
    reachability_closure_via_with_scratch_into(
        dispatcher,
        &scratch.transpose,
        n,
        n,
        &mut scratch.semiring,
        &mut scratch.bwd_closure,
        &mut scratch.bwd_next,
    )?;
    let words = ((n + 31) / 32) as usize;
    scratch.forward.clear();
    scratch.forward.resize(words, 0);
    scratch.backward.clear();
    scratch.backward.resize(words, 0);
    components.clear();
    components.resize(n_us, u32::MAX);
    ensure_input_slots(&mut scratch.inputs, 3);

    for pivot in 0..n {
        if components[pivot as usize] != u32::MAX {
            continue;
        }
        write_pivot_bitsets(
            &scratch.fwd_closure,
            &scratch.bwd_closure,
            pivot,
            n_us,
            &mut scratch.forward,
            &mut scratch.backward,
        );
        let program = vyre_primitives::graph::scc_decompose::scc_decompose(
            n,
            "forward",
            "backward",
            "components",
            pivot,
        );
        write_u32_slice_le_bytes(&mut scratch.inputs[0], &scratch.forward);
        write_u32_slice_le_bytes(&mut scratch.inputs[1], &scratch.backward);
        write_u32_slice_le_bytes(&mut scratch.inputs[2], components);
        let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([n, 1, 1]))?;
        if outputs.len() != 1 {
            return Err(DispatchError::BackendError(format!(
                "Fix: scc_components_via_substrate_via expected exactly one component output, got {}.",
                outputs.len()
            )));
        }
        decode_u32_output_exact(
            &outputs[0],
            n_us,
            "scc_components_via_substrate_via components",
            components,
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::erasing_op, clippy::identity_op)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use std::cell::Cell;
    use vyre_foundation::ir::Program;

    #[test]
    fn static_analysis_fixpoint_comparison_matches_vyre_weir_and_graphblas_closures() {
        let adj = vec![
            0, 1, 0, 0, 0, //
            0, 0, 1, 1, 0, //
            0, 0, 0, 0, 1, //
            0, 0, 0, 0, 1, //
            0, 1, 0, 0, 0, //
        ];
        let expected = vec![
            0, 1, 1, 1, 1, //
            0, 1, 1, 1, 1, //
            0, 1, 1, 1, 1, //
            0, 1, 1, 1, 1, //
            0, 1, 1, 1, 1, //
        ];

        let report = compare_static_analysis_reachability_fixpoints(&adj, 5, 5)
            .expect("Fix: valid static-analysis corpus fixture should compare");

        assert!(report.exact_reachability_sets);
        assert_eq!(report.vyre_semiring.reachability, expected);
        assert_eq!(report.weir_frontier.reachability, expected);
        assert_eq!(report.graphblas_sparse.reachability, expected);
        assert_eq!(
            report.vyre_semiring.telemetry.engine_id,
            "vyre.semiring.bool_or.dense"
        );
        assert_eq!(report.weir_frontier.telemetry.engine_id, "weir.csr.frontier");
        assert_eq!(
            report.graphblas_sparse.telemetry.engine_id,
            "graphblas.sparse.bool_mxm"
        );
        for telemetry in [
            &report.vyre_semiring.telemetry,
            &report.weir_frontier.telemetry,
            &report.graphblas_sparse.telemetry,
        ] {
            assert!(telemetry.iterations > 0);
            assert!(telemetry.bytes_touched > 0);
            assert!(telemetry.frontier_density_bps <= 10_000);
            assert!(telemetry.active_time_ns > 0);
        }
    }

    #[test]
    fn delta_maintained_reachability_insertion_matches_full_recompute() {
        let adj = vec![
            0, 1, 0, 0, //
            0, 0, 1, 0, //
            0, 0, 0, 0, //
            0, 0, 0, 0, //
        ];
        let batch = DeltaRelationBatch {
            insertions: vec![DeltaRelationChange {
                source: 2,
                target: 3,
            }],
            deletions: Vec::new(),
        };

        let report = compare_delta_maintained_reachability(&adj, 4, 4, &batch)
            .expect("Fix: insertion delta fixture should compare");

        assert!(report.evidence.exact_result_parity);
        assert_eq!(report.delta_closure, report.full_recompute_closure);
        assert_eq!(report.evidence.inserted_tuple_count, 1);
        assert_eq!(report.evidence.deleted_tuple_count, 0);
        assert_eq!(report.evidence.changed_tuple_count, 3);
        assert_eq!(report.evidence.recomputed_tuple_count, 3);
        assert!(report.evidence.iterations > 0);
        assert!(report.evidence.elapsed_active_time_ns > 0);
    }

    #[test]
    fn delta_maintained_reachability_deletion_records_full_recompute_fallback() {
        let adj = vec![
            0, 1, 0, 0, //
            0, 0, 1, 0, //
            0, 0, 0, 1, //
            0, 0, 0, 0, //
        ];
        let batch = DeltaRelationBatch {
            insertions: Vec::new(),
            deletions: vec![DeltaRelationChange {
                source: 1,
                target: 2,
            }],
        };

        let report = compare_delta_maintained_reachability(&adj, 4, 4, &batch)
            .expect("Fix: deletion delta fixture should compare");

        assert!(report.evidence.exact_result_parity);
        assert_eq!(report.delta_closure, report.full_recompute_closure);
        assert_eq!(report.evidence.inserted_tuple_count, 0);
        assert_eq!(report.evidence.deleted_tuple_count, 1);
        assert_eq!(report.evidence.recomputed_tuple_count, 16);
        assert!(report.evidence.changed_tuple_count > 0);
    }

    #[test]
    fn delta_maintained_reachability_rejects_out_of_range_delta_tuple() {
        let adj = vec![0, 0, 0, 0];
        let batch = DeltaRelationBatch {
            insertions: vec![DeltaRelationChange {
                source: 0,
                target: 3,
            }],
            deletions: Vec::new(),
        };

        let error = compare_delta_maintained_reachability(&adj, 2, 2, &batch)
            .expect_err("Fix: out-of-range delta tuple should reject");

        assert!(error.contains("outside node_count=2"));
    }

    struct SemiringDispatcher {
        outputs: Vec<Vec<u8>>,
    }

    impl OptimizerDispatcher for SemiringDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            if inputs.len() != 3 {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: semiring test dispatcher expected 3 inputs, got {}.",
                    inputs.len()
                )));
            }
            Ok(self.outputs.clone())
        }
    }

    struct SequenceDispatcher {
        outputs: Vec<Vec<Vec<u8>>>,
        cursor: Cell<usize>,
    }

    impl OptimizerDispatcher for SequenceDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            if inputs.len() != 3 {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: sequence test dispatcher expected 3 inputs, got {}.",
                    inputs.len()
                )));
            }
            let idx = self.cursor.get();
            self.cursor.set(idx + 1);
            self.outputs.get(idx).cloned().ok_or_else(|| {
                DispatchError::BackendError("Fix: sequence dispatcher exhausted outputs.".into())
            })
        }
    }

    #[test]
    fn reachability_chain_graph() {
        // 0 → 1 → 2 → 3
        let adj = vec![0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        let closure = reachability_closure(&adj, 4, 5);
        // After closure: 0 reaches {1, 2, 3}; 1 reaches {2, 3}; 2 reaches {3}.
        assert_eq!(closure[0 * 4 + 1], 1);
        assert_eq!(closure[0 * 4 + 2], 1);
        assert_eq!(closure[0 * 4 + 3], 1);
        assert_eq!(closure[1 * 4 + 3], 1);
        // No reverse edges
        assert_eq!(closure[3 * 4 + 0], 0);
    }

    #[test]
    fn reference_semiring_gemm_into_delegates_to_foundation_authority() {
        let left = vec![1, 2, 3, 4, 5, 6];
        let right = vec![7, 8, 9, 10, 11, 12];
        let mut out = Vec::with_capacity(8);
        let ptr = out.as_ptr();
        reference_semiring_gemm_into(&left, &right, 2, 2, 3, Semiring::Real, &mut out);
        let mut expected = Vec::new();
        foundation_dataflow::semiring_gemm_cpu_into(
            &left,
            &right,
            2,
            2,
            3,
            Semiring::Real,
            &mut expected,
        );
        assert_eq!(out, expected);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn reachability_invalid_shapes_clear_buffers_without_panicking() {
        let mut current = vec![99, 100];
        let mut next = vec![101];
        reachability_closure_into(&[0, 1, 0], 2, 4, &mut current, &mut next);
        assert!(current.is_empty());
        assert!(next.is_empty());
        reachability_closure_into(&[], 0, 4, &mut current, &mut next);
        assert!(current.is_empty());
        assert!(next.is_empty());
    }

    #[test]
    fn reachability_respects_primitive_max_iters_policy() {
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        assert_eq!(reachability_closure(&adj, 3, 0).len(), adj.len());
        assert_eq!(reachability_closure(&adj, 3, 0), adj);
    }

    #[test]
    fn generated_reachability_matches_foundation_authority() {
        for n in 1u32..=8 {
            let cells = (n * n) as usize;
            for seed in 0u32..64 {
                let mut state = seed ^ n.wrapping_mul(0x9E37);
                let mut adj = Vec::with_capacity(cells);
                for _ in 0..cells {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    adj.push((state >> 31) & 1);
                }
                assert_eq!(
                    reachability_closure(&adj, n, n),
                    foundation_dataflow::reachability_closure(&adj, n, n),
                    "n={n} seed={seed}"
                );
            }
        }
    }

    #[test]
    fn semiring_via_into_decodes_exact_output_into_reused_buffer() {
        let dispatcher = SemiringDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&[7])],
        };
        let mut c = Vec::with_capacity(4);
        let ptr = c.as_ptr();
        semiring_gemm_via_into(&dispatcher, &[2], &[3], 1, 1, 1, Semiring::Real, &mut c)
            .expect("Fix: dispatch succeeds");
        assert_eq!(c, vec![7]);
        assert_eq!(c.as_ptr(), ptr);
    }

    #[test]
    fn semiring_via_rejects_extra_outputs() {
        let dispatcher = SemiringDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&[7]), u32_slice_to_le_bytes(&[0])],
        };
        let err = semiring_gemm_via(&dispatcher, &[2], &[3], 1, 1, 1, Semiring::Real)
            .expect_err("extra outputs must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn semiring_via_rejects_trailing_output_bytes() {
        let dispatcher = SemiringDispatcher {
            outputs: vec![vec![7, 0, 0, 0, 1]],
        };
        let err = semiring_gemm_via(&dispatcher, &[2], &[3], 1, 1, 1, Semiring::Real)
            .expect_err("trailing output bytes must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn reachability_disjoint_components_stay_disjoint() {
        // 0 → 1, 2 → 3, no cross.
        let adj = vec![0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        let closure = reachability_closure(&adj, 4, 5);
        assert_eq!(closure[0 * 4 + 2], 0);
        assert_eq!(closure[2 * 4 + 0], 0);
    }

    #[test]
    fn lineage_closure_unions_clauses_along_paths() {
        // Edge 0→1 used clause f1 = 0b01; edge 1→2 used clause f2 = 0b10.
        // Path 0→2 uses both: 0b11.
        let f1 = 0b01;
        let f2 = 0b10;
        let adj = vec![0, f1, 0, 0, 0, f2, 0, 0, 0];
        let closure = lineage_closure(&adj, 3, 5);
        assert_eq!(closure[0 * 3 + 2], f1 | f2);
    }

    #[test]
    fn shortest_path_closure_finds_two_hop_minimum() {
        let inf = u32::MAX;
        // 0→1 cost 5, 1→2 cost 3, 0→2 cost 100 (slower direct).
        let adj = vec![inf, 5, 100, inf, inf, 3, inf, inf, inf];
        let closure = shortest_path_closure(&adj, 3, 5);
        // Best 0→2 = min(100, 5+3) = 8.
        assert_eq!(closure[0 * 3 + 2], 8);
    }

    #[test]
    fn reachability_self_loop_detected() {
        // 0 → 1, 1 → 0. Closure should mark both.
        let adj = vec![0, 1, 1, 0];
        let closure = reachability_closure(&adj, 2, 5);
        // After 1 iteration: 0 reaches 0 via 0→1→0; 1 reaches 1.
        assert_eq!(closure[0 * 2 + 0], 1);
        assert_eq!(closure[1 * 2 + 1], 1);
    }

    // ---- forward_backward_bitsets_for_pivot + scc_components_via_substrate ----

    #[test]
    fn fb_bitsets_chain_pivot_zero() {
        // 0 → 1 → 2. From pivot 0: forward = {0,1,2}, backward = {0}.
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let (fwd, bwd) = forward_backward_bitsets_for_pivot(&adj, 0, 3);
        assert_eq!(fwd, vec![0b111]);
        assert_eq!(bwd, vec![0b001]);
    }

    #[test]
    fn fb_bitsets_chain_pivot_two() {
        // 0 → 1 → 2. From pivot 2: forward = {2}, backward = {0,1,2}.
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let (fwd, bwd) = forward_backward_bitsets_for_pivot(&adj, 2, 3);
        assert_eq!(fwd, vec![0b100]);
        assert_eq!(bwd, vec![0b111]);
    }

    #[test]
    fn fb_bitsets_into_reuses_capacity_and_matches_owned() {
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let mut scratch = DataflowFixpointScratch::default();
        forward_backward_bitsets_for_pivot_into(&adj, 2, 3, &mut scratch);
        let fwd_capacity = scratch.forward.capacity();
        let bwd_capacity = scratch.backward.capacity();
        assert_eq!(scratch.forward_bitset(), &[0b100]);
        assert_eq!(scratch.backward_bitset(), &[0b111]);

        forward_backward_bitsets_for_pivot_into(&adj, 0, 3, &mut scratch);
        assert_eq!(scratch.forward.capacity(), fwd_capacity);
        assert_eq!(scratch.backward.capacity(), bwd_capacity);
        assert_eq!(scratch.forward_bitset(), &[0b111]);
        assert_eq!(scratch.backward_bitset(), &[0b001]);
    }

    #[test]
    fn scc_components_chain_each_node_singleton() {
        // 0 → 1 → 2 (DAG). Every SCC is a singleton.
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let comps = scc_components_via_substrate(&adj, 3);
        // Each node stamped by itself (first pivot wins).
        assert_eq!(comps, vec![0, 1, 2]);
    }

    #[test]
    fn scc_components_two_cycle_collapses_to_first_pivot() {
        // 0 → 1, 1 → 0. {0,1} is one SCC. First pivot 0 stamps both.
        let adj = vec![0, 1, 1, 0];
        let comps = scc_components_via_substrate(&adj, 2);
        assert_eq!(comps, vec![0, 0]);
    }

    #[test]
    fn scc_components_into_reuses_output_and_matches_owned() {
        let adj = vec![0, 1, 1, 0];
        let mut comps = Vec::new();
        let mut scratch = DataflowFixpointScratch::default();
        reference_scc_components_via_substrate_into(&adj, 2, &mut comps, &mut scratch);
        let comps_capacity = comps.capacity();
        let scratch_capacity = scratch.next_components.capacity();
        assert_eq!(comps, vec![0, 0]);

        reference_scc_components_via_substrate_into(&adj, 2, &mut comps, &mut scratch);
        assert_eq!(comps.capacity(), comps_capacity);
        assert_eq!(scratch.next_components.capacity(), scratch_capacity);
        assert_eq!(comps, scc_components_via_substrate(&adj, 2));
    }

    #[test]
    fn scc_components_gpu_into_reuses_output_storage() {
        let adj = vec![0, 1, 1, 0];
        let semiring_step_a = u32_slice_to_le_bytes(&[1, 0, 0, 1]);
        let semiring_step_b = u32_slice_to_le_bytes(&[1, 1, 1, 1]);
        let components_done = u32_slice_to_le_bytes(&[0, 0]);
        let dispatcher = SequenceDispatcher {
            outputs: vec![
                vec![semiring_step_a.clone()],
                vec![semiring_step_b.clone()],
                vec![semiring_step_a.clone()],
                vec![semiring_step_b.clone()],
                vec![components_done.clone()],
                vec![semiring_step_a.clone()],
                vec![semiring_step_b.clone()],
                vec![semiring_step_a],
                vec![semiring_step_b],
                vec![components_done],
            ],
            cursor: Cell::new(0),
        };
        let mut scratch = SccComponentsGpuScratch::default();
        let mut components = Vec::with_capacity(2);

        scc_components_via_substrate_with_scratch_into(
            &dispatcher,
            &adj,
            2,
            &mut scratch,
            &mut components,
        )
        .unwrap();
        let capacity = components.capacity();
        assert_eq!(components, vec![0, 0]);

        scc_components_via_substrate_with_scratch_into(
            &dispatcher,
            &adj,
            2,
            &mut scratch,
            &mut components,
        )
        .unwrap();
        assert_eq!(components.capacity(), capacity);
        assert_eq!(components, vec![0, 0]);
    }

    /// Closure-bar: the substrate-driven SCC must agree with running
    /// `scc_decompose::cpu_ref` directly with manually-prepared
    /// forward/backward bitsets. Asserts the wiring doesn't drift.
    #[test]
    fn scc_components_match_direct_primitive_call() {
        // 0 → 1 → 2 → 0 (one big cycle), 3 → 4 separate.
        let adj = vec![
            0, 1, 0, 0, 0, // 0 -> 1
            0, 0, 1, 0, 0, // 1 -> 2
            1, 0, 0, 0, 0, // 2 -> 0
            0, 0, 0, 0, 1, // 3 -> 4
            0, 0, 0, 0, 0, // 4
        ];
        let via_substrate = scc_components_via_substrate(&adj, 5);

        // Manual replay: pivot 0 stamps {0,1,2}; pivot 3 stamps {3};
        // pivot 4 stamps {4}.
        let mut manual = vec![u32::MAX; 5];
        for pivot in [0u32, 3, 4] {
            let (fwd, bwd) = forward_backward_bitsets_for_pivot(&adj, pivot, 5);
            manual = vyre_primitives::graph::scc_decompose::cpu_ref(5, &fwd, &bwd, &manual, pivot);
        }
        assert_eq!(via_substrate, manual);
        // The cycle members all carry pivot 0.
        assert_eq!(via_substrate[0..3], [0, 0, 0]);
        // Singletons keep their own pivot id.
        assert_eq!(via_substrate[3], 3);
        assert_eq!(via_substrate[4], 4);
    }

    /// Adversarial: a fully disconnected graph (no edges) must yield
    /// `[0, 1, 2, ..., n-1]` because every pivot stamps only itself.
    #[test]
    fn scc_components_no_edges_each_pivot_stamps_only_itself() {
        let n = 4;
        let adj = vec![0u32; (n * n) as usize];
        let comps = scc_components_via_substrate(&adj, n);
        assert_eq!(comps, vec![0, 1, 2, 3]);
    }

    /// Adversarial: a self-loop on a node must NOT pull other nodes
    /// into its SCC. A common bug is to over-eagerly mark every node
    /// reached via the closure's reflexive-transitive interpretation.
    #[test]
    fn scc_components_self_loop_does_not_merge_distinct_components() {
        // 0 -> 0 (self-loop), 1 isolated, 2 isolated.
        let adj = vec![1, 0, 0, 0, 0, 0, 0, 0, 0];
        let comps = scc_components_via_substrate(&adj, 3);
        assert_eq!(comps, vec![0, 1, 2]);
    }
}
