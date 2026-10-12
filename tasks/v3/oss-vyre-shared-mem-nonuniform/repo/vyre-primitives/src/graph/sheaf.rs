//! Sheaf neural network primitive  -  sheaf Laplacian application (#31).
//!
//! Sheaf neural networks (Bodnar-Di Giovanni 2022, Hansen-Gebhart 2023)
//! generalize GNNs from "all nodes share one feature space" to "each
//! edge carries restriction maps between heterogeneous node spaces."
//! The sheaf Laplacian is a block matrix where the (i, j) block is
//! `F_{ij}^T F_{ij}` (composition of restriction maps).
//!
//! This file ships the **block-diagonal sheaf Laplacian apply step**  -
//! given block-encoded restriction maps `F_{ij}` and a per-node
//! feature stalk, propagate one diffusion step:
//!
//! ```text
//!   y_i = Σ_j F_{ij}^T (F_{ij} x_i - F_{ji} x_j)
//! ```
//!
//! This step is the heart of sheaf diffusion. Each lane handles one
//! node's outgoing aggregation. The restriction maps `F_{ij}` are
//! supplied by the caller as a flat block-tensor.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | `vyre-libs::ml::heterophilic_gnn` consumers | heterophilic graph learning |
//! | `vyre-libs::security::call_graph_sheaf` consumers | typed call-graph anomalies |
//! | `vyre-foundation::transform` dispatch-sheaf analysis | vyre's dispatch graph is heterophilic; sheaf diffusion predicts where fusion fails |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::graph::sheaf_diffusion_step";

/// Emit the diagonal sheaf-Laplacian step.
///
/// Inputs:
/// - `stalks`: `n_nodes * d` u32 (16.16 fp). Per-node `d`-dim feature
///   vector (`d` = stalk dimension).
/// - `restriction_diag`: `n_nodes * d` u32  -  the diagonal of each
///   per-node restriction-map composition `F_{ii}^T F_{ii}` reduced to
///   diagonal form (caller computes block-diagonal restriction; this
///   primitive operates on the diagonal-block reduction).
/// - `damping_scaled`: 1-element u32  -  diffusion step size in 16.16.
///
/// Output:
/// - `stalks_next`: `n_nodes * d` u32.
///
/// Per-cell rule:
///   `stalks_next[i, k] = stalks[i, k] - damping · restriction_diag[i, k] · stalks[i, k]`
///
/// = `(1 - damping · restriction_diag) · stalks`
///
/// This is the diagonal-form approximation that's correct when the
/// restriction maps are simultaneously diagonalizable. Full off-
/// diagonal sheaf-Laplacian application composes from this primitive
/// plus a graph-traversal step (#5 chebyshev_filter on the off-
/// diagonal part).
#[must_use]
pub fn sheaf_diffusion_step(
    stalks: &str,
    restriction_diag: &str,
    damping_scaled: &str,
    stalks_next: &str,
    n_nodes: u32,
    d: u32,
) -> Program {
    match try_sheaf_diffusion_step(
        stalks,
        restriction_diag,
        damping_scaled,
        stalks_next,
        n_nodes,
        d,
    ) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, stalks_next, DataType::U32, error),
    }
}

/// Emit the diagonal sheaf-Laplacian step with checked stalk tensor sizing.
pub fn try_sheaf_diffusion_step(
    stalks: &str,
    restriction_diag: &str,
    damping_scaled: &str,
    stalks_next: &str,
    n_nodes: u32,
    d: u32,
) -> Result<Program, String> {
    if n_nodes == 0 {
        return Err("Fix: sheaf_diffusion_step requires n_nodes > 0, got 0.".to_string());
    }
    if d == 0 {
        return Err(format!(
            "Fix: sheaf_diffusion_step requires d > 0, got {d}."
        ));
    }

    let cells = checked_stalk_cells(n_nodes, d)?;
    let t = Expr::InvocationId { axis: 0 };

    // delta = damping · restriction_diag[t] · stalks[t]
    // stalks_next[t] = stalks[t] - delta
    let s = Expr::load(stalks, t.clone());
    let r = Expr::load(restriction_diag, t.clone());
    let d_v = Expr::load(damping_scaled, Expr::u32(0));
    let damped_r = crate::fixed_mul_16_16_expr(d_v, r);
    let delta = crate::fixed_mul_16_16_expr(damped_r, s.clone());
    let value = Expr::sub(s, delta);

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(cells)),
        vec![Node::store(stalks_next, t, value)],
    )];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(stalks, 0, BufferAccess::ReadOnly, DataType::U32).with_count(cells),
            BufferDecl::storage(restriction_diag, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(cells),
            BufferDecl::storage(damping_scaled, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
            BufferDecl::storage(stalks_next, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    ))
}

fn checked_stalk_cells(n_nodes: u32, d: u32) -> Result<u32, String> {
    n_nodes.checked_mul(d).ok_or_else(|| {
        format!(
            "sheaf_diffusion_step n_nodes={n_nodes} d={d} overflows stalk tensor cell count. Fix: shard the sheaf domain before GPU dispatch."
        )
    })
}

/// CPU reference (f64).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sheaf_diffusion_step_cpu(
    stalks: &[f64],
    restriction_diag: &[f64],
    damping: f64,
) -> Vec<f64> {
    try_sheaf_diffusion_step_cpu(stalks, restriction_diag, damping)
        .unwrap_or_else(|error| panic!("{error}"))
}

/// Fallible CPU reference (f64).
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_sheaf_diffusion_step_cpu(
    stalks: &[f64],
    restriction_diag: &[f64],
    damping: f64,
) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    try_sheaf_diffusion_step_cpu_into(stalks, restriction_diag, damping, &mut out)?;
    Ok(out)
}

/// CPU reference (f64), writing into caller-owned storage.
///
/// Clears `out` and reuses its allocation so iterative diffusion loops do not
/// allocate a new vector on every step.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sheaf_diffusion_step_cpu_into(
    stalks: &[f64],
    restriction_diag: &[f64],
    damping: f64,
    out: &mut Vec<f64>,
) {
    try_sheaf_diffusion_step_cpu_into(stalks, restriction_diag, damping, out)
        .unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible CPU reference (f64), writing into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_sheaf_diffusion_step_cpu_into(
    stalks: &[f64],
    restriction_diag: &[f64],
    damping: f64,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    let n = stalks.len().min(restriction_diag.len());
    if n > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            n - out.len(),
            "sheaf diffusion CPU oracle",
            "sheaf_diffusion_step_cpu_into",
        )?;
    }
    out.clear();
    out.extend(
        stalks
            .iter()
            .zip(restriction_diag.iter())
            .take(n)
            .map(|(&s, &r)| s - damping * r * s),
    );
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || sheaf_diffusion_step("stalks", "restriction_diag", "damping", "stalks_next", 1, 1),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[10u32 << 16]),
                to_bytes(&[1u32 << 16]),
                to_bytes(&[1u32 << 15]),
                to_bytes(&[0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[5u32 << 16])]]
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
    fn cpu_zero_damping_holds_stalks() {
        let s = vec![1.0, 2.0, 3.0];
        let r = vec![0.5, 0.5, 0.5];
        let out = sheaf_diffusion_step_cpu(&s, &r, 0.0);
        assert_eq!(out, s);
    }

    #[test]
    fn cpu_unit_restriction_full_damp_zeros() {
        let s = vec![10.0, 20.0];
        let r = vec![1.0, 1.0];
        let out = sheaf_diffusion_step_cpu(&s, &r, 1.0);
        assert!(approx_eq(out[0], 0.0));
        assert!(approx_eq(out[1], 0.0));
    }

    #[test]
    fn cpu_partial_damping_decreases_magnitude() {
        let s = vec![10.0];
        let r = vec![0.5];
        let out = sheaf_diffusion_step_cpu(&s, &r, 0.5);
        // delta = 0.5 · 0.5 · 10 = 2.5; out = 10 - 2.5 = 7.5
        assert!(approx_eq(out[0], 7.5));
    }

    #[test]
    fn cpu_mismatched_inputs_truncate_to_complete_pairs() {
        let out = sheaf_diffusion_step_cpu(&[10.0, 4.0], &[0.5], 1.0);
        assert_eq!(out, vec![5.0]);
    }

    #[test]
    fn checked_cpu_ref_reuses_output_and_truncates_stale_tail() {
        let mut out = Vec::with_capacity(4);
        out.extend_from_slice(&[99.0, 98.0, 97.0, 96.0]);
        let capacity = out.capacity();

        try_sheaf_diffusion_step_cpu_into(&[10.0, 4.0], &[0.5, 0.25], 1.0, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - checked sheaf CPU oracle should reuse caller-owned storage");

        assert_eq!(out.len(), 2);
        assert!(approx_eq(out[0], 5.0));
        assert!(approx_eq(out[1], 3.0));
        assert_eq!(out.capacity(), capacity);

        try_sheaf_diffusion_step_cpu_into(&[10.0], &[0.5], 1.0, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - checked sheaf CPU oracle should truncate stale output");

        assert_eq!(out, vec![5.0]);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn generated_cpu_oracle_reuses_storage_and_matches_closed_form() {
        let mut out = Vec::new();
        for case in 0..4096usize {
            let stalk_len = case % 97;
            let diag_len = (case / 7) % 97;
            let n = stalk_len.min(diag_len);
            let damping = ((case % 31) as f64 - 15.0) / 17.0;
            let stalks: Vec<f64> = (0..stalk_len)
                .map(|idx| ((idx * 13 + case) % 211) as f64 / 11.0 - 7.0)
                .collect();
            let restriction_diag: Vec<f64> = (0..diag_len)
                .map(|idx| ((idx * 17 + case * 3) % 157) as f64 / 19.0 - 3.0)
                .collect();

            try_sheaf_diffusion_step_cpu_into(&stalks, &restriction_diag, damping, &mut out)
                .expect("Fix: caller must pre-size buffers; use fallible reserve or return ResourceExhausted - generated sheaf CPU oracle should reserve and evaluate");

            assert_eq!(
                out.len(),
                n,
                "case {case}: output must truncate to complete stalk/restriction pairs"
            );
            for idx in 0..n {
                let expected = stalks[idx] - damping * restriction_diag[idx] * stalks[idx];
                assert!(
                    approx_eq(out[idx], expected),
                    "case {case} idx {idx}: expected {expected}, got {}",
                    out[idx]
                );
            }
        }
    }

    #[test]
    fn cpu_iterations_decay_to_zero_under_full_restriction() {
        // With r=1 and damping ∈ (0, 1), repeated application drives
        // stalks toward 0.
        let mut s = vec![1.0];
        let r = vec![1.0];
        for _ in 0..100 {
            s = sheaf_diffusion_step_cpu(&s, &r, 0.1);
        }
        assert!(s[0].abs() < 1e-3);
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = sheaf_diffusion_step("s", "rd", "dmp", "sn", 4, 8);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["s", "rd", "dmp", "sn"]);
        assert_eq!(p.buffers[0].count(), 32);
        assert_eq!(p.buffers[1].count(), 32);
        assert_eq!(p.buffers[2].count(), 1);
        assert_eq!(p.buffers[3].count(), 32);
    }

    #[test]
    fn zero_n_nodes_traps() {
        let p = sheaf_diffusion_step("s", "rd", "dmp", "sn", 0, 1);
        assert!(p.stats().trap());
    }

    #[test]
    fn zero_d_traps() {
        let p = sheaf_diffusion_step("s", "rd", "dmp", "sn", 1, 0);
        assert!(p.stats().trap());
    }

    #[test]
    fn checked_builder_rejects_stalk_tensor_overflow() {
        let error = try_sheaf_diffusion_step("s", "rd", "dmp", "sn", u32::MAX, 2)
            .expect_err("checked sheaf builder must reject stalk tensor overflow");

        assert!(
            error.contains("overflows stalk tensor cell count"),
            "error should describe the stalk tensor overflow: {error}"
        );
    }

    #[test]
    fn legacy_builder_does_not_panic_on_stalk_tensor_overflow() {
        let program = sheaf_diffusion_step("s", "rd", "dmp", "sn", u32::MAX, 2);

        assert!(program.stats().trap());
    }

    #[test]
    fn sheaf_builder_source_has_checked_sizing_without_panics() {
        let source = include_str!("sheaf.rs");
        let builder_source = source
            .split("pub fn sheaf_diffusion_step(")
            .nth(1)
            .expect("Fix: sheaf diffusion builder source must be present")
            .split("/// CPU reference")
            .next()
            .expect("Fix: sheaf diffusion builder source must precede CPU oracle");

        assert!(
            builder_source.contains("pub fn try_sheaf_diffusion_step(")
                && builder_source.contains("checked_stalk_cells")
                && !builder_source.contains(concat!("panic", "!("))
                && !builder_source.contains(".unwrap_or_else("),
            "Fix: sheaf_diffusion_step must expose checked release sizing and avoid production panics."
        );
    }

    #[test]
    fn sheaf_cpu_source_uses_fallible_reusable_storage() {
        let source = include_str!("sheaf.rs");
        let cpu_source = source
            .split("/// CPU reference (f64).")
            .nth(1)
            .expect("Fix: sheaf CPU source must be present")
            .split("#[cfg(feature = \"inventory-registry\")]")
            .next()
            .expect("Fix: sheaf CPU source must precede registry entry");

        assert!(
            cpu_source.contains("try_sheaf_diffusion_step_cpu_into")
                && cpu_source.contains("crate::graph::scratch::reserve_graph_items")
                && cpu_source.contains("out.capacity()")
                && !cpu_source.contains("fn reserve_sheaf_cpu_vec")
                && !cpu_source.contains("Vec::with_capacity")
                && !cpu_source.contains(".reserve("),
            "Fix: sheaf CPU oracle must use fallible reusable storage instead of infallible per-step allocation."
        );
    }
}
