//! Strength reduction rewrite.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A.4 item A34.
//!
//! Patterns rewritten:
//! - `Mul(x, lit_pow2)` → `Shl(x, lit_log2)` (u32/i32; left-or-right operand)
//! - `Div(x, lit_pow2)` → `Shr(x, lit_log2)` (u32 only  -  signed div has rounding issues)
//! - `Mod(x, lit_pow2)` → `BitAnd(x, lit_minus_1)` (u32 only; same reason)
//!
//! Phase-1 conservative: only u32 unsigned forms. Float versions and
//! signed Div/Mod power-of-2 reductions are phase-2 territory because
//! they need additional checks for negative-input semantics.

use super::body_index::BodyIndex;
use super::literal::ResultAllocator;
use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use vyre_foundation::ir::BinOp;
use vyre_foundation::optimizer::algebraic_rules::strength_reduce_power_of_two_shift;

#[must_use]
pub fn strength_reduce(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    let mut allocator = ResultAllocator::for_body_tree(&out.body);
    out.body = strength_reduce_body(out.body, &mut allocator);
    out
}

fn strength_reduce_body(mut body: KernelBody, allocator: &mut ResultAllocator) -> KernelBody {
    let index = BodyIndex::new(&body);

    // Two-pass: first decide the rewrite per op, then apply.
    enum Rewrite {
        Mul(u32, u32, u32), // (other_op_id, log2_value, op_index)
        Div(u32, u32, u32), // (lhs_op_id, log2_value, op_index)
        Mod(u32, u32, u32), // (lhs_op_id, mask_value=lit-1, op_index)
    }
    let mut rewrites: Vec<Rewrite> = Vec::new();
    for (idx, op) in body.ops.iter().enumerate() {
        let bin = match &op.kind {
            KernelOpKind::BinOpKind(b) => *b,
            _ => continue,
        };
        if op.operands.len() != 2 {
            continue;
        }
        let lhs = op.operands[0];
        let rhs = op.operands[1];
        let lhs_lit = index.u32_lit(&body, lhs);
        let rhs_lit = index.u32_lit(&body, rhs);
        match bin {
            BinOp::Mul => {
                if let Some((other_id, lit)) = either_lit(lhs, rhs, lhs_lit, rhs_lit) {
                    if let Some(log2) = power_of_2_log(lit) {
                        rewrites.push(Rewrite::Mul(other_id, log2, idx as u32));
                    }
                }
            }
            BinOp::Div => {
                if let Some(lit) = rhs_lit {
                    if let Some(log2) = power_of_2_log(lit) {
                        rewrites.push(Rewrite::Div(lhs, log2, idx as u32));
                    }
                }
            }
            BinOp::Mod => {
                if let Some(lit) = rhs_lit {
                    if power_of_2_log(lit).is_some() {
                        rewrites.push(Rewrite::Mod(lhs, lit - 1, idx as u32));
                    }
                }
            }
            _ => {}
        }
    }

    // Apply rewrites: synthesize new Literal ops first, then patch the
    // BinOp at the recorded index.
    for r in rewrites {
        let (kind, other_id, lit_value, op_idx) = match r {
            Rewrite::Mul(o, log2, idx) => (BinOp::Shl, o, log2, idx),
            Rewrite::Div(lhs, log2, idx) => (BinOp::Shr, lhs, log2, idx),
            Rewrite::Mod(lhs, mask, idx) => (BinOp::BitAnd, lhs, mask, idx),
        };
        let synth_id = allocator.push_literal(
            &mut body.ops,
            &mut body.literals,
            LiteralValue::U32(lit_value),
        );
        body.ops[op_idx as usize].kind = KernelOpKind::BinOpKind(kind);
        body.ops[op_idx as usize].operands = vec![other_id, synth_id];
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| strength_reduce_body(child, allocator))
        .collect();

    body
}

fn either_lit(
    lhs: u32,
    rhs: u32,
    lhs_lit: Option<u32>,
    rhs_lit: Option<u32>,
) -> Option<(u32, u32)> {
    // Prefer the side that's a power of 2 (so we don't bail just because
    // the "preferred" side happens to not be a pow2). When both are
    // literals and only one is pow2, pick that one. When both are pow2,
    // prefer rhs for canonical form. descriptor_const_fold collapses the
    // literal-literal case in a later pass anyway.
    let lhs_p2 = lhs_lit.and_then(power_of_2_log).is_some();
    let rhs_p2 = rhs_lit.and_then(power_of_2_log).is_some();
    if rhs_p2 {
        rhs_lit.map(|r| (lhs, r))
    } else if lhs_p2 {
        lhs_lit.map(|l| (rhs, l))
    } else {
        None
    }
}

fn power_of_2_log(v: u32) -> Option<u32> {
    strength_reduce_power_of_two_shift(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp};

    fn binop_kernel(op: BinOp, lit_lhs: u32, lit_rhs: u32) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(op),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(lit_lhs), LiteralValue::U32(lit_rhs)],
            },
        }
    }

    #[test]
    fn empty_kernel_no_change() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let out = strength_reduce(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn mul_by_pow2_becomes_shl() {
        // x * 8 → x << 3
        let out = strength_reduce(&binop_kernel(BinOp::Mul, 5, 8));
        // Find the BinOp op and confirm it's now Shl.
        let bin_op = out.body.ops.iter().find_map(|o| {
            if let KernelOpKind::BinOpKind(b) = &o.kind {
                Some(*b)
            } else {
                None
            }
        });
        assert_eq!(bin_op, Some(BinOp::Shl));
    }

    #[test]
    fn mul_by_non_pow2_unchanged() {
        let out = strength_reduce(&binop_kernel(BinOp::Mul, 5, 7));
        let bin_op = out.body.ops.iter().find_map(|o| {
            if let KernelOpKind::BinOpKind(b) = &o.kind {
                Some(*b)
            } else {
                None
            }
        });
        assert_eq!(bin_op, Some(BinOp::Mul));
    }

    #[test]
    fn mul_by_pow2_lhs_also_works() {
        // 8 * x → x << 3 (commutative)
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Mul),
                        operands: vec![0, 1], // 0 = lit "8", 1 = lit "5"
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(8), LiteralValue::U32(5)],
            },
        };
        let out = strength_reduce(&desc);
        let bin_op = out.body.ops.iter().find_map(|o| {
            if let KernelOpKind::BinOpKind(b) = &o.kind {
                Some(*b)
            } else {
                None
            }
        });
        assert_eq!(bin_op, Some(BinOp::Shl));
    }

    #[test]
    fn div_by_pow2_becomes_shr() {
        // x / 16 → x >> 4
        let out = strength_reduce(&binop_kernel(BinOp::Div, 100, 16));
        let bin_op = out.body.ops.iter().find_map(|o| {
            if let KernelOpKind::BinOpKind(b) = &o.kind {
                Some(*b)
            } else {
                None
            }
        });
        assert_eq!(bin_op, Some(BinOp::Shr));
    }

    #[test]
    fn div_constant_on_lhs_unchanged() {
        // 16 / x  -  can't strength-reduce (constant on wrong side).
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Div),
                        operands: vec![0, 1], // 0=16 / 1=x
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(16), LiteralValue::U32(7)],
            },
        };
        // For this test pattern the rhs IS a literal too. Strength
        // reduction will fire (since rhs=7 is not pow2 → no change).
        let out = strength_reduce(&desc);
        let bin_op = out.body.ops.iter().find_map(|o| {
            if let KernelOpKind::BinOpKind(b) = &o.kind {
                Some(*b)
            } else {
                None
            }
        });
        assert_eq!(bin_op, Some(BinOp::Div));
    }

    #[test]
    fn mod_by_pow2_becomes_bitand() {
        // x % 4 → x & 3
        let out = strength_reduce(&binop_kernel(BinOp::Mod, 100, 4));
        let bin_op = out.body.ops.iter().find_map(|o| {
            if let KernelOpKind::BinOpKind(b) = &o.kind {
                Some(*b)
            } else {
                None
            }
        });
        assert_eq!(bin_op, Some(BinOp::BitAnd));
        // The mask should be 3 (i.e. 4 - 1).
        let last_lit = out.body.literals.last().unwrap();
        assert_eq!(*last_lit, LiteralValue::U32(3));
    }

    #[test]
    fn mul_by_zero_or_one_not_reduced() {
        // 0 isn't pow2 (we require >= 2). 1 isn't either.
        for lit in [0u32, 1] {
            let out = strength_reduce(&binop_kernel(BinOp::Mul, 5, lit));
            let bin_op = out.body.ops.iter().find_map(|o| {
                if let KernelOpKind::BinOpKind(b) = &o.kind {
                    Some(*b)
                } else {
                    None
                }
            });
            assert_eq!(
                bin_op,
                Some(BinOp::Mul),
                "lit {lit} should NOT trigger reduction"
            );
        }
    }

    #[test]
    fn power_of_2_log_helper() {
        assert_eq!(power_of_2_log(2), Some(1));
        assert_eq!(power_of_2_log(4), Some(2));
        assert_eq!(power_of_2_log(8), Some(3));
        assert_eq!(power_of_2_log(1024), Some(10));
        assert_eq!(power_of_2_log(0), None);
        assert_eq!(power_of_2_log(1), None);
        assert_eq!(power_of_2_log(3), None);
        assert_eq!(power_of_2_log(7), None);
    }

    #[test]
    fn strength_reduce_is_idempotent() {
        let desc = binop_kernel(BinOp::Mul, 5, 8);
        let once = strength_reduce(&desc);
        let twice = strength_reduce(&once);
        // After first pass the BinOp is Shl with a synthesized Literal
        // operand on RHS. Second pass shouldn't change anything because
        // Shl isn't in the reduction patterns.
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
        assert_eq!(once.body.literals.len(), twice.body.literals.len());
    }
}
