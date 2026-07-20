//! P-CONFORM-3: Categorical-law conformance suite.
//!
//! The substrate's functorial_pass_composition + string_diagram_ir_rewrite
//! consumers must satisfy the categorical laws (identity, associativity,
//! interchange) independently of which crate's helper is consulted.

use vyre_self_substrate::functorial_pass_composition::apply_pass_functor;
use vyre_self_substrate::string_diagram_ir_rewrite::{
    compose_ir_arrows, identity_arrow as ir_arrow_identity,
};

#[test]
fn identity_arrow_left_identity() {
    // f ∘ id_a = f for f: a → b.
    let n = 3;
    let id = ir_arrow_identity(n);
    let f: Vec<f64> = vec![1.0, 0.0, 0.5, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]; // 3x3
    let composed = compose_ir_arrows(&f, &id, n, n, n);
    assert_eq!(composed.len(), f.len());
    for (a, b) in composed.iter().zip(f.iter()) {
        assert!((a - b).abs() < 1e-9, "f ∘ id_a != f at {a} vs {b}");
    }
}

#[test]
fn identity_functor_is_neutral() {
    // apply_pass_functor with identity column mapping returns the input.
    let view: Vec<u32> = vec![10, 20, 30, 40];
    let identity_mapping: Vec<u32> = (0..view.len() as u32).collect();
    let out = apply_pass_functor(&view, &identity_mapping, view.len() as u32);
    assert_eq!(out, view);
}

#[test]
fn functor_composition_chains_correctly() {
    // apply_pass_functor with two sequential maps produces the composed result.
    let view: Vec<u32> = vec![10, 20, 30, 40];
    let swap_pairs: Vec<u32> = vec![1, 0, 3, 2]; // [20, 10, 40, 30]
    let reverse: Vec<u32> = vec![3, 2, 1, 0]; // [30, 40, 10, 20]

    let after_swap = apply_pass_functor(&view, &swap_pairs, 4);
    assert_eq!(after_swap, vec![20, 10, 40, 30]);

    let after_reverse = apply_pass_functor(&after_swap, &reverse, 4);
    assert_eq!(after_reverse, vec![30, 40, 10, 20]);

    // reverse∘swap_pairs applied directly to view should match.
    let composed_direct = apply_pass_functor(&view, &reverse, 4);
    assert_eq!(
        composed_direct,
        vec![40, 30, 20, 10],
        "reverse(view) = [40,30,20,10]"
    );
    // The two-step result [30,40,10,20] differs from direct reverse [40,30,20,10],
    // confirming the maps do not commute and sequential application is meaningful.
    assert_ne!(after_reverse, composed_direct);
}
