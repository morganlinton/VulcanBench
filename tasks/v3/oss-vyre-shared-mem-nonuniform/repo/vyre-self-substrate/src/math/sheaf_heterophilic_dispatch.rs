//! Heterophilic dispatch-graph analysis via #31 sheaf diffusion (#31 self-consumer).
//!
//! Closes the recursion thesis for #31  -  sheaf neural networks
//! ship to user dialects (heterophilic graph learning, social
//! networks, code call graphs) AND directly model vyre's own
//! dispatch graph, where compute-bound, memory-bound, and
//! control-flow nodes have fundamentally different "feature spaces"
//! that GNN-style isotropic diffusion can't capture.
//!
//! # The release self-use
//!
//! Vyre's dispatch graph is heterophilic by construction:
//!
//! - **Compute-bound nodes** (FFT, gemm) have feature dimensions
//!   {flops, register pressure, ALU utilization}.
//! - **Memory-bound nodes** (load/store/copy) have features
//!   {bytes/sec, cache hit rate, DRAM utilization}.
//! - **Control-flow nodes** (If, Loop) have features
//!   {branch divergence, predicate cost, scheduling fence count}.
//!
//! These three feature spaces are NOT comparable  -  flops/sec is
//! not the same kind of thing as bytes/sec. Standard GNN
//! homophilic diffusion would average across these heterogeneous
//! kinds and produce nonsense.
//!
//! Sheaf neural networks (Bodnar-Di Giovanni 2022,
//! Hansen-Gebhart 2023) generalize: each node carries its OWN
//! vector space + restriction maps to neighbors. The sheaf
//! Laplacian respects the heterogeneity. Diffusion on the sheaf
//! Laplacian preserves type-correctness.
//!
//! For vyre, sheaf diffusion on the dispatch graph PREDICTS where
//! fusion will fail: nodes whose stalks diverge under sheaf
//! diffusion are nodes whose feature spaces don't align  -  fusing
//! them requires a costly conversion shim.
//!
//! # Algorithm
//!
//! ```text
//! 1. assign each Region a stalk vector in its node-type's feature
//!    space
//! 2. compute the restriction diagonal  -  how strongly each Region
//!    "transmits" features to neighbors (high = compatible types,
//!    low = type-mismatch)
//! 3. one or more sheaf_diffusion_step iterations
//! 4. nodes whose stalks DIVERGE from neighbors after diffusion are
//!    flagged as fusion-incompatible
//! ```
//!
//! # Why this matters
//!
//! Today vyre's fusion analyzer treats the dispatch graph as
//! homogeneous  -  every Region looks the same to the scheduler.
//! Sheaf-diffusion-driven fusion analysis is the FIRST GPU
//! substrate to model dispatch graphs as the heterophilic
//! structures they actually are. Paradigm shift, not optimization.

use crate::dispatch_buffers::{
    ceil_div_u32, checked_product_count, decode_u32_output_exact, ensure_input_slots,
    write_u32_slice_le_bytes, write_zero_bytes,
};
use crate::hardware::scratch::reserve_vec_capacity_or_panic;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::sheaf::sheaf_diffusion_step;
#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::graph::sheaf::{sheaf_diffusion_step_cpu, sheaf_diffusion_step_cpu_into};

/// Caller-owned dispatch scratch for fixed-point sheaf diffusion.
#[derive(Debug, Default)]
pub struct SheafDispatchGpuScratch {
    inputs: Vec<Vec<u8>>,
    damping: Vec<u32>,
}

/// Apply one sheaf-diffusion step to dispatch-graph stalks.
/// `stalks[i]` is Region i's feature scalar (in its own type's
/// feature space); `restriction_diag[i]` is the per-Region
/// transmission coefficient (high = compatible neighbor types,
/// low = mismatch). `damping` is the diffusion rate in `[0, 1]`.
///
/// Returns the diffused stalks.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_diffuse_dispatch_stalks(
    stalks: &[f64],
    restriction_diag: &[f64],
    damping: f64,
) -> Vec<f64> {
    use crate::observability::{bump, sheaf_heterophilic_dispatch_calls};
    bump(&sheaf_heterophilic_dispatch_calls);
    sheaf_diffusion_step_cpu(stalks, restriction_diag, damping)
}

/// Apply one sheaf-diffusion step into caller-owned storage.
///
/// Clears `out` and reuses its allocation.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_diffuse_dispatch_stalks_into(
    stalks: &[f64],
    restriction_diag: &[f64],
    damping: f64,
    out: &mut Vec<f64>,
) {
    use crate::observability::{bump, sheaf_heterophilic_dispatch_calls};
    bump(&sheaf_heterophilic_dispatch_calls);
    sheaf_diffusion_step_cpu_into(stalks, restriction_diag, damping, out);
}

/// Fixed-point production path for one sheaf-diffusion step.
///
/// Inputs are primitive-native 16.16 u32 buffers with shape
/// `n_nodes * d`. The dispatcher runs [`sheaf_diffusion_step`] directly.
///
/// # Errors
///
/// Returns [`DispatchError`] when shapes are invalid, the primitive lane
/// space is exceeded, or the backend returns a malformed output buffer.
pub fn diffuse_dispatch_stalks_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    stalks_fixed: &[u32],
    restriction_diag_fixed: &[u32],
    damping_fixed: u32,
    n_nodes: u32,
    d: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    diffuse_dispatch_stalks_fixed_via_into(
        dispatcher,
        stalks_fixed,
        restriction_diag_fixed,
        damping_fixed,
        n_nodes,
        d,
        &mut out,
    )?;
    Ok(out)
}

/// Fixed-point sheaf-diffusion step into caller-owned output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks or backend execution fail.
pub fn diffuse_dispatch_stalks_fixed_via_into(
    dispatcher: &impl OptimizerDispatcher,
    stalks_fixed: &[u32],
    restriction_diag_fixed: &[u32],
    damping_fixed: u32,
    n_nodes: u32,
    d: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = SheafDispatchGpuScratch::default();
    diffuse_dispatch_stalks_fixed_via_with_scratch_into(
        dispatcher,
        stalks_fixed,
        restriction_diag_fixed,
        damping_fixed,
        n_nodes,
        d,
        &mut scratch,
        out,
    )
}

/// Fixed-point sheaf-diffusion step using caller-owned dispatch scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks or backend execution fail.
pub fn diffuse_dispatch_stalks_fixed_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    stalks_fixed: &[u32],
    restriction_diag_fixed: &[u32],
    damping_fixed: u32,
    n_nodes: u32,
    d: u32,
    scratch: &mut SheafDispatchGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, sheaf_heterophilic_dispatch_calls};
    bump(&sheaf_heterophilic_dispatch_calls);

    let cells = checked_product_count(
        n_nodes,
        d,
        "n_nodes",
        "d",
        "diffuse_dispatch_stalks_fixed_via",
    )?;
    let cells_u32 = u32::try_from(cells).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: diffuse_dispatch_stalks_fixed_via n_nodes*d exceeds the primitive u32 lane limit for n_nodes={n_nodes}, d={d}."
        ))
    })?;
    if stalks_fixed.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: diffuse_dispatch_stalks_fixed_via requires stalks_fixed.len() == n_nodes*d, got len={}, n_nodes={n_nodes}, d={d}, cells={cells}.",
            stalks_fixed.len()
        )));
    }
    if restriction_diag_fixed.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: diffuse_dispatch_stalks_fixed_via requires restriction_diag_fixed.len() == n_nodes*d, got len={}, n_nodes={n_nodes}, d={d}, cells={cells}.",
            restriction_diag_fixed.len()
        )));
    }

    let program = sheaf_diffusion_step(
        "stalks",
        "restriction_diag",
        "damping",
        "stalks_next",
        n_nodes,
        d,
    );
    let out_bytes = cells.checked_mul(std::mem::size_of::<u32>()).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: diffuse_dispatch_stalks_fixed_via output byte count overflows usize for cells={cells}."
        ))
    })?;
    scratch.damping.clear();
    scratch.damping.push(damping_fixed);
    ensure_input_slots(&mut scratch.inputs, 4);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], stalks_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], restriction_diag_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], &scratch.damping);
    write_zero_bytes(&mut scratch.inputs[3], out_bytes);
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some([ceil_div_u32(cells_u32, 256), 1, 1]),
    )?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: diffuse_dispatch_stalks_fixed_via expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], cells, "diffuse_dispatch_stalks_fixed_via", out)
}

/// Iterate sheaf diffusion until convergence (stalks stop changing
/// to within `tol`) or `max_iters` is reached. Returns
/// `(final_stalks, iters_run)`.
#[must_use]
#[cfg(test)]
pub fn diffuse_to_equilibrium(
    initial_stalks: &[f64],
    restriction_diag: &[f64],
    damping: f64,
    tol: f64,
    max_iters: u32,
) -> (Vec<f64>, u32) {
    let mut current = Vec::with_capacity(initial_stalks.len());
    let mut next = Vec::with_capacity(initial_stalks.len());
    let iters = diffuse_to_equilibrium_into(
        initial_stalks,
        restriction_diag,
        damping,
        tol,
        max_iters,
        &mut current,
        &mut next,
    );
    (current, iters)
}

/// Iterate sheaf diffusion into caller-owned storage.
///
/// `out` receives the final stalk vector and `scratch` is reused for each
/// intermediate step.
#[cfg(test)]
pub fn diffuse_to_equilibrium_into(
    initial_stalks: &[f64],
    restriction_diag: &[f64],
    damping: f64,
    tol: f64,
    max_iters: u32,
    out: &mut Vec<f64>,
    scratch: &mut Vec<f64>,
) -> u32 {
    out.clear();
    out.extend_from_slice(initial_stalks);
    for iter in 0..max_iters {
        reference_diffuse_dispatch_stalks_into(out, restriction_diag, damping, scratch);
        let max_change = scratch
            .iter()
            .zip(out.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f64, f64::max);
        std::mem::swap(out, scratch);
        if max_change < tol {
            return iter + 1;
        }
    }
    max_iters
}

/// Identify fusion-incompatible Region pairs: high stalk divergence
/// after diffusion = type-space mismatch. Returns a 0/1 vector;
/// 1 means "this Region's stalk diverged enough to flag fusion-incompatible."
#[must_use]
pub fn flag_fusion_incompatible(
    initial_stalks: &[f64],
    diffused_stalks: &[f64],
    divergence_threshold: f64,
) -> Vec<u32> {
    let mut out = Vec::new();
    flag_fusion_incompatible_into(
        initial_stalks,
        diffused_stalks,
        divergence_threshold,
        &mut out,
    );
    out
}

/// Identify fusion-incompatible Region pairs into caller-owned storage.
pub fn flag_fusion_incompatible_into(
    initial_stalks: &[f64],
    diffused_stalks: &[f64],
    divergence_threshold: f64,
    out: &mut Vec<u32>,
) {
    out.clear();
    reserve_vec_capacity_or_panic(out, initial_stalks.len(), "sheaf incompatibility output");
    initial_stalks
        .iter()
        .zip(diffused_stalks.iter())
        .map(|(&i, &d)| {
            if (i - d).abs() > divergence_threshold {
                1u32
            } else {
                0u32
            }
        })
        .for_each(|flag| out.push(flag));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_foundation::ir::Program;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn zero_damping_holds_initial() {
        let s = vec![1.0, 2.0, 3.0];
        let r = vec![0.5, 0.5, 0.5];
        let out = reference_diffuse_dispatch_stalks(&s, &r, 0.0);
        for (a, b) in s.iter().zip(out.iter()) {
            assert!(approx_eq(*a, *b));
        }
    }

    #[test]
    fn high_damping_drives_to_equilibrium() {
        let s = vec![1.0, 1.0, 1.0];
        let r = vec![1.0, 1.0, 1.0];
        let (final_stalks, iters) = diffuse_to_equilibrium(&s, &r, 0.9, 1e-6, 100);
        // High damping + uniform restriction collapses stalks toward 0.
        assert!(final_stalks.iter().all(|&v| v.abs() < 1.0));
        assert!(iters < 100);
    }

    #[test]
    fn flag_fusion_incompatible_threshold_zero_flags_all_changes() {
        let initial = vec![1.0, 2.0, 3.0];
        let diffused = vec![0.5, 2.0, 2.5];
        let flags = flag_fusion_incompatible(&initial, &diffused, 0.0);
        // 0 != 0.5 → flag; 2 == 2 → no flag; 3 != 2.5 → flag.
        assert_eq!(flags, vec![1, 0, 1]);
    }

    #[test]
    fn high_threshold_flags_nothing() {
        let initial = vec![1.0, 2.0];
        let diffused = vec![1.5, 2.5];
        let flags = flag_fusion_incompatible(&initial, &diffused, 100.0);
        assert_eq!(flags, vec![0, 0]);
    }

    #[test]
    fn flag_fusion_incompatible_into_reuses_buffer() {
        let initial = vec![1.0, 2.0, 3.0];
        let diffused = vec![0.5, 2.0, 2.5];
        let mut flags = Vec::with_capacity(8);
        let ptr = flags.as_ptr();
        flag_fusion_incompatible_into(&initial, &diffused, 0.0, &mut flags);
        assert_eq!(flags, vec![1, 0, 1]);
        assert_eq!(flags.as_ptr(), ptr);
    }

    #[test]
    fn equilibrium_with_zero_max_iters_returns_initial() {
        let s = vec![5.0, 10.0];
        let r = vec![1.0, 1.0];
        let (out, iters) = diffuse_to_equilibrium(&s, &r, 0.5, 1e-6, 0);
        assert_eq!(out, s);
        assert_eq!(iters, 0);
    }

    struct SheafDispatcher;

    impl OptimizerDispatcher for SheafDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 4);
            let stalks = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let restrictions = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
            let damping = crate::hardware::dispatch_buffers::read_u32s(&inputs[2])[0];
            assert_eq!(inputs[3].len(), stalks.len() * std::mem::size_of::<u32>());
            let out: Vec<u32> = stalks
                .iter()
                .zip(restrictions.iter())
                .map(|(&s, &r)| {
                    let damped_r = ((damping as u64 * r as u64) >> 16) as u32;
                    let delta = ((damped_r as u64 * s as u64) >> 16) as u32;
                    s.saturating_sub(delta)
                })
                .collect();
            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn fixed_via_dispatches_sheaf_step() {
        let one = 1u32 << 16;
        let half = 1u32 << 15;
        let out = diffuse_dispatch_stalks_fixed_via(
            &SheafDispatcher,
            &[10 * one, 20 * one],
            &[one, one],
            half,
            2,
            1,
        )
        .unwrap();
        assert_eq!(out, vec![5 * one, 10 * one]);
    }

    #[test]
    fn fixed_via_rejects_shape_mismatch() {
        let err = diffuse_dispatch_stalks_fixed_via(&SheafDispatcher, &[1, 2, 3], &[1, 2], 1, 2, 2)
            .unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }

    #[test]
    fn fixed_via_with_scratch_reuses_input_buffers() {
        let one = 1u32 << 16;
        let half = 1u32 << 15;
        let mut scratch = SheafDispatchGpuScratch::default();
        let mut out = Vec::new();

        diffuse_dispatch_stalks_fixed_via_with_scratch_into(
            &SheafDispatcher,
            &[10 * one, 20 * one],
            &[one, one],
            half,
            2,
            1,
            &mut scratch,
            &mut out,
        )
        .unwrap();
        let input_ptrs: Vec<*const u8> = scratch.inputs.iter().map(Vec::as_ptr).collect();
        diffuse_dispatch_stalks_fixed_via_with_scratch_into(
            &SheafDispatcher,
            &[8 * one, 12 * one],
            &[one, one],
            half,
            2,
            1,
            &mut scratch,
            &mut out,
        )
        .unwrap();

        for (before, after) in input_ptrs
            .iter()
            .zip(scratch.inputs.iter().map(Vec::as_ptr))
        {
            assert_eq!(*before, after);
        }
    }

    #[test]
    fn production_source_keeps_cpu_sheaf_helpers_out_of_via_path() {
        let source = include_str!("sheaf_heterophilic_dispatch.rs");
        let via_section = source
            .split("pub fn diffuse_dispatch_stalks_fixed_via")
            .nth(1)
            .expect("Fix: via section should exist")
            .split("#[must_use]\n#[cfg(test)]\npub fn diffuse_to_equilibrium")
            .next()
            .expect("Fix: test-only equilibrium marker should exist");

        assert!(!via_section.contains("_cpu"));
        assert!(!via_section.contains("reference_diffuse"));
    }
}
