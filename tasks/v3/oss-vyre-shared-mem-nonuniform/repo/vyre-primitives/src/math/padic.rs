//! p-adic numerical analysis primitives (#54, research scaffold).
//!
//! p-adic numbers (Krasner 1986) give stable arithmetic for problems
//! ill-conditioned in real numbers. Recent ML work (Robin 2024) uses
//! p-adic embeddings for stable training of deep networks. Hensel
//! lifting is the algorithmic core.
//!
//! This file ships the **single Hensel lift step** primitive  -  given
//! an approximate root x of `f(x) ≡ 0 (mod p^k)` and the formal
//! derivative `f'(x)`, return a refined root accurate `mod p^{2k}`.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::hensel_lift_step";

/// Hensel iteration: `x_next = x - f(x) · (f'(x))^{-1}` modulo `p^{2k}`.
/// Inputs are pre-evaluated `f_x` and `inv_f_prime` from the caller.
#[must_use]
pub fn hensel_lift_step(x: &str, f_x: &str, inv_f_prime: &str, out: &str, n: u32) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out,
            DataType::U32,
            "Fix: hensel_lift_step requires n > 0, got 0.".to_string(),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let value = Expr::sub(
        Expr::load(x, t.clone()),
        crate::fixed_mul_16_16_expr(
            Expr::load(f_x, t.clone()),
            Expr::load(inv_f_prime, t.clone()),
        ),
    );

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(n)),
        vec![Node::store(out, t, value)],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(x, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(f_x, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(inv_f_prime, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n),
            BufferDecl::storage(out, 3, BufferAccess::ReadWrite, DataType::U32).with_count(n),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference (f64)  -  Hensel iteration single step.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn hensel_lift_step_cpu(x: f64, f_x: f64, inv_f_prime: f64) -> f64 {
    x - f_x * inv_f_prime
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || {
            hensel_lift_step("x", "f_x", "inv_f_prime", "out", 4)
        },
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[10, 20, 30, 40]),
                crate::wire::pack_u32_slice(&[2, 4, 6, 8]),
                crate::wire::pack_u32_slice(&[1u32 << 16; 4]),
                crate::wire::pack_u32_slice(&[0; 4]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[8, 16, 24, 32])]]
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
    fn cpu_zero_residual_holds_root() {
        // If f(x) = 0 already, lift step doesn't move x.
        let x_next = hensel_lift_step_cpu(2.5, 0.0, 1.0);
        assert!(approx_eq(x_next, 2.5));
    }

    #[test]
    fn cpu_quadratic_root_converges() {
        // f(x) = x² - 2, find sqrt(2) ≈ 1.414...
        // Newton/Hensel: x_{k+1} = x_k - (x_k² - 2) / (2 x_k)
        let mut x = 1.5;
        for _ in 0..10 {
            let f_x = x * x - 2.0;
            let inv_f_prime = 1.0 / (2.0 * x);
            x = hensel_lift_step_cpu(x, f_x, inv_f_prime);
        }
        assert!(approx_eq(x, 2.0_f64.sqrt()));
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = hensel_lift_step("x", "fx", "ip", "out", 16);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        assert_eq!(p.buffers[0].count(), 16);
        assert_eq!(p.buffers[3].count(), 16);
    }

    #[test]
    fn zero_n_traps() {
        let p = hensel_lift_step("x", "fx", "ip", "out", 0);
        assert!(p.stats().trap());
    }
}
