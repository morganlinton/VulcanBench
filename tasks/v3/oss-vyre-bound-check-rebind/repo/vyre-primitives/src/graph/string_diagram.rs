//! String diagram compilation primitive (#53).
//!
//! String diagrams (Selinger 2010, Coecke-Kissinger ZX) are the visual
//! language of monoidal categories  -  a generalized tensor network.
//! Recent work (Patterson 2022 DisCoPy) compiles them to numeric
//! tensor contractions.
//!
//! This file ships the **monoidal composition step** primitive  -
//! sequential composition `g · f` of two morphisms encoded as small
//! tensors `f: A → B` and `g: B → C`, producing `g · f: A → C`. This
//! is matrix multiplication with categorical intent carried in the
//! stable op id.

use vyre_foundation::ir::{DataType, Program};

use crate::fixed_u32_matmul::{checked_cells, fixed_u32_matmul_program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::graph::monoidal_compose";

/// Sequential composition step. Same shape as
/// [`crate::math::tensor_network::tn_pair_contract`]; ships under graph
/// because string diagrams are graphs of morphisms.
#[must_use]
pub fn monoidal_compose(f: &str, g: &str, out: &str, a: u32, b: u32, c: u32) -> Program {
    match try_monoidal_compose(f, g, out, a, b, c) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, out, DataType::U32, error),
    }
}

/// Sequential composition step with checked tensor cell counts.
pub fn try_monoidal_compose(
    f: &str,
    g: &str,
    out: &str,
    a: u32,
    b: u32,
    c: u32,
) -> Result<Program, String> {
    if a == 0 || b == 0 || c == 0 {
        return Err(format!(
            "Fix: monoidal_compose requires a, b, c > 0, got a={a}, b={b}, c={c}."
        ));
    }

    let f_cells = checked_cells("monoidal_compose f input", a, b)?;
    let g_cells = checked_cells("monoidal_compose g input", b, c)?;
    let cells = checked_cells("monoidal_compose output", a, c)?;
    Ok(fixed_u32_matmul_program(
        OP_ID, f, g, out, a, b, c, f_cells, g_cells, cells,
    ))
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn monoidal_compose_cpu(f: &[f64], g: &[f64], a: u32, b: u32, c: u32) -> Vec<f64> {
    try_monoidal_compose_cpu(f, g, a, b, c).unwrap_or_else(|error| panic!("{error}"))
}

/// Fallible CPU reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_monoidal_compose_cpu(
    f: &[f64],
    g: &[f64],
    a: u32,
    b: u32,
    c: u32,
) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    try_monoidal_compose_cpu_into(f, g, a, b, c, &mut out)?;
    Ok(out)
}

/// CPU reference using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn monoidal_compose_cpu_into(f: &[f64], g: &[f64], a: u32, b: u32, c: u32, out: &mut Vec<f64>) {
    try_monoidal_compose_cpu_into(f, g, a, b, c, out).unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible CPU reference using caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_monoidal_compose_cpu_into(
    f: &[f64],
    g: &[f64],
    a: u32,
    b: u32,
    c: u32,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    let a = a as usize;
    let b = b as usize;
    let c = c as usize;
    let _f_cells = a.checked_mul(b).ok_or_else(|| {
        "monoidal_compose CPU oracle f shape overflows cell count. Fix: reduce a*b before parity comparison.".to_string()
    })?;
    let _g_cells = b.checked_mul(c).ok_or_else(|| {
        "monoidal_compose CPU oracle g shape overflows cell count. Fix: reduce b*c before parity comparison.".to_string()
    })?;
    let out_cells = a.checked_mul(c).ok_or_else(|| {
        "monoidal_compose CPU oracle output shape overflows cell count. Fix: reduce a*c before parity comparison.".to_string()
    })?;
    if out_cells > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            out_cells - out.len(),
            "string diagram CPU oracle",
            "monoidal_compose CPU output",
        )?;
    }
    out.clear();
    out.resize(out_cells, 0.0);
    for i in 0..a {
        for j in 0..c {
            for k in 0..b {
                let f_value = f.get(i * b + k).copied().unwrap_or(0.0);
                let g_value = g.get(k * c + j).copied().unwrap_or(0.0);
                out[i * c + j] += f_value * g_value;
            }
        }
    }
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || monoidal_compose("f", "g", "out", 2, 2, 2),
        Some(|| {
            let one = 1u32 << 16;
            vec![vec![
                crate::wire::pack_u32_slice(&[one, 0, 0, one]),
                crate::wire::pack_u32_slice(&[2 * one, 3 * one, 5 * one, 7 * one]),
                crate::wire::pack_u32_slice(&[0; 4]),
            ]]
        }),
        Some(|| {
            let one = 1u32 << 16;
            vec![vec![crate::wire::pack_u32_slice(&[
                2 * one, 3 * one, 5 * one, 7 * one,
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
    fn cpu_identity_compose_passthrough() {
        let f = vec![1.0, 2.0, 3.0, 4.0];
        let i = vec![1.0, 0.0, 0.0, 1.0];
        let out = monoidal_compose_cpu(&f, &i, 2, 2, 2);
        assert_eq!(out, f);
    }

    #[test]
    fn cpu_short_inputs_are_zero_padded() {
        let out = monoidal_compose_cpu(&[2.0], &[3.0, 4.0], 1, 2, 2);
        assert_eq!(out, vec![6.0, 8.0]);
    }

    #[test]
    fn checked_cpu_ref_reuses_output_and_truncates_stale_tail() {
        let mut out = Vec::with_capacity(4);
        out.extend_from_slice(&[99.0, 98.0, 97.0, 96.0]);
        let capacity = out.capacity();

        try_monoidal_compose_cpu_into(&[2.0, 3.0], &[5.0, 7.0, 11.0, 13.0], 1, 2, 2, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - checked CPU oracle should reuse caller-owned storage");

        assert_eq!(out.len(), 2);
        assert!(approx_eq(out[0], 43.0));
        assert!(approx_eq(out[1], 53.0));
        assert_eq!(out.capacity(), capacity);

        try_monoidal_compose_cpu_into(&[4.0], &[6.0], 1, 1, 1, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - checked CPU oracle should truncate stale output cells");

        assert_eq!(out, vec![24.0]);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn checked_cpu_ref_preserves_output_on_reservation_failure() {
        let mut out = vec![1.0, 2.0, 3.0];
        let err = try_monoidal_compose_cpu_into(&[], &[], u32::MAX, 1, u32::MAX, &mut out)
            .expect_err("checked CPU oracle must reject impossible output reservations");

        assert!(
            err.contains("monoidal_compose CPU output") || err.contains("reserve"),
            "error should describe output reservation failure: {err}"
        );
        assert_eq!(out, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn cpu_associativity_holds() {
        // (h · g) · f = h · (g · f)
        let f = vec![1.0, 2.0]; // 1x2
        let g = vec![3.0, 4.0]; // 2x1
        let h = vec![5.0]; // 1x1
        let lhs_inner = monoidal_compose_cpu(&f, &g, 1, 2, 1); // 1x1
        let lhs = monoidal_compose_cpu(&lhs_inner, &h, 1, 1, 1); // 1x1
        let rhs_inner = monoidal_compose_cpu(&g, &h, 2, 1, 1); // 2x1
        let rhs = monoidal_compose_cpu(&f, &rhs_inner, 1, 2, 1); // 1x1
        assert!(approx_eq(lhs[0], rhs[0]));
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = monoidal_compose("f", "g", "h", 2, 3, 4);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        assert_eq!(p.buffers[0].count(), 6);
        assert_eq!(p.buffers[1].count(), 12);
        assert_eq!(p.buffers[2].count(), 8);
    }

    #[test]
    fn zero_a_traps() {
        let p = monoidal_compose("f", "g", "h", 0, 1, 1);
        assert!(p.stats().trap());
    }

    #[test]
    fn checked_monoidal_compose_rejects_zero_dimension() {
        let error = try_monoidal_compose("f", "g", "h", 0, 1, 1)
            .expect_err("checked monoidal compose builder must reject zero dimensions");

        assert!(
            error.contains("requires a, b, c > 0"),
            "error should describe the invalid tensor shape: {error}"
        );
    }

    #[test]
    fn checked_monoidal_compose_rejects_output_cell_overflow() {
        let error = try_monoidal_compose("f", "g", "h", u32::MAX, 1, 2)
            .expect_err("checked monoidal compose builder must reject output overflow");

        assert!(
            error.contains("overflows cell count"),
            "error should describe the output tensor overflow: {error}"
        );
    }

    #[test]
    fn legacy_monoidal_compose_does_not_panic_on_output_cell_overflow() {
        let program = monoidal_compose("f", "g", "h", u32::MAX, 1, 2);

        assert!(program.stats().trap());
    }

    #[test]
    fn monoidal_compose_source_has_checked_api_without_panics() {
        let source = include_str!("string_diagram.rs");
        let builder_source = source
            .split("/// Sequential composition step.")
            .nth(1)
            .expect("Fix: monoidal compose builder source must be present")
            .split("/// CPU reference.")
            .next()
            .expect("Fix: monoidal compose builder source must precede CPU oracle");

        assert!(
            builder_source.contains("pub fn try_monoidal_compose(")
                && !builder_source.contains(concat!("panic", "!("))
                && !builder_source.contains(".unwrap_or_else("),
            "Fix: monoidal_compose must expose checked release API and avoid production panics."
        );
    }

    #[test]
    fn monoidal_compose_cpu_source_uses_checked_reusable_output() {
        let source = include_str!("string_diagram.rs");
        let cpu_source = source
            .split("/// CPU reference.")
            .nth(1)
            .expect("Fix: monoidal compose CPU source must be present")
            .split("#[cfg(feature = \"inventory-registry\")]")
            .next()
            .expect("Fix: monoidal compose CPU source must precede registry entry");

        assert!(
            cpu_source.contains("pub fn try_monoidal_compose_cpu_into(")
                && cpu_source.contains("crate::graph::scratch::reserve_graph_items")
                && cpu_source.contains("out.capacity()")
                && !cpu_source.contains("out.resize(a * c, 0.0)")
                && !cpu_source.contains("Vec::with_capacity"),
            "Fix: monoidal_compose CPU oracle must use fallible caller-owned output storage."
        );
    }
}
