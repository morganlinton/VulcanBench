//! Drop unused literal-pool entries.
//!
//! Walks each `KernelBody`; collects every pool index referenced by a
//! `Literal` op (operand 0). Filters the body's `literals` Vec to keep
//! only the referenced entries, renumbers pool indices dense `0..N`,
//! rewrites every `Literal` op's pool-index operand to the new index.
//!
//! ## When does this fire?
//!
//! Surprisingly often. `descriptor_const_fold` synthesizes new pool entries every
//! time it folds a `BinOp(Lit, Lit)` (or `UnOp(Lit)` or `Cast(Lit)`)
//! and never reclaims the now-orphaned source entries. After a few
//! folding rounds, the pool can grow well beyond what's actually
//! referenced  -  the `optimize` example shows literals growing 4 → 9
//! across two `run_all_once` iterations.
//!
//! ## Per-body
//!
//! Each `KernelBody` has its own literal pool, so this pass operates
//! per-body. Recurses into child bodies.

use crate::{KernelBody, KernelDescriptor, KernelOpKind};

#[must_use]
pub fn drop_unused_literals(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = drop_unused_literals_body(out.body);
    out
}

fn drop_unused_literals_body(mut body: KernelBody) -> KernelBody {
    // Step 1: collect referenced pool indices.
    let mut referenced = vec![false; body.literals.len()];
    let mut referenced_count = 0usize;
    for op in &body.ops {
        if matches!(op.kind, KernelOpKind::Literal) {
            if let Some(&pool_idx) = op.operands.first() {
                let pool_idx = pool_idx as usize;
                if let Some(slot) = referenced.get_mut(pool_idx) {
                    if !*slot {
                        *slot = true;
                        referenced_count += 1;
                    }
                }
            }
        }
    }

    // Early bail when every entry is referenced.
    if referenced_count == body.literals.len() {
        // Still recurse into children.
        body.child_bodies = body
            .child_bodies
            .into_iter()
            .map(drop_unused_literals_body)
            .collect();
        return body;
    }

    // Step 2: build old_idx → new_idx map for surviving entries.
    let mut remap = vec![u32::MAX; body.literals.len()];
    let mut new_literals = Vec::with_capacity(referenced_count);
    let old_literals = std::mem::take(&mut body.literals);
    for (old_idx, lit) in old_literals.into_iter().enumerate() {
        if referenced[old_idx] {
            let new_idx = new_literals.len() as u32;
            remap[old_idx] = new_idx;
            new_literals.push(lit);
        }
    }

    // Step 3: rewrite every Literal op's pool-index operand.
    let old_ops = std::mem::take(&mut body.ops);
    body.ops = old_ops
        .into_iter()
        .map(|mut op| {
            if matches!(op.kind, KernelOpKind::Literal) {
                if let Some(&old) = op.operands.first() {
                    if let Some(&new) = remap.get(old as usize) {
                        op.operands[0] = new;
                        return op;
                    }
                }
            }
            op
        })
        .collect();

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(drop_unused_literals_body)
        .collect();

    body.literals = new_literals;
    body
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
    fn empty_kernel_no_op() {
        let desc = empty_desc(vec![], vec![]);
        let out = drop_unused_literals(&desc);
        assert!(out.body.literals.is_empty());
    }

    #[test]
    fn all_referenced_unchanged() {
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
            ],
            vec![LiteralValue::U32(7), LiteralValue::U32(99)],
        );
        let out = drop_unused_literals(&desc);
        assert_eq!(out.body.literals.len(), 2);
        assert_eq!(out.body.literals[0], LiteralValue::U32(7));
        assert_eq!(out.body.literals[1], LiteralValue::U32(99));
    }

    #[test]
    fn unused_entries_dropped_and_referenced_renumbered() {
        // Pool: [0=A, 1=B (unused), 2=C, 3=D (unused)]
        // Ops: Lit(pool=0), Lit(pool=2)
        // After: pool [A, C], ops Lit(0), Lit(1)
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(1),
                },
            ],
            vec![
                LiteralValue::U32(10), // referenced
                LiteralValue::U32(20), // unused
                LiteralValue::U32(30), // referenced
                LiteralValue::U32(40), // unused
            ],
        );
        let out = drop_unused_literals(&desc);
        assert_eq!(out.body.literals.len(), 2);
        assert_eq!(out.body.literals[0], LiteralValue::U32(10));
        assert_eq!(out.body.literals[1], LiteralValue::U32(30));
        // Operand 0 of first Lit op stays at 0.
        assert_eq!(out.body.ops[0].operands, vec![0]);
        // Operand 0 of second Lit op was 2, renumbered to 1.
        assert_eq!(out.body.ops[1].operands, vec![1]);
    }

    #[test]
    fn drops_when_no_literal_ops_reference_pool() {
        // No Literal ops at all; entire pool is unused.
        let desc = empty_desc(vec![], vec![LiteralValue::U32(1), LiteralValue::U32(2)]);
        let out = drop_unused_literals(&desc);
        assert!(out.body.literals.is_empty());
    }

    #[test]
    fn idempotent() {
        let desc = empty_desc(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![1],
                result: Some(0),
            }],
            vec![LiteralValue::U32(1), LiteralValue::U32(99)],
        );
        let once = drop_unused_literals(&desc);
        let twice = drop_unused_literals(&once);
        assert_eq!(once.body.literals, twice.body.literals);
        assert_eq!(once.body.ops, twice.body.ops);
    }

    #[test]
    fn child_body_pool_processed_independently() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(0),
                    }],
                    child_bodies: vec![],
                    literals: vec![
                        LiteralValue::U32(5), // unused in child
                        LiteralValue::U32(8), // referenced in child
                    ],
                }],
                literals: vec![],
            },
        };
        let out = drop_unused_literals(&desc);
        let child = &out.body.child_bodies[0];
        assert_eq!(child.literals.len(), 1);
        assert_eq!(child.literals[0], LiteralValue::U32(8));
        assert_eq!(child.ops[0].operands, vec![0]); // renumbered from 1
    }

    #[test]
    fn out_of_range_pool_index_treated_as_unreferenced() {
        // A Literal op pointing at a non-existent pool index  -  verify
        // catches this; here we just confirm we don't crash.
        let desc = empty_desc(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![99],
                result: Some(0),
            }],
            vec![LiteralValue::U32(7)],
        );
        let out = drop_unused_literals(&desc);
        // The orphan Literal op stays (we don't validate); pool gets
        // dropped because the only referenced index (99) is out of
        // range, so nothing in 0..1 is "referenced".
        assert!(out.body.literals.is_empty());
    }
}
