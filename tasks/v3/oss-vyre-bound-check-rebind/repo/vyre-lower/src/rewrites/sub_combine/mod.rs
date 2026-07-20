//! Sub-chain combine  -  fold `Sub(Sub(x, Lit(a)), Lit(b))` to
//! `Sub(x, Lit(a + b))` (wrap-checked). Sub is non-commutative so
//! only the canonical right-chain form folds; `Sub(Lit, Sub(...))`
//! is left to `egraph_saturation`.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4
//! (algebraic simplification family). Companion to `add_combine`,
//! `mul_combine`, `bitwise_combine`.
//!
//! Pattern rewritten (when both Lits are U32 and a + b doesn't wrap,
//! and the inner Sub has exactly one consumer):
//! - `Sub(Sub(x, Lit(a)), Lit(b))` → `Sub(x, Lit(a + b))`
//!
//! Out-of-scope: wrapping addition of literal pair, multi-consumer
//! inner Sub, signed arithmetic (I32 left to a future pass), and the
//! commuted forms `Sub(Lit, Sub(x, Lit))` / `Sub(Lit, Sub(Lit, x))`
//! which require sign tracking.
//!
//! Recurses. Idempotent. Wired immediately after `mul_combine` in
//! `CANONICAL_REWRITE_PASSES`.

use crate::rewrites::rhs_lit_chain::{combine_rhs_lit_chain, RhsLitChainRule};
use crate::KernelDescriptor;
use vyre_foundation::ir::BinOp;

#[must_use]
pub fn sub_combine(desc: &KernelDescriptor) -> KernelDescriptor {
    combine_rhs_lit_chain(
        desc,
        RhsLitChainRule {
            op: BinOp::Sub,
            combine_literals: u32::checked_add,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrites::rhs_lit_chain::test_support::{
        assert_multi_consumer_rhs_chain_left_alone, assert_rhs_chain_combines,
        assert_rhs_chain_left_alone, assert_rhs_chain_recurses_into_child,
        assert_rhs_chain_rewrite_is_idempotent, binop, descriptor_with, empty_body, lit_u32,
        nonliteral_source, op_at,
    };
    use vyre_foundation::ir::BinOp;

    const DESC_ID: &str = "sub_combine_test";

    #[test]
    fn sub_chain_combines_when_no_wrap() {
        assert_rhs_chain_combines(DESC_ID, sub_combine, BinOp::Sub, 3, 5, 8);
    }

    #[test]
    fn wrapping_sum_left_alone() {
        assert_rhs_chain_left_alone(
            DESC_ID,
            sub_combine,
            BinOp::Sub,
            0xFFFF_FFFF,
            1,
            "Fix: refuse to combine when literal sum overflows u32.",
        );
    }

    #[test]
    fn inner_sub_with_multiple_consumers_left_alone() {
        assert_multi_consumer_rhs_chain_left_alone(DESC_ID, sub_combine, BinOp::Sub, 3, 5);
    }

    #[test]
    fn sub_with_lhs_lit_left_alone() {
        // (a - x) - b is NOT folded  -  handling that requires sign tracking.
        let mut body = empty_body();
        nonliteral_source(&mut body, 0);
        lit_u32(&mut body, 100, 1);
        binop(&mut body, BinOp::Sub, 1, 0, 2);
        lit_u32(&mut body, 5, 3);
        binop(&mut body, BinOp::Sub, 2, 3, 4);
        let desc = sub_combine(&descriptor_with(DESC_ID, body));
        let outer = op_at(&desc, 4);
        assert_eq!(
            outer.operands[0], 2,
            "Fix: Sub(Lit, x) inner shape must not fold via the sum path."
        );
    }

    #[test]
    fn rewrite_is_idempotent() {
        assert_rhs_chain_rewrite_is_idempotent(DESC_ID, sub_combine, BinOp::Sub, 3, 5);
    }

    #[test]
    fn recurses_into_child_bodies() {
        assert_rhs_chain_recurses_into_child(DESC_ID, sub_combine, BinOp::Sub, 4, 6, 10);
    }
}
