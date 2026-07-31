//! Algebraic canonicalization.
//!
//! For commutative `BinOp` ops, sort the two operand result-ids
//! ascending. `Add(r5, r1)` becomes `Add(r1, r5)`. Two structurally-
//! equal expressions written with operands in different orders
//! normalize to the same form, so the downstream `descriptor_cse` pass can merge
//! them; without this, CSE only catches operand-identical duplicates.
//!
//! Commutative ops handled: `Add`, `Mul`, `BitAnd`, `BitOr`, `BitXor`,
//! `Min`, `Max`, `Eq`, `Ne`, `WrappingAdd`.
//!
//! NOT commutative: `Sub`, `Div`, `Mod`, `Shl`, `Shr`, `Lt`, `Le`,
//! `Gt`, `Ge`  -  operand order matters semantically. Float Add/Mul
//! aren't strictly commutative under IEEE 754 NaN rules either, but
//! we don't model that distinction here (descriptor BinOp is type-
//! agnostic). Folding under floats with NaNs would change which NaN
//! payload propagates; in practice GPUs don't preserve NaN payloads
//! anyway, so this is harmless.

use vyre_foundation::ir::BinOp;

use crate::{KernelBody, KernelDescriptor, KernelOpKind};

#[must_use]
pub fn canonicalize(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    canonicalize_body(&mut out.body);
    out
}

fn canonicalize_body(body: &mut KernelBody) {
    for op in &mut body.ops {
        if let KernelOpKind::BinOpKind(bo) = &op.kind {
            if is_commutative(*bo) && op.operands.len() == 2 && op.operands[0] > op.operands[1] {
                op.operands.swap(0, 1);
            }
        }
    }
    for child in &mut body.child_bodies {
        canonicalize_body(child);
    }
}

fn is_commutative(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Add
            | BinOp::Mul
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor
            | BinOp::Min
            | BinOp::Max
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::WrappingAdd
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };

    fn empty_desc(ops: Vec<KernelOp>, literals: Vec<LiteralValue>) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals,
            },
        }
    }

    #[test]
    fn add_with_swapped_operands_normalizes() {
        let desc = empty_desc(
            vec![
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
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![1, 0],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(4)],
        );
        let out = canonicalize(&desc);
        assert_eq!(out.body.ops[2].operands, vec![0, 1]);
    }

    #[test]
    fn sub_is_not_commuted() {
        let desc = empty_desc(
            vec![
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
                    kind: KernelOpKind::BinOpKind(BinOp::Sub),
                    operands: vec![1, 0],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(4)],
        );
        let out = canonicalize(&desc);
        // Sub is NOT commutative  -  order preserved.
        assert_eq!(out.body.ops[2].operands, vec![1, 0]);
    }

    #[test]
    fn already_canonical_unchanged() {
        let desc = empty_desc(
            vec![
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
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(4)],
        );
        let out = canonicalize(&desc);
        assert_eq!(out.body.ops[2].operands, vec![0, 1]);
    }

    #[test]
    fn equal_operands_unchanged() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::BitAnd),
                    operands: vec![0, 0],
                    result: Some(1),
                },
            ],
            vec![LiteralValue::U32(7)],
        );
        let out = canonicalize(&desc);
        assert_eq!(out.body.ops[1].operands, vec![0, 0]);
    }

    #[test]
    fn enables_cse_to_merge_swapped_duplicates() {
        // r2 = Add(r1, r0); r3 = Add(r0, r1). Without canonicalize,
        // CSE wouldn't merge. With canonicalize, both become Add(r0, r1),
        // and CSE can drop one.
        let desc = empty_desc(
            vec![
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
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![1, 0],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(4)],
        );
        let canon = canonicalize(&desc);
        // Both Adds now have operands [0, 1].
        assert_eq!(canon.body.ops[2].operands, vec![0, 1]);
        assert_eq!(canon.body.ops[3].operands, vec![0, 1]);
        // Now CSE can collapse them.
        let after_cse = crate::rewrites::descriptor_cse(&canon);
        let add_count = after_cse
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::BinOpKind(BinOp::Add)))
            .count();
        assert_eq!(add_count, 1);
    }

    #[test]
    fn min_and_max_canonicalize() {
        for op in [BinOp::Min, BinOp::Max] {
            let desc = empty_desc(
                vec![
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
                        operands: vec![1, 0],
                        result: Some(2),
                    },
                ],
                vec![LiteralValue::U32(3), LiteralValue::U32(4)],
            );
            let out = canonicalize(&desc);
            assert_eq!(
                out.body.ops[2].operands,
                vec![0, 1],
                "{op:?} should canonicalize"
            );
        }
    }

    #[test]
    fn shl_shr_lt_gt_not_canonicalized() {
        for op in [BinOp::Shl, BinOp::Shr, BinOp::Lt, BinOp::Gt] {
            let desc = empty_desc(
                vec![
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
                        operands: vec![1, 0],
                        result: Some(2),
                    },
                ],
                vec![LiteralValue::U32(3), LiteralValue::U32(4)],
            );
            let out = canonicalize(&desc);
            assert_eq!(
                out.body.ops[2].operands,
                vec![1, 0],
                "{op:?} must NOT be commuted"
            );
        }
    }

    #[test]
    fn idempotent() {
        let desc = empty_desc(
            vec![
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
                    operands: vec![1, 0],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(4)],
        );
        let once = canonicalize(&desc);
        let twice = canonicalize(&once);
        assert_eq!(once.body.ops, twice.body.ops);
    }

    #[test]
    fn empty_kernel_is_noop() {
        let desc = empty_desc(vec![], vec![]);
        let out = canonicalize(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn child_bodies_recurse() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![KernelBody {
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
                            kind: KernelOpKind::BinOpKind(BinOp::Add),
                            operands: vec![1, 0],
                            result: Some(2),
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(3), LiteralValue::U32(4)],
                }],
                literals: vec![],
            },
        };
        let out = canonicalize(&desc);
        assert_eq!(out.body.child_bodies[0].ops[2].operands, vec![0, 1]);
    }
}
