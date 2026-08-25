//! Regression for FINDING-GPU-11: region_inline flattened identically-named
//! Region-scoped `Let` bindings into colliding siblings, so
//! `fft_convolve_circular_complex` failed V032 ("duplicate sibling let binding
//! `u_re_s1_b0_k0` in the same region") after pre_lowering::optimize and the
//! backend refused to execute it. The fix wraps a flattened region's body in a
//! Node::Block when it declares top-level lets, preserving the scope.
//!
//! This reproduces the failure CPU-side (optimize -> validate); the original GPU
//! differential (vyre-driver-wgpu cat_a_gpu_differential::diff_universal_registry)
//! is the end-to-end check.

use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_libs::math::fft::fft_convolve_circular_complex;

#[test]
fn fft_convolve_optimizes_without_duplicate_sibling_let() {
    // n = 4 gives a 2-stage radix-2 FFT, so stage-1 butterflies emit the
    // `u_re_s1_b0_k0` bindings that previously collided after region inlining.
    let program = fft_convolve_circular_complex(
        "signal",
        "kernel",
        "signal_freq",
        "kernel_freq",
        "product_freq",
        "output",
        4,
    )
    .expect("fft_convolve_circular_complex must build for n=4 (power of two)");

    // Pre-fix this is where region_inline collapsed two stage-scoped `let
    // u_re_s1_b0_k0` into duplicate siblings.
    let optimized = optimize(program);

    let errors = vyre::ir::validate(&optimized);
    let messages: Vec<String> = errors.iter().map(|e| e.message().to_string()).collect();

    assert!(
        !messages.iter().any(|m| m.contains("duplicate sibling let")),
        "FINDING-GPU-11 regression: optimized fft_convolve must not contain duplicate \
         sibling let bindings (V032). Validation errors: {messages:?}"
    );
    // The op is well-formed structurally, so the optimized program must validate
    // cleanly — assert the full contract, not just the absence of V032.
    assert!(
        errors.is_empty(),
        "optimized fft_convolve_circular_complex must validate cleanly, got: {messages:?}"
    );
}
