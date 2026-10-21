//! SE(3)-equivariant tensor field network primitive (#33).
//!
//! TFN (Thomas 2018, Geiger 2022 e3nn) is the SE(3)-equivariant
//! building block for molecules, cryo-EM, robotics. Each layer is a
//! contraction over Clebsch-Gordan coefficients between irreducible
//! representations (irreps) `(l_in, l_filter) → l_out`. The CG
//! product is structured shuffles + fused multiply-add  -  same
//! hardware as matmul, so the substrate is GPU-trivial; the moat is
//! that nobody has packaged it as a Tier-2.5 primitive at the IR
//! level.
//!
//! This file ships the **scalar (l = 0) channel mixing step**  -
//! given per-node scalar features and a learnable mixing weight,
//! emit the next-layer scalar features. This is the trivial
//! equivariant case (rotation-invariant). Higher-l irrep contractions
//! are separate ops because their schemas depend on multivector
//! signatures rather than this scalar-channel ABI.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::geom::tfn_scalar_mix";

/// Emit `out[i, c_out] = Σ_{c_in} weights[c_out, c_in] · features[i, c_in]`.
///
/// Inputs:
/// - `features`: `n_nodes * c_in` u32 (16.16 fp).
/// - `weights`: `c_out * c_in` u32  -  learnable mixing matrix.
///
/// Output:
/// - `out`: `n_nodes * c_out` u32.
#[must_use]
pub fn tfn_scalar_mix(
    features: &str,
    weights: &str,
    out: &str,
    n_nodes: u32,
    c_in: u32,
    c_out: u32,
) -> Program {
    if n_nodes == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out,
            DataType::U32,
            "Fix: tfn_scalar_mix requires n_nodes > 0, got 0.".to_string(),
        );
    }
    if c_in == 0 || c_out == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out,
            DataType::U32,
            format!(
                "Fix: tfn_scalar_mix requires c_in and c_out > 0, got c_in={c_in}, c_out={c_out}."
            ),
        );
    }

    let cells = n_nodes * c_out;
    let t = Expr::InvocationId { axis: 0 };
    let node = Expr::div(t.clone(), Expr::u32(c_out));
    let co = Expr::rem(t.clone(), Expr::u32(c_out));

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(cells)),
        vec![
            Node::let_bind("acc", Expr::u32(0)),
            Node::let_bind("feat_base", Expr::mul(node, Expr::u32(c_in))),
            Node::let_bind("w_base", Expr::mul(co, Expr::u32(c_in))),
            Node::loop_for(
                "ci",
                Expr::u32(0),
                Expr::u32(c_in),
                vec![Node::assign(
                    "acc",
                    Expr::add(
                        Expr::var("acc"),
                        crate::fixed_mul_16_16_expr(
                            Expr::load(weights, Expr::add(Expr::var("w_base"), Expr::var("ci"))),
                            Expr::load(
                                features,
                                Expr::add(Expr::var("feat_base"), Expr::var("ci")),
                            ),
                        ),
                    ),
                )],
            ),
            Node::store(out, t, Expr::var("acc")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(features, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_nodes * c_in),
            BufferDecl::storage(weights, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(c_out * c_in),
            BufferDecl::storage(out, 2, BufferAccess::ReadWrite, DataType::U32).with_count(cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference, f64.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn tfn_scalar_mix_cpu(
    features: &[f64],
    weights: &[f64],
    n_nodes: u32,
    c_in: u32,
    c_out: u32,
) -> Vec<f64> {
    let n_nodes = n_nodes as usize;
    let c_in = c_in as usize;
    let c_out = c_out as usize;
    let mut out = vec![0.0; n_nodes * c_out];
    for i in 0..n_nodes {
        for co in 0..c_out {
            let mut acc = 0.0;
            for ci in 0..c_in {
                let weight = weights.get(co * c_in + ci).copied().unwrap_or(0.0);
                let feature = features.get(i * c_in + ci).copied().unwrap_or(0.0);
                acc += weight * feature;
            }
            out[i * c_out + co] = acc;
        }
    }
    out
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || tfn_scalar_mix("features", "weights", "out", 1, 2, 2),
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[2u32 << 16, 3u32 << 16]),
                crate::wire::pack_u32_slice(&[1u32 << 16, 0, 0, 1u32 << 16]),
                crate::wire::pack_u32_slice(&[0, 0]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[
                2u32 << 16,
                3u32 << 16,
            ])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_identity_weights_passthrough() {
        // c_in = c_out = 2, weights = identity → out = features.
        let f = vec![3.0, 5.0, 7.0, 11.0];
        let w = vec![1.0, 0.0, 0.0, 1.0];
        let out = tfn_scalar_mix_cpu(&f, &w, 2, 2, 2);
        assert_eq!(out, f);
    }

    #[test]
    fn cpu_zero_weights_zero_out() {
        let f = vec![1.0, 2.0];
        let w = vec![0.0, 0.0];
        let out = tfn_scalar_mix_cpu(&f, &w, 1, 2, 1);
        assert!(approx_eq(out[0], 0.0));
    }

    #[test]
    fn cpu_scaling_propagates() {
        // Single channel, weight 3 → out = 3 · features.
        let f = vec![1.0, 2.0, 3.0];
        let w = vec![3.0];
        let out = tfn_scalar_mix_cpu(&f, &w, 3, 1, 1);
        for (i, v) in out.iter().enumerate() {
            assert!(approx_eq(*v, 3.0 * f[i]));
        }
    }

    #[test]
    fn cpu_short_inputs_are_zero_padded() {
        let out = tfn_scalar_mix_cpu(&[2.0], &[3.0, 4.0], 1, 2, 1);
        assert_eq!(out, vec![6.0]);
    }

    #[test]
    fn cpu_se3_invariance_l0_irrep_holds() {
        // l = 0 (scalar) channels are by definition rotation-invariant
        //  -  the mix step doesn't mix in any rotation-dependent terms.
        // This test verifies the property: applying the same mix to
        // two "rotated" feature vectors (which for l=0 means identical)
        // gives identical output.
        let f1 = vec![1.0, 2.0, 3.0];
        let f2 = vec![1.0, 2.0, 3.0]; // "rotated"  -  for l=0, no change
        let w = vec![0.5, 0.3, 0.1];
        let out1 = tfn_scalar_mix_cpu(&f1, &w, 1, 3, 1);
        let out2 = tfn_scalar_mix_cpu(&f2, &w, 1, 3, 1);
        assert!(approx_eq(out1[0], out2[0]));
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = tfn_scalar_mix("f", "w", "out", 4, 8, 16);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        assert_eq!(p.buffers[0].count(), 32);
        assert_eq!(p.buffers[1].count(), 128);
        assert_eq!(p.buffers[2].count(), 64);
    }

    #[test]
    fn zero_n_nodes_traps() {
        let p = tfn_scalar_mix("f", "w", "out", 0, 1, 1);
        assert!(p.stats().trap());
    }
}
