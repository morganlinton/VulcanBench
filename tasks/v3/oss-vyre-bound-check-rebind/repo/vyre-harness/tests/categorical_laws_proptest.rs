//! Property tests for categorical laws over functorial pass composition and IR rewrites.
use proptest::prelude::*;
use vyre_primitives::graph::sheaf::sheaf_diffusion_step_cpu;
use vyre_self_substrate::functorial_pass_composition::{
    apply_pass_functor, compose_passes, identity_functor,
};
use vyre_self_substrate::string_diagram_ir_rewrite::{
    compose_ir_arrows, composition_associates, identity_arrow,
};

fn approx_eq_vec(a: &[f64], b: &[f64]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let tol = 1e-9;
    a.iter()
        .zip(b.iter())
        .all(|(&x, &y)| (x - y).abs() < tol * (1.0 + x.abs() + y.abs()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10000))]

    #[test]
    fn law_1_functor_preservation(
        (view_in, n_mid, n_out, mapping_f, mapping_g) in (1..16usize, 1..16u32, 1..16u32).prop_flat_map(|(n_in, n_mid, n_out)| {
            (
                prop::collection::vec(0..100u32, n_in),
                Just(n_mid), Just(n_out),
                prop::collection::vec(0..n_mid, n_in),
                prop::collection::vec(0..n_out, n_mid as usize),
            )
        })
    ) {
        let mapping_gf: Vec<u32> = mapping_f.iter().map(|&i| mapping_g[i as usize]).collect();
        let direct = apply_pass_functor(&view_in, &mapping_gf, n_out);
        let composed = compose_passes(&view_in, &mapping_f, n_mid, &mapping_g, n_out);
        assert_eq!(direct, composed);
    }

    #[test]
    fn law_2_identity_laws(
        (f, a, b) in (1..8u32, 1..8u32).prop_flat_map(|(a, b)| {
            (prop::collection::vec(-10.0..10.0f64, (a * b) as usize), Just(a), Just(b))
        })
    ) {
        let id_a = identity_arrow(a);
        let id_b = identity_arrow(b);
        let right_id = compose_ir_arrows(&f, &id_b, a, b, b);
        assert!(approx_eq_vec(&right_id, &f));
        let left_id = compose_ir_arrows(&id_a, &f, a, a, b);
        assert!(approx_eq_vec(&left_id, &f));
    }

    #[test]
    fn law_3_associativity(
        (f, g, h, a, b, c, d) in (1..4u32, 1..4u32, 1..4u32, 1..4u32).prop_flat_map(|(a, b, c, d)| {
            (
                prop::collection::vec(-5.0..5.0f64, (a * b) as usize),
                prop::collection::vec(-5.0..5.0f64, (b * c) as usize),
                prop::collection::vec(-5.0..5.0f64, (c * d) as usize),
                Just(a), Just(b), Just(c), Just(d)
            )
        })
    ) {
        assert!(composition_associates(&f, &g, &h, a, b, c, d));
    }

    #[test]
    fn law_4_sheaf_coherence(
        stalks in prop::collection::vec(-10.0..10.0f64, 1..16),
        restriction_diag in prop::collection::vec(0.0..10.0f64, 1..16),
    ) {
        let n = stalks.len().min(restriction_diag.len());
        let s = &stalks[..n];
        let r = &restriction_diag[..n];
        let out_zero = sheaf_diffusion_step_cpu(s, r, 0.0);
        assert_eq!(out_zero, s);
    }

    #[test]
    fn law_5_yoneda_lemma_analog(
        (view_in, n_out, mapping) in (1..16usize, 1..16u32).prop_flat_map(|(n_in, n_out)| {
            (prop::collection::vec(0..100u32, n_in), Just(n_out), prop::collection::vec(0..n_out, n_in))
        })
    ) {
        let n_in = view_in.len() as u32;
        let id_in = identity_functor(n_in);
        let id_out = identity_functor(n_out);
        let f_after_id = compose_passes(&view_in, &id_in, n_in, &mapping, n_out);
        let f_alone = apply_pass_functor(&view_in, &mapping, n_out);
        assert_eq!(f_after_id, f_alone);
        let id_after_f = compose_passes(&view_in, &mapping, n_out, &id_out, n_out);
        assert_eq!(id_after_f, f_alone);
    }
}
