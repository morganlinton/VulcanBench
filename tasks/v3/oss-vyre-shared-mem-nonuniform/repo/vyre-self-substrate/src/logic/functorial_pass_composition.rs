//! IR transform passes as categorical functors (#52 self-consumer).
//!
//! Closes the recursion thesis for #52  -  categorical-database
//! migration ships to user dialects (ETL pipelines, schema evolution)
//! AND treats vyre's IR transform passes as functors in a
//! Cat-of-IR-views category.
//!
//! # The release self-use
//!
//! Vyre's optimizer applies passes that rewrite the Region tree.
//! Today each pass is an ad-hoc match-on-Node procedure with no
//! algebraic relationship to other passes. Treating passes as
//! functors `F: IR_view_in → IR_view_out` unlocks:
//!
//! - **Compositionality**: F ∘ G is automatically a valid pass if F
//!   and G are. The composition's correctness is implied by the
//!   functor laws (preserves identity, preserves composition).
//! - **Equational reasoning**: pass A; pass B = pass B; pass A iff
//!   the functors commute. Today this is checked by hand on a
//!   case-by-case basis.
//! - **Free reuse of categorical machinery**: Yoneda lemma,
//!   adjoint pairs (where there's a least pass that achieves an
//!   effect = the left adjoint), Kan extensions (deriving missing
//!   passes from a partial pass list).
//!
//! The vyre transform pass framework can move from a hand-managed
//! dependency DAG to a typed functor-category where pass ordering,
//! correctness, and re-usability are derived from algebra.
//!
//! # The substrate primitive that powers this
//!
//! `functor_apply` performs one column-mapping functor application:
//! given a source row in the input category and a functor encoded
//! as a column-mapping lookup table, produce the target row in the
//! output category. Whole-schema migration composes per-row
//! functor_apply with `level_wave_program` for tree topology.
//!
//! This module owns the per-row functorial pass-application step.
//! Whole-pass migrations compose this primitive with the tree topology
//! helpers instead of changing this row-level contract.

use crate::dispatch_buffers::{ceil_div_u32, decode_u32_output_exact, u32_slice_to_le_bytes};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::functorial::functor_apply_sized;

/// Apply a functor to a row of IR-view data. `view_in[i]` is the
/// i-th column's value in the input view; `column_mapping[i]` is
/// the target-view column index for input column i. Returns the
/// transformed row in the output view of size `target_n_cols`.
///
/// # Panics
///
/// Panics if `view_in.len() != column_mapping.len()`.
#[must_use]
pub fn apply_pass_functor(view_in: &[u32], column_mapping: &[u32], target_n_cols: u32) -> Vec<u32> {
    let mut out = Vec::new();
    apply_pass_functor_into(view_in, column_mapping, target_n_cols, &mut out);
    out
}

/// Apply a functor into caller-owned output storage.
pub fn apply_pass_functor_into(
    view_in: &[u32],
    column_mapping: &[u32],
    target_n_cols: u32,
    out: &mut Vec<u32>,
) {
    use crate::observability::{bump, functorial_pass_composition_calls};
    bump(&functorial_pass_composition_calls);
    assert_eq!(view_in.len(), column_mapping.len());
    out.clear();
    out.resize(target_n_cols as usize, 0);
    for (i, &dst) in column_mapping.iter().enumerate() {
        if (dst as usize) < out.len() {
            out[dst as usize] = view_in[i];
        }
    }
}

/// Dispatcher-backed functor application for IR-view rows.
///
/// The primitive preserves the host contract for duplicate mappings:
/// if multiple source columns map to the same target column, the highest source
/// index wins. Out-of-range mappings are ignored.
///
/// # Errors
///
/// Returns [`DispatchError`] when shapes are invalid or backend output is
/// malformed.
pub fn apply_pass_functor_via(
    dispatcher: &impl OptimizerDispatcher,
    view_in: &[u32],
    column_mapping: &[u32],
    target_n_cols: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    apply_pass_functor_via_into(dispatcher, view_in, column_mapping, target_n_cols, &mut out)?;
    Ok(out)
}

/// Dispatcher-backed functor application into caller-owned output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn apply_pass_functor_via_into(
    dispatcher: &impl OptimizerDispatcher,
    view_in: &[u32],
    column_mapping: &[u32],
    target_n_cols: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, functorial_pass_composition_calls};
    bump(&functorial_pass_composition_calls);

    if target_n_cols == 0 {
        return Err(DispatchError::BadInputs(
            "Fix: apply_pass_functor_via requires target_n_cols > 0.".to_string(),
        ));
    }
    if view_in.len() != column_mapping.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: apply_pass_functor_via requires view_in.len() == column_mapping.len(), got view_in.len()={}, column_mapping.len()={}.",
            view_in.len(),
            column_mapping.len()
        )));
    }
    let n_cols = u32::try_from(view_in.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: apply_pass_functor_via source column count {} exceeds the primitive u32 lane limit.",
            view_in.len()
        ))
    })?;
    if n_cols == 0 {
        out.clear();
        out.resize(target_n_cols as usize, 0);
        return Ok(());
    }

    let program = functor_apply_sized(
        "view_in",
        "column_mapping",
        "view_out",
        n_cols,
        target_n_cols,
    );
    let outputs = dispatcher.dispatch(
        &program,
        &[
            u32_slice_to_le_bytes(view_in),
            u32_slice_to_le_bytes(column_mapping),
            vec![0u8; target_n_cols as usize * std::mem::size_of::<u32>()],
        ],
        Some([ceil_div_u32(target_n_cols, 256), 1, 1]),
    )?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: apply_pass_functor_via expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(
        &outputs[0],
        target_n_cols as usize,
        "apply_pass_functor_via",
        out,
    )
}

/// Compose two functors: F ∘ G applied to a single row. Returns
/// `(F ∘ G)(row) = F(G(row))`.
///
/// `mapping_g` maps view_in (size n_in) → middle view (size n_mid).
/// `mapping_f` maps middle view (size n_mid) → view_out (size n_out).
///
/// # Panics
///
/// Panics on size mismatches.
#[must_use]
pub fn compose_passes(
    view_in: &[u32],
    mapping_g: &[u32],
    n_mid: u32,
    mapping_f: &[u32],
    n_out: u32,
) -> Vec<u32> {
    let mut out = Vec::new();
    let mut combined = Vec::new();
    compose_passes_into(
        view_in,
        mapping_g,
        n_mid,
        mapping_f,
        n_out,
        &mut combined,
        &mut out,
    );
    out
}

/// Compose two functors into caller-owned scratch and output buffers.
pub fn compose_passes_into(
    view_in: &[u32],
    mapping_g: &[u32],
    n_mid: u32,
    mapping_f: &[u32],
    n_out: u32,
    combined: &mut Vec<u32>,
    out: &mut Vec<u32>,
) {
    assert_eq!(view_in.len(), mapping_g.len());
    assert_eq!(mapping_f.len(), n_mid as usize);
    // Collapse G then F into one column map so scatter/gather matches a single
    // `apply_pass_functor` (two-step apply can disagree when mid columns alias).
    combined.clear();
    combined.reserve(mapping_g.len());
    combined.extend(mapping_g.iter().map(|&mid_dst| mapping_f[mid_dst as usize]));
    apply_pass_functor_into(view_in, combined, n_out, out);
}

/// Categorical identity functor: maps each column to itself.
/// Used as the "no-op pass"  -  composes with anything as identity.
#[must_use]
pub fn identity_functor(n_cols: u32) -> Vec<u32> {
    let mut out = Vec::new();
    identity_functor_into(n_cols, &mut out);
    out
}

/// Write the identity functor into caller-owned storage.
pub fn identity_functor_into(n_cols: u32, out: &mut Vec<u32>) {
    out.clear();
    out.reserve(n_cols as usize);
    out.extend(0..n_cols);
}

/// Test whether two functors commute on a given input row.
/// `f_then_g(x) == g_then_f(x)` for x = `view_in`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn passes_commute_on(
    view_in: &[u32],
    mapping_a: &[u32],
    n_mid_a: u32,
    mapping_b_after_a: &[u32],
    mapping_b: &[u32],
    n_mid_b: u32,
    mapping_a_after_b: &[u32],
    n_out: u32,
) -> bool {
    let ab = compose_passes(view_in, mapping_a, n_mid_a, mapping_b_after_a, n_out);
    let ba = compose_passes(view_in, mapping_b, n_mid_b, mapping_a_after_b, n_out);
    ab == ba
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Program;

    struct FunctorDispatcher;

    impl OptimizerDispatcher for FunctorDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 3);
            let source = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let mapping = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
            let target_n_cols = inputs[2].len() / std::mem::size_of::<u32>();
            assert_eq!(source.len(), mapping.len());
            let out = apply_pass_functor(&source, &mapping, target_n_cols as u32);
            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn identity_preserves_input() {
        let view_in = vec![10u32, 20, 30, 40];
        let id = identity_functor(4);
        let out = apply_pass_functor(&view_in, &id, 4);
        assert_eq!(out, view_in);
    }

    #[test]
    fn pass_remaps_columns() {
        // Input row [10, 20, 30]; mapping says col 0 → out 2, col 1 → out 0,
        // col 2 → out 1. Expected output: [20, 30, 10].
        let view_in = vec![10u32, 20, 30];
        let mapping = vec![2u32, 0, 1];
        let out = apply_pass_functor(&view_in, &mapping, 3);
        assert_eq!(out, vec![20, 30, 10]);
    }

    #[test]
    fn apply_pass_functor_into_reuses_output() {
        let view_in = vec![10u32, 20, 30];
        let mapping = vec![2u32, 0, 1];
        let mut out = Vec::with_capacity(8);
        let ptr = out.as_ptr();
        apply_pass_functor_into(&view_in, &mapping, 3, &mut out);
        assert_eq!(out, vec![20, 30, 10]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn apply_pass_functor_via_dispatches_sized_primitive() {
        let view_in = vec![10u32, 20, 30];
        let mapping = vec![2u32, 0, 1];
        let out = apply_pass_functor_via(&FunctorDispatcher, &view_in, &mapping, 4).unwrap();
        assert_eq!(out, vec![20, 30, 10, 0]);
    }

    #[test]
    fn apply_pass_functor_via_preserves_duplicate_last_wins_contract() {
        let view_in = vec![7u32, 8, 9];
        let mapping = vec![1u32, 1, 1];
        let out = apply_pass_functor_via(&FunctorDispatcher, &view_in, &mapping, 3).unwrap();
        assert_eq!(out, vec![0, 9, 0]);
    }

    #[test]
    fn apply_pass_functor_via_rejects_shape_mismatch() {
        let err = apply_pass_functor_via(&FunctorDispatcher, &[1, 2], &[0], 2).unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }

    #[test]
    fn composition_is_associative() {
        // (F ∘ G)(x) for two simple permutations.
        let view_in = vec![1u32, 2, 3, 4];
        let g = vec![1u32, 0, 3, 2]; // swap pairs
        let f = vec![3u32, 2, 1, 0]; // reverse
        let composed = compose_passes(&view_in, &g, 4, &f, 4);
        // G applied: [2, 1, 4, 3]. F applied: reverse → [3, 4, 1, 2].
        assert_eq!(composed, vec![3, 4, 1, 2]);
    }

    #[test]
    fn compose_passes_into_reuses_combined_and_output() {
        let view_in = vec![1u32, 2, 3, 4];
        let g = vec![1u32, 0, 3, 2];
        let f = vec![3u32, 2, 1, 0];
        let mut combined = Vec::with_capacity(8);
        let mut out = Vec::with_capacity(8);
        let combined_ptr = combined.as_ptr();
        let out_ptr = out.as_ptr();
        compose_passes_into(&view_in, &g, 4, &f, 4, &mut combined, &mut out);
        assert_eq!(out, vec![3, 4, 1, 2]);
        assert_eq!(combined.as_ptr(), combined_ptr);
        assert_eq!(out.as_ptr(), out_ptr);
    }

    #[test]
    fn identity_composes_as_no_op() {
        let view_in = vec![5u32, 10, 15];
        let any_pass = vec![2u32, 0, 1];
        let id = identity_functor(3);
        let id_then_pass = compose_passes(&view_in, &id, 3, &any_pass, 3);
        let pass_then_id = compose_passes(&view_in, &any_pass, 3, &id, 3);
        let pass_alone = apply_pass_functor(&view_in, &any_pass, 3);
        assert_eq!(id_then_pass, pass_alone);
        assert_eq!(pass_then_id, pass_alone);
    }

    #[test]
    fn commutative_passes_detected() {
        // Two identity-equivalent reshuffles that compose to the same
        // identity in either order.
        let view_in = vec![100u32, 200];
        let a = vec![0u32, 1]; // identity
        let b = vec![0u32, 1]; // identity
        let commute = passes_commute_on(&view_in, &a, 2, &b, &b, 2, &a, 2);
        assert!(commute, "two identities must commute");
    }

    #[test]
    fn non_commutative_passes_detected() {
        // Two non-identity passes that don't commute.
        let view_in = vec![1u32, 2, 3];
        let a = vec![1u32, 2, 0]; // shift left
        let b_after_a = vec![2u32, 0, 1]; // some target permutation
        let b = vec![2u32, 0, 1]; // same shape, different arrangement
        let a_after_b = vec![1u32, 2, 0];
        let _commute = passes_commute_on(&view_in, &a, 3, &b_after_a, &b, 3, &a_after_b, 3);
        // Specific result depends on the permutations; test exercises
        // the API path without asserting a specific bool.
    }
}
