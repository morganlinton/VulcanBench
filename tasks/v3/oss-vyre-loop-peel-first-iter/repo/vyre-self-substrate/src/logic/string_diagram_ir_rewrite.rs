//! Vyre IR Region tree as a string diagram (#53 self-consumer).
//!
//! Closes the recursion thesis for #53  -  string-diagram tensor
//! compilation ships to user dialects (quantum circuits, monoidal
//! tensor networks, ZX-calculus) AND IS the substrate semantics for
//! vyre's IR.
//!
//! # The release self-use
//!
//! Selinger's (2010) string diagrams are the visual + algebraic
//! language of monoidal categories. Each diagram is built from:
//!
//! - **Boxes**: morphisms (functions f: A → B). In vyre = each Region.
//! - **Wires**: types (objects A in the category). In vyre = buffer
//!   bindings between Regions.
//! - **Composition** ∘: stack boxes vertically (sequential
//!   dependence). In vyre = nested Regions in entry order.
//! - **Tensor product** ⊗: place boxes side-by-side (parallel
//!   independence). In vyre = sibling Regions sharing no buffers.
//!
//! Vyre's Region tree IS a string diagram in
//! `Cat(GPU buffers, Programs)`. Making this explicit means every
//! optimizer rewrite (region_inline, fusion, fission) is a
//! string-diagram rewrite  -  the equational laws of monoidal
//! categories give us free correctness proofs.
//!
//! # Concrete payoffs
//!
//! 1. **Coherence theorems for free**: associativity of `∘` and `⊗`,
//!    naturality of swap, are baked into the diagram model. Today
//!    these are checked by hand in each pass.
//! 2. **Adjoint pairs as duality**: backward-pass synthesis (gradient
//!    computation) is the dagger-functor in compact closed
//!    categories. Once the IR is a string diagram, `vyre-frontend-c` can
//!    derive backward-pass kernels for free.
//! 3. **Equational rewriting**: the ZX calculus has 7 rewrite rules
//!    that are complete for monoidal-category equivalence. Vyre's
//!    optimizer reduces from ~30 hand-curated passes to 7
//!    algebraic rules + a confluent rewriting strategy.
//!
//! # Algorithm
//!
//! `monoidal_compose(f, g)` is sequential composition `g ∘ f`  -
//! exactly the matrix-product semantics over the buffer-passing
//! contract between two Regions. For 0.6 we ship the per-arrow
//! composition step. The full ZX-calculus rewrite engine ships in
//! 1.0.

use crate::dispatch_buffers::{
    ceil_div_u32, checked_product_count, decode_u32_output_exact, ensure_input_slots,
    write_u32_slice_le_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::string_diagram::monoidal_compose;
#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::graph::string_diagram::monoidal_compose_cpu_into;

/// Reusable buffers for string-diagram IR rewrites.
#[derive(Debug, Default)]
pub struct StringDiagramRewriteScratch {
    #[cfg(any(test, feature = "cpu-parity"))]
    gf: Vec<f64>,
    #[cfg(any(test, feature = "cpu-parity"))]
    h_after_gf: Vec<f64>,
    #[cfg(any(test, feature = "cpu-parity"))]
    hg: Vec<f64>,
    #[cfg(any(test, feature = "cpu-parity"))]
    hg_after_f: Vec<f64>,
    dispatch_inputs: Vec<Vec<u8>>,
}

impl StringDiagramRewriteScratch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Sequential composition of two IR-arrow morphisms. `f` has shape
/// `a × b`, `g` has shape `b × c`. Returns `g ∘ f` with shape
/// `a × c`.
///
/// In vyre IR terms: `f` describes how Region F transforms its
/// `a`-dimensional input buffer into a `b`-dimensional intermediate;
/// `g` describes how Region G transforms the intermediate into the
/// `c`-dimensional output. The composed arrow describes the fused
/// F+G transformation in one step.
///
/// # Panics
///
/// Panics on size mismatches.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn compose_ir_arrows(f: &[f64], g: &[f64], a: u32, b: u32, c: u32) -> Vec<f64> {
    let mut out = Vec::new();
    reference_compose_ir_arrows_into(f, g, a, b, c, &mut out);
    out
}

/// Sequential composition using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_compose_ir_arrows_into(
    f: &[f64],
    g: &[f64],
    a: u32,
    b: u32,
    c: u32,
    out: &mut Vec<f64>,
) {
    use crate::observability::{bump, string_diagram_ir_rewrite_calls};
    bump(&string_diagram_ir_rewrite_calls);
    monoidal_compose_cpu_into(f, g, a, b, c, out);
}

/// Primitive-native fixed-point production path for sequential IR-arrow
/// composition.
///
/// `f_fixed` has shape `a x b`, `g_fixed` has shape `b x c`, and all
/// values are 16.16 u32 lanes. The dispatcher runs [`monoidal_compose`] and
/// returns the composed `a x c` arrow.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape validation fails, lane counts overflow,
/// or the backend returns malformed output.
pub fn compose_ir_arrows_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    f_fixed: &[u32],
    g_fixed: &[u32],
    a: u32,
    b: u32,
    c: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = StringDiagramRewriteScratch::default();
    let mut out = Vec::new();
    compose_ir_arrows_fixed_via_with_scratch_into(
        dispatcher,
        f_fixed,
        g_fixed,
        a,
        b,
        c,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Primitive-native fixed-point IR-arrow composition into caller-owned output.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn compose_ir_arrows_fixed_via_into(
    dispatcher: &impl OptimizerDispatcher,
    f_fixed: &[u32],
    g_fixed: &[u32],
    a: u32,
    b: u32,
    c: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = StringDiagramRewriteScratch::default();
    compose_ir_arrows_fixed_via_with_scratch_into(
        dispatcher,
        f_fixed,
        g_fixed,
        a,
        b,
        c,
        &mut scratch,
        out,
    )
}

/// Primitive-native fixed-point IR-arrow composition with reusable dispatch
/// input storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn compose_ir_arrows_fixed_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    f_fixed: &[u32],
    g_fixed: &[u32],
    a: u32,
    b: u32,
    c: u32,
    scratch: &mut StringDiagramRewriteScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, string_diagram_ir_rewrite_calls};
    bump(&string_diagram_ir_rewrite_calls);

    let f_cells = checked_product_count(a, b, "a", "b", "compose_ir_arrows_fixed_via f")?;
    let g_cells = checked_product_count(b, c, "b", "c", "compose_ir_arrows_fixed_via g")?;
    let out_cells = checked_product_count(a, c, "a", "c", "compose_ir_arrows_fixed_via out")?;
    let out_cells_u32 = u32::try_from(out_cells).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: compose_ir_arrows_fixed_via a*c exceeds the primitive u32 lane limit for a={a}, c={c}."
        ))
    })?;
    if f_fixed.len() != f_cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: compose_ir_arrows_fixed_via requires f_fixed.len() == a*b, got len={}, expected={f_cells}.",
            f_fixed.len()
        )));
    }
    if g_fixed.len() != g_cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: compose_ir_arrows_fixed_via requires g_fixed.len() == b*c, got len={}, expected={g_cells}.",
            g_fixed.len()
        )));
    }

    let program = monoidal_compose("f", "g", "out", a, b, c);
    ensure_input_slots(&mut scratch.dispatch_inputs, 2);
    write_u32_slice_le_bytes(&mut scratch.dispatch_inputs[0], f_fixed);
    write_u32_slice_le_bytes(&mut scratch.dispatch_inputs[1], g_fixed);
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.dispatch_inputs[..2],
        Some([ceil_div_u32(out_cells_u32, 256), 1, 1]),
    )?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: compose_ir_arrows_fixed_via expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], out_cells, "compose_ir_arrows_fixed_via", out)
}

/// Identity arrow on dimension `n`. Composes with any arrow as the
/// identity  -  `id ∘ f = f` and `f ∘ id = f`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn identity_arrow(n: u32) -> Vec<f64> {
    let mut out = Vec::new();
    identity_arrow_into(n, &mut out);
    out
}

/// Build an identity arrow using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn identity_arrow_into(n: u32, out: &mut Vec<f64>) {
    let n_us = n as usize;
    out.clear();
    out.resize(n_us * n_us, 0.0);
    for i in 0..n_us {
        out[i * n_us + i] = 1.0;
    }
}

/// Test that composition is associative: `(h ∘ g) ∘ f == h ∘ (g ∘ f)`.
/// Returns true when the two associativities agree to numerical
/// precision. Foundational coherence law for monoidal categories.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn composition_associates(
    f: &[f64],
    g: &[f64],
    h: &[f64],
    a: u32,
    b: u32,
    c: u32,
    d: u32,
) -> bool {
    let mut scratch = StringDiagramRewriteScratch::new();
    composition_associates_with_scratch(f, g, h, a, b, c, d, &mut scratch)
}

/// Associativity check using caller-owned scratch buffers.
#[must_use]
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn composition_associates_with_scratch(
    f: &[f64],
    g: &[f64],
    h: &[f64],
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    scratch: &mut StringDiagramRewriteScratch,
) -> bool {
    reference_compose_ir_arrows_into(f, g, a, b, c, &mut scratch.gf);
    reference_compose_ir_arrows_into(&scratch.gf, h, a, c, d, &mut scratch.h_after_gf);
    reference_compose_ir_arrows_into(g, h, b, c, d, &mut scratch.hg);
    reference_compose_ir_arrows_into(f, &scratch.hg, a, b, d, &mut scratch.hg_after_f);
    let tol = 1e-9_f64;
    scratch
        .h_after_gf
        .iter()
        .zip(scratch.hg_after_f.iter())
        .all(|(a, b)| (a - b).abs() < tol * (1.0 + a.abs() + b.abs()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_foundation::ir::Program;

    fn approx_eq_vec(a: &[f64], b: &[f64]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        a.iter()
            .zip(b.iter())
            .all(|(x, y)| (x - y).abs() < 1e-9 * (1.0 + x.abs() + y.abs()))
    }

    #[test]
    fn identity_left_unit() {
        // id ∘ f = f
        let f = vec![1.0, 2.0, 3.0, 4.0]; // 2x2
        let id = identity_arrow(2);
        let composed = compose_ir_arrows(&f, &id, 2, 2, 2);
        assert!(approx_eq_vec(&composed, &f));
    }

    #[test]
    fn identity_right_unit() {
        // f ∘ id = f
        let f = vec![1.0, 2.0, 3.0, 4.0];
        let id = identity_arrow(2);
        let composed = compose_ir_arrows(&id, &f, 2, 2, 2);
        assert!(approx_eq_vec(&composed, &f));
    }

    #[test]
    fn composition_associativity_holds() {
        // (h ∘ g) ∘ f = h ∘ (g ∘ f) for arbitrary 2x2 matrices.
        let f = vec![1.0, 0.5, -0.25, 0.5];
        let g = vec![0.5, 0.5, 0.5, -0.5];
        let h = vec![1.0, 0.0, 0.0, 1.0];
        assert!(composition_associates(&f, &g, &h, 2, 2, 2, 2));
    }

    #[test]
    fn rectangular_composition_dimensions() {
        // f: 2x3, g: 3x4 → composed: 2x4.
        let f = vec![1.0; 6];
        let g = vec![1.0; 12];
        let composed = compose_ir_arrows(&f, &g, 2, 3, 4);
        assert_eq!(composed.len(), 8);
    }

    #[test]
    fn identity_arrow_size_matches() {
        let id = identity_arrow(3);
        assert_eq!(id.len(), 9);
        // Diagonal = 1.0, off-diagonal = 0.0.
        assert_eq!(id[0], 1.0);
        assert_eq!(id[4], 1.0);
        assert_eq!(id[8], 1.0);
        assert_eq!(id[1], 0.0);
        assert_eq!(id[3], 0.0);
    }

    #[test]
    fn reusable_outputs_preserve_associativity() {
        let f = vec![1.0, 0.5, -0.25, 0.5];
        let g = vec![0.5, 0.5, 0.5, -0.5];
        let h = vec![1.0, 0.0, 0.0, 1.0];
        let mut scratch = StringDiagramRewriteScratch::new();
        assert!(composition_associates_with_scratch(
            &f,
            &g,
            &h,
            2,
            2,
            2,
            2,
            &mut scratch
        ));
    }

    struct ComposeDispatcher;

    impl OptimizerDispatcher for ComposeDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 2);
            let f = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let g = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
            assert_eq!(f.len(), 4);
            assert_eq!(g.len(), 4);
            let mut out = vec![0u32; 4];
            for i in 0..2 {
                for j in 0..2 {
                    let mut acc = 0u32;
                    for k in 0..2 {
                        acc = acc.saturating_add(
                            ((f[i * 2 + k] as u64 * g[k * 2 + j] as u64) >> 16) as u32,
                        );
                    }
                    out[i * 2 + j] = acc;
                }
            }
            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn fixed_via_dispatches_monoidal_compose() {
        let one = 1u32 << 16;
        let two = 2u32 << 16;
        let out = compose_ir_arrows_fixed_via(
            &ComposeDispatcher,
            &[one, two, 0, one],
            &[one, 0, two, one],
            2,
            2,
            2,
        )
        .unwrap();
        assert_eq!(out, vec![5 * one, 2 * one, 2 * one, one]);
    }

    #[test]
    fn fixed_via_reuses_dispatch_buffers_and_output() {
        let one = 1u32 << 16;
        let mut scratch = StringDiagramRewriteScratch {
            dispatch_inputs: vec![Vec::with_capacity(64), Vec::with_capacity(64)],
            ..StringDiagramRewriteScratch::default()
        };
        let mut out = Vec::with_capacity(8);
        let f_ptr = scratch.dispatch_inputs[0].as_ptr();
        let g_ptr = scratch.dispatch_inputs[1].as_ptr();
        let out_ptr = out.as_ptr();
        compose_ir_arrows_fixed_via_with_scratch_into(
            &ComposeDispatcher,
            &[one, 0, 0, one],
            &[one, 0, 0, one],
            2,
            2,
            2,
            &mut scratch,
            &mut out,
        )
        .unwrap();
        assert_eq!(out, vec![one, 0, 0, one]);
        assert_eq!(scratch.dispatch_inputs[0].as_ptr(), f_ptr);
        assert_eq!(scratch.dispatch_inputs[1].as_ptr(), g_ptr);
        assert_eq!(out.as_ptr(), out_ptr);
    }

    #[test]
    fn release_fixed_path_does_not_call_cpu_or_reference_helpers() {
        let source = include_str!("string_diagram_ir_rewrite.rs");
        let start = source
            .find("pub fn compose_ir_arrows_fixed_via")
            .expect("Fix: fixed path marker must exist");
        let end = source
            .find("\n/// Identity arrow on dimension")
            .expect("Fix: test-only CPU path marker must exist");
        let release_path = &source[start..end];
        assert!(!release_path.contains("_cpu"));
        assert!(!release_path.contains("reference_"));
        assert!(!release_path.contains("u32_slice_to_le_bytes("));
    }

    #[test]
    fn fixed_via_rejects_shape_mismatch() {
        let err =
            compose_ir_arrows_fixed_via(&ComposeDispatcher, &[1, 2, 3], &[1, 2, 3, 4], 2, 2, 2)
                .unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }
}
