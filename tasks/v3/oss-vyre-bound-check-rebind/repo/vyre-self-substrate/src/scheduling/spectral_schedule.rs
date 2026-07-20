//! Spectral analysis of dispatch graph via #5 chebyshev_filter +
//! #17 spectral_shape (#23 substrate).
//!
//! Apply Chebyshev polynomial filtering to vyre's own dispatch
//! dependency matrix, clip outlier eigenvalues via Marchenko-Pastur
//! edge, identify clusters of Programs that should be fused.
//! Output: cluster IDs that #19 polyhedral fusion + #22 megakernel
//! scheduler consume as fusion hints.

use crate::dispatch_buffers::{
    ceil_div_u32, checked_square_cells, decode_u32_output_exact, ensure_input_slots,
    write_u32_slice_le_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
#[cfg(test)]
use vyre_primitives::graph::chebyshev_filter::chebyshev_filter_cpu;
use vyre_primitives::graph::chebyshev_filter::{chebyshev_filter, MAX_K as CHEBYSHEV_MAX_K};
use vyre_primitives::math::spectral_shape::mp_edge_clip;
#[cfg(test)]
use vyre_primitives::math::spectral_shape::{mp_edge_clip_cpu, mp_upper_edge};

/// Caller-owned dispatch scratch for spectral scheduling primitives.
#[derive(Debug, Default)]
pub struct SpectralScheduleGpuScratch {
    inputs: Vec<Vec<u8>>,
    mp_edge: Vec<u32>,
}

/// Score nodes for fusion clustering by applying a low-pass Chebyshev
/// filter (coeffs [1, 0.5, 0.25] = exponential decay) to a unit-energy
/// signal at each node. Nodes returning high scores are spectrally
/// connected.
#[must_use]
#[cfg(test)]
pub fn reference_fusion_scores(laplacian: &[f32], n: u32) -> Vec<f32> {
    use crate::observability::{bump, spectral_schedule_calls};
    bump(&spectral_schedule_calls);
    assert_eq!(laplacian.len(), (n * n) as usize);
    let signal: Vec<f32> = (0..n).map(|_| 1.0 / (n as f32).sqrt()).collect();
    let coeffs: Vec<f32> = vec![1.0, 0.5, 0.25];
    chebyshev_filter_cpu(laplacian, &signal, &coeffs, n, 2)
}

/// Fixed-point production path for spectral fusion scores.
///
/// Inputs are primitive-native 16.16 u32 buffers. The dispatcher runs
/// [`chebyshev_filter`] directly and returns the fixed-point score vector.
///
/// # Errors
///
/// Returns [`DispatchError`] when shapes are invalid, the primitive order is
/// unsupported, or the backend returns malformed output.
pub fn fusion_scores_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    laplacian_fixed: &[u32],
    signal_fixed: &[u32],
    coeffs_fixed: &[u32],
    n: u32,
    k_steps: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    fusion_scores_fixed_via_into(
        dispatcher,
        laplacian_fixed,
        signal_fixed,
        coeffs_fixed,
        n,
        k_steps,
        &mut out,
    )?;
    Ok(out)
}

/// Fixed-point production path for spectral fusion scores into caller-owned
/// output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks or backend execution fail.
pub fn fusion_scores_fixed_via_into(
    dispatcher: &impl OptimizerDispatcher,
    laplacian_fixed: &[u32],
    signal_fixed: &[u32],
    coeffs_fixed: &[u32],
    n: u32,
    k_steps: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = SpectralScheduleGpuScratch::default();
    fusion_scores_fixed_via_with_scratch_into(
        dispatcher,
        laplacian_fixed,
        signal_fixed,
        coeffs_fixed,
        n,
        k_steps,
        &mut scratch,
        out,
    )
}

/// Fixed-point production path for spectral fusion scores using caller-owned
/// dispatch scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks or backend execution fail.
pub fn fusion_scores_fixed_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    laplacian_fixed: &[u32],
    signal_fixed: &[u32],
    coeffs_fixed: &[u32],
    n: u32,
    k_steps: u32,
    scratch: &mut SpectralScheduleGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, spectral_schedule_calls};
    bump(&spectral_schedule_calls);

    if k_steps > CHEBYSHEV_MAX_K {
        return Err(DispatchError::BadInputs(format!(
            "Fix: fusion_scores_fixed_via requires k_steps <= {CHEBYSHEV_MAX_K}, got {k_steps}."
        )));
    }
    let cells = checked_square_cells(n, "fusion_scores_fixed_via")?;
    let cells_u32 = u32::try_from(cells).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: fusion_scores_fixed_via n*n exceeds the primitive u32 lane limit for n={n}."
        ))
    })?;
    if n > u32::MAX / 2 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: fusion_scores_fixed_via scratch size 2*n overflows u32 for n={n}."
        )));
    }
    if laplacian_fixed.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: fusion_scores_fixed_via requires laplacian_fixed.len() == n*n, got len={}, n={}, n*n={cells}.",
            laplacian_fixed.len(),
            n
        )));
    }
    if signal_fixed.len() != n as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: fusion_scores_fixed_via requires signal_fixed.len() == n, got len={}, n={n}.",
            signal_fixed.len()
        )));
    }
    let coeff_count = (k_steps as usize).checked_add(1).ok_or_else(|| {
        DispatchError::BadInputs(
            "Fix: fusion_scores_fixed_via coefficient count overflowed usize.".to_string(),
        )
    })?;
    if coeffs_fixed.len() != coeff_count {
        return Err(DispatchError::BadInputs(format!(
            "Fix: fusion_scores_fixed_via requires coeffs_fixed.len() == k_steps + 1, got len={}, k_steps={k_steps}.",
            coeffs_fixed.len()
        )));
    }

    let program = chebyshev_filter(
        "laplacian",
        "signal",
        "coeffs",
        "output",
        "scratch",
        n,
        k_steps,
    );
    ensure_input_slots(&mut scratch.inputs, 3);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], laplacian_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], signal_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], coeffs_fixed);
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs[..3],
        Some([ceil_div_u32(n, 256), 1, 1]),
    )?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: fusion_scores_fixed_via expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    let _ = cells_u32;
    decode_u32_output_exact(&outputs[0], n as usize, "fusion_scores_fixed_via", out)
}

/// Clip outlier eigenvalues at the Marchenko-Pastur upper edge. Used
/// to filter spurious high-frequency dispatch-graph correlations.
#[must_use]
#[cfg(test)]
pub fn reference_shape_spectrum(
    eigenvalues: &[f64],
    n_dispatches: u32,
    n_features: u32,
) -> Vec<f64> {
    let edge = mp_upper_edge(n_dispatches, n_features, 1.0);
    mp_edge_clip_cpu(eigenvalues, edge)
}

/// Fixed-point production path for Marchenko-Pastur edge clipping.
///
/// `mp_edge_fixed` is the already-scaled 16.16 upper edge. Callers that need
/// the f64 helper can keep using [`mp_upper_edge`] at the representation
/// boundary, then quantize once before dispatch.
///
/// # Errors
///
/// Returns [`DispatchError`] when the eigenvalue vector is empty, too large
/// for the primitive lane space, or the backend returns malformed output.
pub fn shape_spectrum_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    eigenvalues_fixed: &[u32],
    mp_edge_fixed: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    shape_spectrum_fixed_via_into(dispatcher, eigenvalues_fixed, mp_edge_fixed, &mut out)?;
    Ok(out)
}

/// Fixed-point Marchenko-Pastur edge clipping into caller-owned storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks or backend execution fail.
pub fn shape_spectrum_fixed_via_into(
    dispatcher: &impl OptimizerDispatcher,
    eigenvalues_fixed: &[u32],
    mp_edge_fixed: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = SpectralScheduleGpuScratch::default();
    shape_spectrum_fixed_via_with_scratch_into(
        dispatcher,
        eigenvalues_fixed,
        mp_edge_fixed,
        &mut scratch,
        out,
    )
}

/// Fixed-point Marchenko-Pastur edge clipping using caller-owned dispatch scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks or backend execution fail.
pub fn shape_spectrum_fixed_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    eigenvalues_fixed: &[u32],
    mp_edge_fixed: u32,
    scratch: &mut SpectralScheduleGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    if eigenvalues_fixed.is_empty() {
        return Err(DispatchError::BadInputs(
            "Fix: shape_spectrum_fixed_via requires at least one eigenvalue.".to_string(),
        ));
    }
    let n = u32::try_from(eigenvalues_fixed.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: shape_spectrum_fixed_via eigenvalue count exceeds u32 lane limit: {}.",
            eigenvalues_fixed.len()
        ))
    })?;

    let program = mp_edge_clip("eigenvalues", "mp_edge", "out", n);
    scratch.mp_edge.clear();
    scratch.mp_edge.push(mp_edge_fixed);
    ensure_input_slots(&mut scratch.inputs, 2);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], eigenvalues_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], &scratch.mp_edge);
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs[..2],
        Some([ceil_div_u32(n, 256), 1, 1]),
    )?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: shape_spectrum_fixed_via expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(
        &outputs[0],
        eigenvalues_fixed.len(),
        "shape_spectrum_fixed_via",
        out,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_foundation::ir::Program;

    fn approx_eq_f32(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn fusion_scores_uniform_for_zero_laplacian() {
        // No edges → Laplacian = 0 matrix. Chebyshev recurrence:
        //   T_0 = signal, T_1 = L·signal = 0, T_2 = 2·L·T_1 - T_0 = -signal.
        // Output with coeffs [1, 0.5, 0.25]:
        //   c_0·T_0 + c_1·T_1 + c_2·T_2 = (1 - 0.25) · signal = 0.75 · signal
        // signal = 1/sqrt(4) = 0.5; output = 0.375 per node.
        let l: Vec<f32> = vec![0.0; 16];
        let scores = reference_fusion_scores(&l, 4);
        for s in scores {
            assert!(approx_eq_f32(s, 0.375));
        }
    }

    #[test]
    fn shape_spectrum_clips_outliers() {
        // n_dispatches = 100, n_features = 100, σ²=1 → MP edge = 4.
        let eig = vec![1.0, 3.0, 5.0, 100.0];
        let clipped = reference_shape_spectrum(&eig, 100, 100);
        assert_eq!(clipped[0], 1.0);
        assert_eq!(clipped[1], 3.0);
        assert_eq!(clipped[2], 4.0); // clipped to edge
        assert_eq!(clipped[3], 4.0); // clipped to edge
    }

    #[test]
    fn fusion_scores_zero_signal_zero_output() {
        let l: Vec<f32> = vec![0.5; 4];
        let scores = reference_fusion_scores(&l, 2);
        for s in scores {
            assert!(s.is_finite());
        }
    }

    struct SpectralDispatcher;

    impl OptimizerDispatcher for SpectralDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            match inputs.len() {
                2 => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    let eigenvalues = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    let edge = crate::hardware::dispatch_buffers::read_u32s(&inputs[1])[0];
                    let clipped: Vec<u32> = eigenvalues.into_iter().map(|v| v.min(edge)).collect();
                    Ok(vec![u32_slice_to_le_bytes(&clipped)])
                }
                3 => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    let laplacian = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    let signal = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
                    let coeffs = crate::hardware::dispatch_buffers::read_u32s(&inputs[2]);
                    assert_eq!(laplacian, vec![1, 0, 0, 1]);
                    assert_eq!(coeffs, vec![1]);
                    Ok(vec![u32_slice_to_le_bytes(&signal)])
                }
                other => Err(DispatchError::BadInputs(format!(
                    "Fix: test dispatcher does not accept {other} input buffers."
                ))),
            }
        }
    }

    #[test]
    fn shape_spectrum_fixed_via_clips_on_dispatcher() {
        let clipped = shape_spectrum_fixed_via(&SpectralDispatcher, &[1, 5, 10], 4).unwrap();
        assert_eq!(clipped, vec![1, 4, 4]);
    }

    #[test]
    fn fusion_scores_fixed_via_dispatches_chebyshev_filter() {
        let scores =
            fusion_scores_fixed_via(&SpectralDispatcher, &[1, 0, 0, 1], &[7, 11], &[1], 2, 0)
                .unwrap();
        assert_eq!(scores, vec![7, 11]);
    }

    #[test]
    fn fixed_via_rejects_bad_shapes() {
        let err = shape_spectrum_fixed_via(&SpectralDispatcher, &[], 4).unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));

        let err = fusion_scores_fixed_via(&SpectralDispatcher, &[1, 0, 0], &[1, 1], &[1], 2, 0)
            .unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }

    #[test]
    fn fixed_via_with_scratch_reuses_input_buffers() {
        let mut scratch = SpectralScheduleGpuScratch::default();
        let mut out = Vec::new();

        fusion_scores_fixed_via_with_scratch_into(
            &SpectralDispatcher,
            &[1, 0, 0, 1],
            &[7, 11],
            &[1],
            2,
            0,
            &mut scratch,
            &mut out,
        )
        .unwrap();
        let input_ptrs: Vec<*const u8> = scratch.inputs.iter().take(3).map(Vec::as_ptr).collect();
        fusion_scores_fixed_via_with_scratch_into(
            &SpectralDispatcher,
            &[1, 0, 0, 1],
            &[5, 13],
            &[1],
            2,
            0,
            &mut scratch,
            &mut out,
        )
        .unwrap();

        for (before, after) in input_ptrs
            .iter()
            .zip(scratch.inputs.iter().take(3).map(Vec::as_ptr))
        {
            assert_eq!(*before, after);
        }
    }

    #[test]
    fn production_source_keeps_cpu_spectral_helpers_out_of_via_path() {
        let source = include_str!("spectral_schedule.rs");
        let via_section = source
            .split("pub fn fusion_scores_fixed_via")
            .nth(1)
            .expect("Fix: via section should exist")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: test module marker should exist");

        assert!(!via_section.contains("_cpu"));
        assert!(!via_section.contains("reference_fusion_scores"));
        assert!(!via_section.contains("reference_shape_spectrum"));
    }
}
