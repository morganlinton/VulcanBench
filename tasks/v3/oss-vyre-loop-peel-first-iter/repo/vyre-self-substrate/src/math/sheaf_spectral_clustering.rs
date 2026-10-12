//! Sheaf-spectral clustering of dispatch graphs.
//!
//! Self-consumer for [#9 `sheaf_laplacian_eigenvalue`](vyre_primitives::math::sheaf_laplacian_eigenvalue).
//!
//! The dispatch graph's sheaf Laplacian carries spectral information
//! about cluster structure: the dominant eigenvalue corresponds to
//! the longest principal direction of the graph's heterophilic
//! diffusion. Combined with the existing
//! [`super::sheaf_heterophilic_dispatch::flag_fusion_incompatible`]
//! divergence flagging, this gives:
//!
//! - **Spectral gap**  -  eigenvalue magnitude indicates how cleanly
//!   the graph splits into clusters. Large gap = clean clusters,
//!   safe to fuse within each cluster.
//! - **Suggested cluster count**  -  derived from the eigenvalue
//!   spectrum via the substrate's power-iteration diagonal output.
//!
//! Used by the megakernel scheduler when the matroid heuristic
//! produces ambiguous results (many tied gain values)  -  falls back
//! to spectral cluster suggestions for tie-breaking.

#[cfg(test)]
use crate::dispatch_buffers::u32_slice_to_le_bytes;
use crate::dispatch_buffers::{
    ceil_div_u32, checked_product_count, decode_u32_output_exact, ensure_input_slots,
    write_u32_slice_le_bytes, write_zero_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::math::sheaf_laplacian_eigenvalue::cpu_ref_into;
use vyre_primitives::math::sheaf_laplacian_eigenvalue::sheaf_laplacian_eigenvalue;

/// Default power-iteration count for spectral cluster signal.
/// 32 iterations converges the dominant eigenvalue to <1e-6 relative
/// error on dispatch graphs we've measured (n ≤ 256).
pub const DEFAULT_POWER_ITERATIONS: u32 = 32;

/// Reusable buffers for sheaf-spectral power iteration.
#[derive(Debug, Default)]
pub struct SheafSpectrumScratch {
    #[cfg(any(test, feature = "cpu-parity"))]
    v_init: Vec<f64>,
    #[cfg(any(test, feature = "cpu-parity"))]
    v: Vec<f64>,
    #[cfg(any(test, feature = "cpu-parity"))]
    v_next: Vec<f64>,
}

impl SheafSpectrumScratch {
    /// Dominant eigenvector from the last spectral solve.
    #[must_use]
    #[cfg(any(test, feature = "cpu-parity"))]
    pub fn eigenvector(&self) -> &[f64] {
        &self.v
    }
}

/// Caller-owned GPU dispatch scratch for fixed-point sheaf spectra.
#[derive(Debug, Default)]
pub struct SheafSpectrumGpuScratch {
    inputs: Vec<Vec<u8>>,
}

/// Fixed-point dominant sheaf spectrum returned by the GPU-dispatchable path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedSheafSpectrum {
    /// Dominant eigenvalue/norm signal in primitive-native 16.16 storage.
    pub lambda: u32,
    /// Final eigenvector buffer in primitive-native 16.16 storage.
    pub eigenvector: Vec<u32>,
}

/// Compute the dominant eigenvalue + eigenvector of the dispatch
/// graph's sheaf Laplacian. The eigenvalue magnitude is the spectral
/// gap signal; the eigenvector indicates which work items lie on the
/// principal cluster boundary.
///
/// `restriction_diag[i]` is the per-item transmission coefficient
/// from the existing
/// [`super::sheaf_heterophilic_dispatch`] wire. Pass the same vector
/// the diffusion step uses.
///
/// Returns `(dominant_eigenvalue, eigenvector)` of length `n`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn dominant_spectrum(restriction_diag: &[f64], iterations: u32) -> (f64, Vec<f64>) {
    use crate::observability::{bump, sheaf_spectral_clustering_calls};
    bump(&sheaf_spectral_clustering_calls);
    let mut scratch = SheafSpectrumScratch::default();
    let lambda = dominant_spectrum_with_scratch(restriction_diag, iterations, &mut scratch);
    (lambda, scratch.v)
}

/// Compute the dominant eigenvalue using reusable spectral scratch.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn dominant_spectrum_with_scratch(
    restriction_diag: &[f64],
    iterations: u32,
    scratch: &mut SheafSpectrumScratch,
) -> f64 {
    reference_dominant_spectrum_into(
        restriction_diag,
        iterations,
        &mut scratch.v_init,
        &mut scratch.v,
        &mut scratch.v_next,
    )
}

/// Compute the dominant eigenvalue into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_dominant_spectrum_into(
    restriction_diag: &[f64],
    iterations: u32,
    v_init: &mut Vec<f64>,
    v: &mut Vec<f64>,
    v_next: &mut Vec<f64>,
) -> f64 {
    let n = restriction_diag.len();
    if n == 0 {
        v_init.clear();
        v.clear();
        v_next.clear();
        return 0.0;
    }
    let inv_sqrt_n = 1.0 / (n as f64).sqrt();
    v_init.clear();
    v_init.resize(n, inv_sqrt_n);
    cpu_ref_into(restriction_diag, v_init, iterations, v, v_next)
}

/// Fixed-point production path for sheaf spectral clustering.
///
/// `restriction_diag_fixed` and `v_init_fixed` are primitive-native 16.16
/// buffers with shape `n_nodes * d`. The dispatcher runs
/// [`sheaf_laplacian_eigenvalue`] directly and returns both the lambda output
/// and the mutated eigenvector buffer.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks fail, the primitive lane space
/// is exceeded, or the backend returns malformed output buffers.
pub fn dominant_spectrum_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    restriction_diag_fixed: &[u32],
    v_init_fixed: &[u32],
    n_nodes: u32,
    d: u32,
    iterations: u32,
) -> Result<FixedSheafSpectrum, DispatchError> {
    let mut scratch = SheafSpectrumGpuScratch::default();
    let mut eigenvector = Vec::new();
    let lambda = dominant_spectrum_fixed_via_with_scratch_into(
        dispatcher,
        restriction_diag_fixed,
        v_init_fixed,
        n_nodes,
        d,
        iterations,
        &mut scratch,
        &mut eigenvector,
    )?;
    Ok(FixedSheafSpectrum {
        lambda,
        eigenvector,
    })
}

/// Fixed-point sheaf spectral clustering into caller-owned eigenvector
/// storage. Returns the fixed-point lambda output.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks or backend execution fail.
pub fn dominant_spectrum_fixed_via_into(
    dispatcher: &impl OptimizerDispatcher,
    restriction_diag_fixed: &[u32],
    v_init_fixed: &[u32],
    n_nodes: u32,
    d: u32,
    iterations: u32,
    eigenvector_out: &mut Vec<u32>,
) -> Result<u32, DispatchError> {
    let mut scratch = SheafSpectrumGpuScratch::default();
    dominant_spectrum_fixed_via_with_scratch_into(
        dispatcher,
        restriction_diag_fixed,
        v_init_fixed,
        n_nodes,
        d,
        iterations,
        &mut scratch,
        eigenvector_out,
    )
}

/// Fixed-point sheaf spectral clustering with reusable dispatch input storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks or backend execution fail.
pub fn dominant_spectrum_fixed_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    restriction_diag_fixed: &[u32],
    v_init_fixed: &[u32],
    n_nodes: u32,
    d: u32,
    iterations: u32,
    scratch: &mut SheafSpectrumGpuScratch,
    eigenvector_out: &mut Vec<u32>,
) -> Result<u32, DispatchError> {
    use crate::observability::{bump, sheaf_spectral_clustering_calls};
    bump(&sheaf_spectral_clustering_calls);

    let cells = checked_product_count(n_nodes, d, "n_nodes", "d", "dominant_spectrum_fixed_via")?;
    let cells_u32 = u32::try_from(cells).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: dominant_spectrum_fixed_via n_nodes*d exceeds the primitive u32 lane limit for n_nodes={n_nodes}, d={d}."
        ))
    })?;
    if restriction_diag_fixed.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: dominant_spectrum_fixed_via requires restriction_diag_fixed.len() == n_nodes*d, got len={}, n_nodes={n_nodes}, d={d}, cells={cells}.",
            restriction_diag_fixed.len()
        )));
    }
    if v_init_fixed.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: dominant_spectrum_fixed_via requires v_init_fixed.len() == n_nodes*d, got len={}, n_nodes={n_nodes}, d={d}, cells={cells}.",
            v_init_fixed.len()
        )));
    }

    let program = sheaf_laplacian_eigenvalue(
        "restriction_diag",
        "v",
        "lambda",
        "scratch_v",
        "scratch_norm",
        n_nodes,
        d,
        iterations,
    );
    let vector_bytes = cells
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: dominant_spectrum_fixed_via scratch byte count overflows usize for cells={cells}."
            ))
        })?;
    ensure_input_slots(&mut scratch.inputs, 6);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], restriction_diag_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], v_init_fixed);
    write_zero_bytes(&mut scratch.inputs[2], std::mem::size_of::<u32>());
    write_zero_bytes(&mut scratch.inputs[3], vector_bytes);
    write_zero_bytes(&mut scratch.inputs[4], std::mem::size_of::<u32>());
    scratch.inputs[5].clear();
    scratch.inputs[5].extend_from_slice(&(1u32 << 16).to_le_bytes());
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs[..6],
        Some([ceil_div_u32(cells_u32, 256), 1, 1]),
    )?;
    if outputs.len() != 2 {
        return Err(DispatchError::BackendError(format!(
            "Fix: dominant_spectrum_fixed_via expected exactly eigenvector and lambda outputs, got {} buffer(s).",
            outputs.len()
        )));
    }

    decode_u32_output_exact(
        &outputs[0],
        cells,
        "dominant_spectrum_fixed_via eigenvector",
        eigenvector_out,
    )?;
    let mut lambda = Vec::with_capacity(1);
    decode_u32_output_exact(
        &outputs[1],
        1,
        "dominant_spectrum_fixed_via lambda",
        &mut lambda,
    )?;
    Ok(lambda[0])
}

/// Convenience: spectral gap signal in `[0, 1]` derived from the
/// dominant eigenvalue. Higher = cleaner cluster separation.
#[must_use]
#[cfg(test)]
pub fn spectral_gap(restriction_diag: &[f64]) -> f64 {
    let mut scratch = SheafSpectrumScratch::default();
    spectral_gap_into(restriction_diag, &mut scratch)
}

/// Compute spectral gap using caller-owned power-iteration scratch.
#[cfg(test)]
pub fn spectral_gap_into(restriction_diag: &[f64], scratch: &mut SheafSpectrumScratch) -> f64 {
    let lambda =
        dominant_spectrum_with_scratch(restriction_diag, DEFAULT_POWER_ITERATIONS, scratch);
    // Eigenvalues of a sheaf Laplacian on transmission diagonals are
    // bounded by max(restriction_diag); normalize to [0, 1].
    let max_diag = restriction_diag.iter().cloned().fold(0.0_f64, f64::max);
    if max_diag <= 1e-20 {
        0.0
    } else {
        (lambda / max_diag).clamp(0.0, 1.0)
    }
}

/// Derive a suggested cluster count from the principal eigenvector
/// sign pattern. Items whose eigenvector entry has the same sign
/// belong in the same cluster; flips between consecutive items
/// suggest cluster boundaries. Returns the count of distinct sign
/// runs (≥ 1).
#[must_use]
#[cfg(test)]
pub fn suggested_cluster_count(restriction_diag: &[f64]) -> u32 {
    let mut scratch = SheafSpectrumScratch::default();
    suggested_cluster_count_into(restriction_diag, &mut scratch)
}

/// Derive suggested cluster count using caller-owned spectral scratch.
#[cfg(test)]
pub fn suggested_cluster_count_into(
    restriction_diag: &[f64],
    scratch: &mut SheafSpectrumScratch,
) -> u32 {
    dominant_spectrum_with_scratch(restriction_diag, DEFAULT_POWER_ITERATIONS, scratch);
    let v = scratch.eigenvector();
    if v.is_empty() {
        return 0;
    }
    let mut count: u32 = 1;
    let mut last_sign = v[0].signum();
    for &x in v.iter().skip(1) {
        let sign = x.signum();
        if sign != 0.0 && sign != last_sign && last_sign != 0.0 {
            count = count.saturating_add(1);
            last_sign = sign;
        } else if last_sign == 0.0 && sign != 0.0 {
            last_sign = sign;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Program;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn dominant_eigenvalue_of_uniform_diag_is_diag_value() {
        // restriction = [0.7, 0.7, 0.7, 0.7] → dominant eigenvalue = 0.7.
        let diag = vec![0.7, 0.7, 0.7, 0.7];
        let (lambda, _v) = dominant_spectrum(&diag, 64);
        assert!(approx_eq(lambda, 0.7), "got lambda={lambda}");
    }

    #[test]
    fn dominant_eigenvalue_of_nonuniform_picks_max() {
        // restriction = [0.1, 0.5, 0.9, 0.3] → dominant eigenvalue ≈ 0.9.
        let diag = vec![0.1, 0.5, 0.9, 0.3];
        let (lambda, v) = dominant_spectrum(&diag, 128);
        assert!((lambda - 0.9).abs() < 0.01, "got lambda={lambda}");
        // Eigenvector should localize on index 2 (the 0.9 entry).
        let max_idx = v
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        assert_eq!(max_idx, 2);
    }

    #[test]
    fn empty_input_returns_zero_spectrum() {
        let (lambda, v) = dominant_spectrum(&[], 32);
        assert_eq!(lambda, 0.0);
        assert!(v.is_empty());
    }

    #[test]
    fn spectral_gap_is_one_for_uniform_diag() {
        // Uniform diagonal  -  eigenvalue equals max  -  gap = 1.
        let diag = vec![0.5; 8];
        let gap = spectral_gap(&diag);
        assert!((gap - 1.0).abs() < 1e-3);
    }

    #[test]
    fn scratch_paths_match_owned_spectral_helpers() {
        let diag = vec![0.1, 0.5, 0.9, 0.3];
        let (owned_lambda, owned_v) = dominant_spectrum(&diag, 64);
        let mut scratch = SheafSpectrumScratch::default();
        let borrowed_lambda = dominant_spectrum_with_scratch(&diag, 64, &mut scratch);
        assert!(approx_eq(owned_lambda, borrowed_lambda));
        assert_eq!(scratch.eigenvector().len(), owned_v.len());

        let owned_gap = spectral_gap(&diag);
        let scratch_gap = spectral_gap_into(&diag, &mut scratch);
        assert!(approx_eq(owned_gap, scratch_gap));

        let owned_count = suggested_cluster_count(&diag);
        let scratch_count = suggested_cluster_count_into(&diag, &mut scratch);
        assert_eq!(owned_count, scratch_count);
    }

    #[test]
    fn suggested_cluster_count_at_least_one() {
        let diag = vec![0.7; 4];
        let count = suggested_cluster_count(&diag);
        assert!(count >= 1);
    }

    struct SpectrumDispatcher;

    impl OptimizerDispatcher for SpectrumDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 6);
            let restriction = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let v = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
            let one_fp = crate::hardware::dispatch_buffers::read_u32s(&inputs[5])[0];
            assert_eq!(one_fp, 1u32 << 16);
            let eigenvector: Vec<u32> = restriction
                .iter()
                .zip(v.iter())
                .map(|(&r, &value)| ((r as u64 * value as u64) >> 16) as u32)
                .collect();
            let lambda = eigenvector.iter().copied().max().unwrap_or(0);
            Ok(vec![
                u32_slice_to_le_bytes(&eigenvector),
                lambda.to_le_bytes().to_vec(),
            ])
        }
    }

    struct ExtraSpectrumDispatcher;

    impl OptimizerDispatcher for ExtraSpectrumDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Ok(vec![
                u32_slice_to_le_bytes(&[1]),
                u32_slice_to_le_bytes(&[1]),
                u32_slice_to_le_bytes(&[1]),
            ])
        }
    }

    struct TrailingLambdaDispatcher;

    impl OptimizerDispatcher for TrailingLambdaDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            Ok(vec![u32_slice_to_le_bytes(&[1]), vec![1, 0, 0, 0, 2]])
        }
    }

    #[test]
    fn fixed_via_dispatches_sheaf_spectrum() {
        let one = 1u32 << 16;
        let spectrum = dominant_spectrum_fixed_via(
            &SpectrumDispatcher,
            &[one, one / 2],
            &[8 * one, 4 * one],
            2,
            1,
            1,
        )
        .unwrap();
        assert_eq!(spectrum.eigenvector, vec![8 * one, 2 * one]);
        assert_eq!(spectrum.lambda, 8 * one);
    }

    #[test]
    fn fixed_via_reuses_dispatch_buffers_and_eigenvector_output() {
        let one = 1u32 << 16;
        let mut scratch = SheafSpectrumGpuScratch {
            inputs: vec![
                Vec::with_capacity(64),
                Vec::with_capacity(64),
                Vec::with_capacity(8),
                Vec::with_capacity(64),
                Vec::with_capacity(8),
                Vec::with_capacity(8),
            ],
        };
        let mut eigenvector = Vec::with_capacity(4);
        let input_caps = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let out_ptr = eigenvector.as_ptr();
        let lambda = dominant_spectrum_fixed_via_with_scratch_into(
            &SpectrumDispatcher,
            &[one, one / 2],
            &[8 * one, 4 * one],
            2,
            1,
            1,
            &mut scratch,
            &mut eigenvector,
        )
        .unwrap();
        assert_eq!(lambda, 8 * one);
        assert_eq!(eigenvector, vec![8 * one, 2 * one]);
        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_caps
        );
        assert_eq!(eigenvector.as_ptr(), out_ptr);
    }

    #[test]
    fn release_fixed_path_does_not_call_cpu_or_reference_helpers() {
        let source = include_str!("sheaf_spectral_clustering.rs");
        let start = source
            .find("pub fn dominant_spectrum_fixed_via")
            .expect("Fix: fixed path marker must exist");
        let end = source
            .find("\n/// Convenience: spectral gap")
            .expect("Fix: test-only CPU marker must exist");
        let release_path = &source[start..end];
        assert!(!release_path.contains("_cpu"));
        assert!(!release_path.contains("reference_"));
        assert!(!release_path.contains("vec![0u32"));
        assert!(!release_path.contains("u32_slice_to_le_bytes("));
    }

    #[test]
    fn fixed_via_rejects_shape_mismatch() {
        let err = dominant_spectrum_fixed_via(&SpectrumDispatcher, &[1, 2, 3], &[1, 2], 2, 2, 1)
            .unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }

    #[test]
    fn fixed_via_rejects_extra_outputs() {
        let err =
            dominant_spectrum_fixed_via(&ExtraSpectrumDispatcher, &[1], &[1], 1, 1, 1).unwrap_err();
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn fixed_via_rejects_trailing_lambda_bytes() {
        let err = dominant_spectrum_fixed_via(&TrailingLambdaDispatcher, &[1], &[1], 1, 1, 1)
            .unwrap_err();
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }
}
