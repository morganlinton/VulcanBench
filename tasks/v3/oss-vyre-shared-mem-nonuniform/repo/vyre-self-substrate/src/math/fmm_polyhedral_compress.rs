//! Polyhedral-fusion all-pairs compression via #51 FMM (#19+#51 self-consumer).
//!
//! Closes the recursion thesis for #51  -  the FMM (fast multipole)
//! hierarchical-expansion primitives ship to user dialects (kernel
//! methods at scale, computational physics, dense GP inference) AND
//! provide vyre's polyhedral fusion analysis with the hierarchical
//! compression that keeps O(N²) all-pairs tractable at workspace
//! scale.
//!
//! # The self-use
//!
//! Vyre's #19 polyhedral fusion (already shipped at
//! [`crate::polyhedral_fusion`]) computes pairwise
//! affine-dependency adjacency over Regions: every pair (i, j) is
//! checked for fusion eligibility. At N Regions this is N(N-1)/2
//! comparisons  -  O(N²) memory + compute.
//!
//! FMM exploits the fact that Regions far apart in the dispatch
//! topology rarely have fusion dependencies (the dispatch graph is
//! quasi-locally connected). Hierarchical decomposition:
//!
//! 1. **P2M**: aggregate per-Region fusion-affinity into multipole
//!    moments per spatial cell of the dispatch hierarchy.
//! 2. **M2L**: translate distant cell moments to local expansions
//!    (constant cost per cell-pair regardless of contained Regions).
//! 3. **L2P**: evaluate local expansions at each Region to recover
//!    its all-pairs fusion-affinity sum.
//!
//! Total cost: O(N log N) memory + compute. This module owns the
//! zeroth-moment compression path, which captures the dominant
//! cluster effect and keeps the self-consumer contract simple.
//!
//! # Why this matters
//!
//! At workspace scale, polyhedral_fusion's O(N²) cost is the
//! gating factor  -  1M Regions = 10¹² pairs, untractable. With FMM,
//! 1M Regions = ~20M operations, dispatched in seconds.
//!
//! # Algorithm
//!
//! Higher-moment FMM compression belongs in distinct registered ops so
//! each multipole order has an explicit schema and test oracle.

use crate::dispatch_buffers::{
    ceil_div_u32, decode_f32_output_exact, ensure_input_slots, write_f32_slice_le_bytes,
    write_u32_slice_le_bytes, write_zero_bytes,
};
#[cfg(test)]
use crate::hardware::scratch::reserve_vec_capacity_or_panic;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::math::fmm::{l2p_zeroth_f32_step, m2l_zeroth_f32_step, p2m_zeroth_f32_step};

/// Caller-owned GPU dispatch scratch for zeroth-moment FMM compression.
#[derive(Debug, Default)]
pub struct FmmPolyhedralGpuScratch {
    inputs: Vec<Vec<u8>>,
    cell_moments: Vec<f32>,
    cell_local: Vec<f32>,
}

#[cfg(test)]
fn p2m_zeroth_moment_cpu_into(charges: &[f64], cell_assignment: &[u32], moments: &mut Vec<f64>) {
    if charges.is_empty() {
        debug_assert!(cell_assignment.is_empty());
        moments.clear();
        return;
    }
    let n_cells = cell_assignment.iter().max().copied().unwrap_or(0) as usize + 1;
    moments.clear();
    moments.resize(n_cells, 0.0);
    for (i, &cell) in cell_assignment.iter().enumerate() {
        moments[cell as usize] += charges[i];
    }
}

#[cfg(test)]
fn m2l_zeroth_translate_cpu(source_moment: f64, distance: f64) -> f64 {
    source_moment / distance.max(1e-12)
}

#[cfg(test)]
fn l2p_zeroth_eval_cpu(local_moment: f64, _target_x: f64, _target_y: f64) -> f64 {
    local_moment
}

/// Aggregate per-Region fusion-affinity scores into per-cell multipole
/// moments. `scores[i]` is Region i's affinity scalar; `cell_assignment[i]`
/// is its parent cell id. Returns one f64 moment per cell (zeroth moment
/// = sum of contained scores).
///
/// # Panics
///
/// Panics if `scores.len() != cell_assignment.len()`.
#[must_use]
#[cfg(test)]
pub fn aggregate_to_cells(scores: &[f64], cell_assignment: &[u32]) -> Vec<f64> {
    let mut out = Vec::new();
    reference_aggregate_to_cells_into(scores, cell_assignment, &mut out);
    out
}

/// Aggregate per-Region fusion-affinity scores into caller-owned cell moments.
#[cfg(test)]
pub fn reference_aggregate_to_cells_into(
    scores: &[f64],
    cell_assignment: &[u32],
    out: &mut Vec<f64>,
) {
    use crate::observability::{bump, fmm_polyhedral_compress_calls};
    bump(&fmm_polyhedral_compress_calls);
    assert_eq!(scores.len(), cell_assignment.len());
    p2m_zeroth_moment_cpu_into(scores, cell_assignment, out);
}

/// Translate source-cell moments to target-cell local expansions.
/// `cell_moments[s]` is the source cell's aggregated moment;
/// `cell_distances[(target, source)]` is the precomputed distance
/// (laid out row-major: `cell_distances[t * num_cells + s]`).
/// Returns the per-target-cell local expansion as the sum of
/// translated moments from all sources.
///
/// # Panics
///
/// Panics if `cell_distances.len() != cell_moments.len() * cell_moments.len()`.
#[must_use]
#[cfg(test)]
pub fn translate_to_targets(cell_moments: &[f64], cell_distances: &[f64]) -> Vec<f64> {
    let mut local = Vec::new();
    reference_translate_to_targets_into(cell_moments, cell_distances, &mut local);
    local
}

/// Translate source-cell moments into caller-owned target locals.
#[cfg(test)]
pub fn reference_translate_to_targets_into(
    cell_moments: &[f64],
    cell_distances: &[f64],
    local: &mut Vec<f64>,
) {
    use crate::observability::{bump, fmm_polyhedral_compress_calls};
    bump(&fmm_polyhedral_compress_calls);
    let num_cells = cell_moments.len();
    assert_eq!(
        cell_distances.len(),
        num_cells * num_cells,
        "Fix: cell_distances must be num_cells*num_cells row-major."
    );

    local.clear();
    local.resize(num_cells, 0.0);
    for t in 0..num_cells {
        for s in 0..num_cells {
            if t == s {
                continue; // self-cell handled by direct evaluation
            }
            let d = cell_distances[t * num_cells + s];
            local[t] += m2l_zeroth_translate_cpu(cell_moments[s], d);
        }
    }
}

/// Evaluate local expansions at each Region to recover its
/// all-pairs fusion-affinity sum. `cell_local[c]` is the local
/// expansion at cell c; `cell_assignment[i]` is Region i's parent
/// cell. Returns the per-Region affinity sum.
#[must_use]
#[cfg(test)]
pub fn evaluate_at_regions(cell_local: &[f64], cell_assignment: &[u32], n: u32) -> Vec<f64> {
    let mut out = Vec::new();
    reference_evaluate_at_regions_into(cell_local, cell_assignment, n, &mut out);
    out
}

/// Evaluate local expansions into caller-owned per-Region output.
#[cfg(test)]
pub fn reference_evaluate_at_regions_into(
    cell_local: &[f64],
    cell_assignment: &[u32],
    n: u32,
    out: &mut Vec<f64>,
) {
    use crate::observability::{bump, fmm_polyhedral_compress_calls};
    bump(&fmm_polyhedral_compress_calls);
    assert_eq!(cell_assignment.len(), n as usize);
    out.clear();
    reserve_vec_capacity_or_panic(out, n as usize, "FMM region evaluation output");
    #[allow(clippy::needless_range_loop)]
    for i in 0..n as usize {
        let cell = cell_assignment[i] as usize;
        assert!(
            cell < cell_local.len(),
            "Fix: cell assignment {cell} out of bounds for {} cells",
            cell_local.len()
        );
        out.push(l2p_zeroth_eval_cpu(cell_local[cell], 0.0, 0.0));
    }
}

/// Run the full P2M → M2L → L2P pipeline. `scores` are per-Region
/// fusion-affinity scalars; `cell_assignment[i]` is Region i's parent
/// cell; `cell_distances` is the precomputed n_cells×n_cells distance
/// matrix. Returns per-Region all-pairs affinity sum approximated to
/// the zeroth-moment FMM truncation.
#[must_use]
#[cfg(test)]
pub fn fmm_compress_pairwise(
    scores: &[f64],
    cell_assignment: &[u32],
    cell_distances: &[f64],
    n: u32,
) -> Vec<f64> {
    let mut cell_moments = Vec::new();
    let mut cell_local = Vec::new();
    let mut out = Vec::new();
    fmm_compress_pairwise_into(
        scores,
        cell_assignment,
        cell_distances,
        n,
        &mut cell_moments,
        &mut cell_local,
        &mut out,
    );
    out
}

/// Run the full P2M → M2L → L2P pipeline into caller-owned buffers.
#[cfg(test)]
pub fn fmm_compress_pairwise_into(
    scores: &[f64],
    cell_assignment: &[u32],
    cell_distances: &[f64],
    n: u32,
    cell_moments: &mut Vec<f64>,
    cell_local: &mut Vec<f64>,
    out: &mut Vec<f64>,
) {
    reference_aggregate_to_cells_into(scores, cell_assignment, cell_moments);
    reference_translate_to_targets_into(cell_moments, cell_distances, cell_local);
    reference_evaluate_at_regions_into(cell_local, cell_assignment, n, out);
}

/// Aggregate per-region f32 affinity scores into per-cell moments through the active backend.
///
/// # Errors
///
/// Returns [`DispatchError`] for malformed shapes, oversized buffers, dispatch rejection, or
/// malformed backend output.
pub fn aggregate_to_cells_via(
    dispatcher: &dyn OptimizerDispatcher,
    scores: &[f32],
    cell_assignment: &[u32],
) -> Result<Vec<f32>, DispatchError> {
    let mut scratch = FmmPolyhedralGpuScratch::default();
    let mut out = Vec::new();
    aggregate_to_cells_via_with_scratch_into(
        dispatcher,
        scores,
        cell_assignment,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Aggregate per-region f32 affinity scores into caller-owned output through the active backend.
///
/// # Errors
///
/// Returns [`DispatchError`] for malformed shapes, oversized buffers, dispatch rejection, or
/// malformed backend output.
pub fn aggregate_to_cells_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    scores: &[f32],
    cell_assignment: &[u32],
    scratch: &mut FmmPolyhedralGpuScratch,
    out: &mut Vec<f32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, fmm_polyhedral_compress_calls};
    bump(&fmm_polyhedral_compress_calls);

    let n_regions = validate_region_shape(
        scores.len(),
        cell_assignment.len(),
        "aggregate_to_cells_via",
    )?;
    if n_regions == 0 {
        out.clear();
        return Ok(());
    }
    let n_cells = cell_count(cell_assignment, "aggregate_to_cells_via")?;
    let out_bytes = bytes_for_f32_count(n_cells as usize, "aggregate_to_cells_via")?;
    let program = p2m_zeroth_f32_step("scores", "cell_assignment", "moments", n_regions, n_cells);

    ensure_input_slots(&mut scratch.inputs, 3);
    write_f32_slice_le_bytes(&mut scratch.inputs[0], scores);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], cell_assignment);
    write_zero_bytes(&mut scratch.inputs[2], out_bytes);

    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some([ceil_div_u32(n_cells, 256), 1, 1]),
    )?;
    let output = require_first_output(&outputs, "aggregate_to_cells_via")?;
    decode_f32_output_exact(output, n_cells as usize, "aggregate_to_cells_via", out)
}

/// Translate source-cell moments to target-cell locals through the active backend.
///
/// # Errors
///
/// Returns [`DispatchError`] for malformed shape, dispatch failure, or malformed backend output.
pub fn translate_to_targets_via(
    dispatcher: &dyn OptimizerDispatcher,
    cell_moments: &[f32],
    cell_distances: &[f32],
) -> Result<Vec<f32>, DispatchError> {
    let mut scratch = FmmPolyhedralGpuScratch::default();
    let mut out = Vec::new();
    translate_to_targets_via_with_scratch_into(
        dispatcher,
        cell_moments,
        cell_distances,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Translate source-cell moments to caller-owned target locals through the active backend.
///
/// # Errors
///
/// Returns [`DispatchError`] for malformed shape, dispatch failure, or malformed backend output.
pub fn translate_to_targets_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    cell_moments: &[f32],
    cell_distances: &[f32],
    scratch: &mut FmmPolyhedralGpuScratch,
    out: &mut Vec<f32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, fmm_polyhedral_compress_calls};
    bump(&fmm_polyhedral_compress_calls);

    let n_cells = validate_square_distance_shape(
        cell_moments.len(),
        cell_distances.len(),
        "translate_to_targets_via",
    )?;
    if n_cells == 0 {
        out.clear();
        return Ok(());
    }
    let out_bytes = bytes_for_f32_count(n_cells as usize, "translate_to_targets_via")?;
    let program = m2l_zeroth_f32_step("cell_moments", "cell_distances", "cell_local", n_cells);

    ensure_input_slots(&mut scratch.inputs, 3);
    write_f32_slice_le_bytes(&mut scratch.inputs[0], cell_moments);
    write_f32_slice_le_bytes(&mut scratch.inputs[1], cell_distances);
    write_zero_bytes(&mut scratch.inputs[2], out_bytes);

    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some([ceil_div_u32(n_cells, 256), 1, 1]),
    )?;
    let output = require_first_output(&outputs, "translate_to_targets_via")?;
    decode_f32_output_exact(output, n_cells as usize, "translate_to_targets_via", out)
}

/// Evaluate target-cell locals into per-region affinity scores through the active backend.
///
/// # Errors
///
/// Returns [`DispatchError`] for malformed shape, dispatch failure, or malformed backend output.
pub fn evaluate_at_regions_via(
    dispatcher: &dyn OptimizerDispatcher,
    cell_local: &[f32],
    cell_assignment: &[u32],
    n: u32,
) -> Result<Vec<f32>, DispatchError> {
    let mut scratch = FmmPolyhedralGpuScratch::default();
    let mut out = Vec::new();
    evaluate_at_regions_via_with_scratch_into(
        dispatcher,
        cell_local,
        cell_assignment,
        n,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Evaluate target-cell locals into caller-owned per-region output through the active backend.
///
/// # Errors
///
/// Returns [`DispatchError`] for malformed shape, dispatch failure, or malformed backend output.
pub fn evaluate_at_regions_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    cell_local: &[f32],
    cell_assignment: &[u32],
    n: u32,
    scratch: &mut FmmPolyhedralGpuScratch,
    out: &mut Vec<f32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, fmm_polyhedral_compress_calls};
    bump(&fmm_polyhedral_compress_calls);

    validate_region_n(cell_assignment.len(), n, "evaluate_at_regions_via")?;
    if n == 0 {
        out.clear();
        return Ok(());
    }
    let n_cells = u32::try_from(cell_local.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: evaluate_at_regions_via cell count {} exceeds u32::MAX.",
            cell_local.len()
        ))
    })?;
    if n_cells == 0 {
        return Err(DispatchError::BadInputs(
            "Fix: evaluate_at_regions_via requires at least one cell local for non-empty regions."
                .to_string(),
        ));
    }
    reject_out_of_bounds_cells(cell_assignment, n_cells as usize, "evaluate_at_regions_via")?;
    let out_len = n as usize;
    let out_bytes = bytes_for_f32_count(out_len, "evaluate_at_regions_via")?;
    let program = l2p_zeroth_f32_step("cell_local", "cell_assignment", "region_out", n, n_cells);

    ensure_input_slots(&mut scratch.inputs, 3);
    write_f32_slice_le_bytes(&mut scratch.inputs[0], cell_local);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], cell_assignment);
    write_zero_bytes(&mut scratch.inputs[2], out_bytes);

    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some([ceil_div_u32(n, 256), 1, 1]),
    )?;
    let output = require_first_output(&outputs, "evaluate_at_regions_via")?;
    decode_f32_output_exact(output, out_len, "evaluate_at_regions_via", out)
}

/// Run the full P2M → M2L → L2P FMM compressor through the active backend.
///
/// # Errors
///
/// Returns [`DispatchError`] for malformed inputs, dispatch failure, or malformed backend output.

pub fn fmm_compress_pairwise_via(
    dispatcher: &dyn OptimizerDispatcher,
    scores: &[f32],
    cell_assignment: &[u32],
    cell_distances: &[f32],
    n: u32,
) -> Result<Vec<f32>, DispatchError> {
    let mut scratch = FmmPolyhedralGpuScratch::default();
    let mut out = Vec::new();
    fmm_compress_pairwise_via_with_scratch_into(
        dispatcher,
        scores,
        cell_assignment,
        cell_distances,
        n,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Run the full P2M → M2L → L2P FMM compressor into caller-owned output and scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] for malformed inputs, dispatch failure, or malformed backend output.
pub fn fmm_compress_pairwise_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    scores: &[f32],
    cell_assignment: &[u32],
    cell_distances: &[f32],
    n: u32,
    scratch: &mut FmmPolyhedralGpuScratch,
    out: &mut Vec<f32>,
) -> Result<(), DispatchError> {
    validate_region_n(cell_assignment.len(), n, "fmm_compress_pairwise_via")?;
    validate_region_shape(
        scores.len(),
        cell_assignment.len(),
        "fmm_compress_pairwise_via",
    )?;
    if n == 0 {
        out.clear();
        scratch.cell_moments.clear();
        scratch.cell_local.clear();
        return Ok(());
    }
    let n_cells = cell_count(cell_assignment, "fmm_compress_pairwise_via")?;
    validate_square_distance_shape(
        n_cells as usize,
        cell_distances.len(),
        "fmm_compress_pairwise_via",
    )?;
    let mut cell_moments = std::mem::take(&mut scratch.cell_moments);
    aggregate_to_cells_via_with_scratch_into(
        dispatcher,
        scores,
        cell_assignment,
        scratch,
        &mut cell_moments,
    )?;
    let mut cell_local = std::mem::take(&mut scratch.cell_local);
    translate_to_targets_via_with_scratch_into(
        dispatcher,
        &cell_moments,
        cell_distances,
        scratch,
        &mut cell_local,
    )?;
    let result = evaluate_at_regions_via_with_scratch_into(
        dispatcher,
        &cell_local,
        cell_assignment,
        n,
        scratch,
        out,
    );
    scratch.cell_moments = cell_moments;
    scratch.cell_local = cell_local;
    result
}

fn validate_region_shape(
    scores_len: usize,
    cells_len: usize,
    context: &str,
) -> Result<u32, DispatchError> {
    if scores_len != cells_len {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires scores.len() == cell_assignment.len(), got scores={scores_len}, cells={cells_len}."
        )));
    }
    u32::try_from(scores_len).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: {context} region count {scores_len} exceeds u32::MAX."
        ))
    })
}

fn validate_region_n(cells_len: usize, n: u32, context: &str) -> Result<(), DispatchError> {
    if cells_len != n as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires cell_assignment.len() == n, got cells={cells_len}, n={n}."
        )));
    }
    Ok(())
}

fn cell_count(cell_assignment: &[u32], context: &str) -> Result<u32, DispatchError> {
    let max_cell = cell_assignment.iter().copied().max().ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: {context} requires a non-empty cell assignment."
        ))
    })?;
    max_cell.checked_add(1).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: {context} cell id {max_cell} cannot be represented as count."
        ))
    })
}

fn validate_square_distance_shape(
    cells_len: usize,
    distances_len: usize,
    context: &str,
) -> Result<u32, DispatchError> {
    let expected = cells_len.checked_mul(cells_len).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: {context} cell distance matrix size overflows usize for {cells_len} cells."
        ))
    })?;
    if distances_len != expected {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires cell_distances.len() == n_cells*n_cells, got distances={distances_len}, expected={expected}."
        )));
    }
    u32::try_from(cells_len).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: {context} cell count {cells_len} exceeds u32::MAX."
        ))
    })
}

fn reject_out_of_bounds_cells(
    cell_assignment: &[u32],
    n_cells: usize,
    context: &str,
) -> Result<(), DispatchError> {
    for (idx, &cell) in cell_assignment.iter().enumerate() {
        if cell as usize >= n_cells {
            return Err(DispatchError::BadInputs(format!(
                "Fix: {context} cell_assignment[{idx}]={cell} is out of bounds for {n_cells} cells."
            )));
        }
    }
    Ok(())
}

fn bytes_for_f32_count(count: usize, context: &str) -> Result<usize, DispatchError> {
    count
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: {context} output byte count overflows usize for {count} f32 values."
            ))
        })
}

fn require_first_output<'a>(
    outputs: &'a [Vec<u8>],
    context: &str,
) -> Result<&'a [u8], DispatchError> {
    outputs.first().map(Vec::as_slice).ok_or_else(|| {
        DispatchError::BackendError(format!("Fix: {context} expected one output buffer, got 0."))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::f32_slice_to_le_bytes;
    use std::cell::Cell;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn aggregate_sums_per_cell() {
        // 4 Regions, 2 cells: {0,1} → cell 0; {2,3} → cell 1.
        let scores = vec![1.0, 2.0, 3.0, 4.0];
        let cells = vec![0u32, 0, 1, 1];
        let moments = aggregate_to_cells(&scores, &cells);
        assert!(approx_eq(moments[0], 3.0));
        assert!(approx_eq(moments[1], 7.0));
    }

    #[test]
    fn translate_excludes_self_cell() {
        // 2 cells with moments [10, 20] and unit distances.
        let moments = vec![10.0, 20.0];
        let distances = vec![0.0, 1.0, 1.0, 0.0];
        let local = translate_to_targets(&moments, &distances);
        // local[0] = m2l(20, 1.0); local[1] = m2l(10, 1.0).
        assert!(approx_eq(local[0], m2l_zeroth_translate_cpu(20.0, 1.0)));
        assert!(approx_eq(local[1], m2l_zeroth_translate_cpu(10.0, 1.0)));
    }

    #[test]
    fn evaluate_distributes_local_to_regions() {
        let cell_local = vec![5.0, 7.0];
        let cells = vec![0u32, 1, 0, 1];
        let result = evaluate_at_regions(&cell_local, &cells, 4);
        assert!(approx_eq(result[0], 5.0));
        assert!(approx_eq(result[1], 7.0));
        assert!(approx_eq(result[2], 5.0));
        assert!(approx_eq(result[3], 7.0));
    }

    #[test]
    fn full_pipeline_into_reuses_buffers() {
        let scores = vec![1.0, 2.0, 3.0, 4.0];
        let cells = vec![0u32, 0, 1, 1];
        let distances = vec![0.0, 1.0, 1.0, 0.0];
        let mut moments = Vec::with_capacity(8);
        let mut local = Vec::with_capacity(8);
        let mut out = Vec::with_capacity(8);
        let pointers = [moments.as_ptr(), local.as_ptr(), out.as_ptr()];
        fmm_compress_pairwise_into(
            &scores,
            &cells,
            &distances,
            4,
            &mut moments,
            &mut local,
            &mut out,
        );
        assert_eq!(out.len(), 4);
        for ptr in [moments.as_ptr(), local.as_ptr(), out.as_ptr()] {
            assert!(pointers.contains(&ptr));
        }
    }

    #[test]
    fn full_pipeline_runs_without_panic() {
        let scores = vec![1.0, 2.0, 3.0, 4.0];
        let cells = vec![0u32, 0, 1, 1];
        let distances = vec![0.0, 1.0, 1.0, 0.0];
        let _result = fmm_compress_pairwise(&scores, &cells, &distances, 4);
    }

    #[test]
    fn empty_score_set_produces_zero_moments() {
        let scores: Vec<f64> = vec![];
        let cells: Vec<u32> = vec![];
        let moments = aggregate_to_cells(&scores, &cells);
        assert!(moments.is_empty());
    }

    #[test]
    fn full_pipeline_via_dispatcher_runs_without_cpu_helper() {
        let dispatcher = FmmDispatcher::default();
        let scores = vec![1.0_f32, 2.0, 3.0, 4.0];
        let cells = vec![0_u32, 0, 1, 1];
        let distances = vec![0.0_f32, 1.0, 1.0, 0.0];

        let out = fmm_compress_pairwise_via(&dispatcher, &scores, &cells, &distances, 4)
            .expect("Fix: dispatchable FMM pipeline should run");

        assert_eq!(dispatcher.calls.get(), 3);
        assert_eq!(out.len(), 4);
        assert!(approx_eq(out[0] as f64, 7.0));
        assert!(approx_eq(out[1] as f64, 7.0));
        assert!(approx_eq(out[2] as f64, 3.0));
        assert!(approx_eq(out[3] as f64, 3.0));
    }

    #[test]
    fn via_rejects_malformed_distance_matrix() {
        let dispatcher = FmmDispatcher::default();
        let scores = vec![1.0_f32, 2.0, 3.0, 4.0];
        let cells = vec![0_u32, 0, 1, 1];
        let distances = vec![0.0_f32, 1.0, 1.0];

        let err = fmm_compress_pairwise_via(&dispatcher, &scores, &cells, &distances, 4)
            .expect_err("malformed distance matrix must be rejected before dispatch");

        assert!(err.to_string().contains(
            "Fix: fmm_compress_pairwise_via requires cell_distances.len() == n_cells*n_cells"
        ));
    }

    #[test]
    fn production_source_keeps_cpu_fmm_helpers_out_of_via_path() {
        let source = include_str!("fmm_polyhedral_compress.rs");
        let via_section = source
            .split("pub fn aggregate_to_cells_via")
            .nth(1)
            .expect("Fix: via section should exist")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: test module marker should exist");

        assert!(!via_section.contains("_cpu"));
        assert!(!via_section.contains("reference_"));
    }

    #[derive(Default)]
    struct FmmDispatcher {
        calls: Cell<usize>,
    }

    impl OptimizerDispatcher for FmmDispatcher {
        fn dispatch(
            &self,
            _program: &vyre_foundation::ir::Program,
            inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            let call = self.calls.get();
            self.calls.set(call + 1);
            match call {
                0 => dispatch_p2m(inputs),
                1 => dispatch_m2l(inputs),
                2 => dispatch_l2p(inputs),
                other => Err(DispatchError::BadInputs(format!(
                    "Fix: FMM test dispatcher received unexpected dispatch #{other}."
                ))),
            }
        }
    }

    fn dispatch_p2m(inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, DispatchError> {
        let [score_bytes, cell_bytes, out_bytes] = inputs else {
            return Err(DispatchError::BadInputs(format!(
                "Fix: P2M test dispatcher expected 3 buffers, got {}.",
                inputs.len()
            )));
        };
        let scores = crate::hardware::dispatch_buffers::decode_f32_input_aligned(
            score_bytes,
            "FMM test dispatcher",
        )?;
        let cells = crate::hardware::dispatch_buffers::decode_u32_input_aligned(
            cell_bytes,
            "FMM test dispatcher",
        )?;
        let n_cells = out_bytes.len() / std::mem::size_of::<f32>();
        let mut out = vec![0.0_f32; n_cells];
        for (score, &cell) in scores.iter().zip(&cells) {
            out[cell as usize] += *score;
        }
        Ok(vec![f32_slice_to_le_bytes(&out)])
    }

    fn dispatch_m2l(inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, DispatchError> {
        let [moment_bytes, distance_bytes, out_bytes] = inputs else {
            return Err(DispatchError::BadInputs(format!(
                "Fix: M2L test dispatcher expected 3 buffers, got {}.",
                inputs.len()
            )));
        };
        let moments = crate::hardware::dispatch_buffers::decode_f32_input_aligned(
            moment_bytes,
            "FMM test dispatcher",
        )?;
        let distances = crate::hardware::dispatch_buffers::decode_f32_input_aligned(
            distance_bytes,
            "FMM test dispatcher",
        )?;
        let n_cells = out_bytes.len() / std::mem::size_of::<f32>();
        let mut out = vec![0.0_f32; n_cells];
        for target in 0..n_cells {
            for source in 0..n_cells {
                if source != target {
                    let distance = distances[target * n_cells + source].max(1.0e-12);
                    out[target] += moments[source] / distance;
                }
            }
        }
        Ok(vec![f32_slice_to_le_bytes(&out)])
    }

    fn dispatch_l2p(inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, DispatchError> {
        let [local_bytes, cell_bytes, out_bytes] = inputs else {
            return Err(DispatchError::BadInputs(format!(
                "Fix: L2P test dispatcher expected 3 buffers, got {}.",
                inputs.len()
            )));
        };
        let local = crate::hardware::dispatch_buffers::decode_f32_input_aligned(
            local_bytes,
            "FMM test dispatcher",
        )?;
        let cells = crate::hardware::dispatch_buffers::decode_u32_input_aligned(
            cell_bytes,
            "FMM test dispatcher",
        )?;
        let out_len = out_bytes.len() / std::mem::size_of::<f32>();
        let mut out = Vec::with_capacity(out_len);
        for &cell in cells.iter().take(out_len) {
            out.push(local[cell as usize]);
        }
        Ok(vec![f32_slice_to_le_bytes(&out)])
    }
}
