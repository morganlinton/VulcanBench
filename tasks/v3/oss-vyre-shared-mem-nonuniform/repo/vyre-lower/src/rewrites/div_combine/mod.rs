//! Div-chain combine  -  fold `Div(Div(x, Lit(a)), Lit(b))` to
//! `Div(x, Lit(a * b))` (wrap-checked, divisor non-zero) for unsigned
//! floor division. Identity:
//!     ⌊⌊x / a⌋ / b⌋ = ⌊x / (a * b)⌋ for x ≥ 0, a > 0, b > 0.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4
//! (algebraic simplification family). Companion to `add_combine`,
//! `sub_combine`, `mul_combine`, `bitwise_combine`.
//!
//! Pattern rewritten (when both Lits are U32, both > 0, and a * b
//! doesn't wrap, and the inner Div has exactly one consumer):
//! - `Div(Div(x, Lit(a)), Lit(b))` → `Div(x, Lit(a * b))`
//!
//! Out-of-scope: zero divisors (preserved unchanged so the runtime
//! div-by-zero trap stays observable), wrapping product, multi-consumer
//! inner Div, signed division (I32 left to a future pass), and the
//! rotated forms `Div(Lit, Div(...))` which require non-trivial
//! reasoning over partial quotients.
//!
//! Recurses. Idempotent. Wired immediately after `sub_combine` in
//! `CANONICAL_REWRITE_PASSES`.

use crate::rewrites::rhs_lit_chain::{combine_rhs_lit_chain, RhsLitChainRule};
use crate::KernelDescriptor;
use vyre_foundation::ir::BinOp;

#[must_use]
pub fn div_combine(desc: &KernelDescriptor) -> KernelDescriptor {
    combine_rhs_lit_chain(
        desc,
        RhsLitChainRule {
            op: BinOp::Div,
            combine_literals: |a, b| {
                if a == 0 || b == 0 {
                    None
                } else {
                    a.checked_mul(b)
                }
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrites::rhs_lit_chain::test_support::{
        assert_multi_consumer_rhs_chain_left_alone, assert_rhs_chain_combines,
        assert_rhs_chain_left_alone, assert_rhs_chain_recurses_into_child,
        assert_rhs_chain_rewrite_is_idempotent,
    };
    use vyre_foundation::ir::BinOp;

    const DESC_ID: &str = "div_combine_test";

    #[test]
    fn div_chain_combines_when_no_wrap_and_nonzero() {
        assert_rhs_chain_combines(DESC_ID, div_combine, BinOp::Div, 3, 5, 15);
    }

    #[test]
    fn wrapping_product_left_alone() {
        assert_rhs_chain_left_alone(
            DESC_ID,
            div_combine,
            BinOp::Div,
            0x1_0000,
            0x1_0000,
            "Fix: refuse on overflow.",
        );
    }

    #[test]
    fn zero_divisor_left_alone() {
        assert_rhs_chain_left_alone(
            DESC_ID,
            div_combine,
            BinOp::Div,
            0,
            5,
            "Fix: never absorb a div-by-zero into a fold.",
        );
    }

    #[test]
    fn outer_zero_divisor_left_alone() {
        assert_rhs_chain_left_alone(
            DESC_ID,
            div_combine,
            BinOp::Div,
            5,
            0,
            "Fix: outer div-by-zero must remain observable.",
        );
    }

    #[test]
    fn inner_div_with_multiple_consumers_left_alone() {
        assert_multi_consumer_rhs_chain_left_alone(DESC_ID, div_combine, BinOp::Div, 3, 5);
    }

    #[test]
    fn rewrite_is_idempotent() {
        assert_rhs_chain_rewrite_is_idempotent(DESC_ID, div_combine, BinOp::Div, 3, 5);
    }

    #[test]
    fn recurses_into_child_bodies() {
        assert_rhs_chain_recurses_into_child(DESC_ID, div_combine, BinOp::Div, 4, 6, 24);
    }
}
