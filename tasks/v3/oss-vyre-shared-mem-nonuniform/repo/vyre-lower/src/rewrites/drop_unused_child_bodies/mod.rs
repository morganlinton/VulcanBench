//! Drop unused child bodies.
//!
//! Walks each `KernelBody`; collects every child-body index referenced
//! by ops whose operand classifier yields `ChildBodyIdx`. Filters
//! `child_bodies` to keep only the referenced entries, renumbers
//! indices dense `0..N`, rewrites every op's child-body-idx operand
//! to the new index.
//!
//! ## Why this matters
//!
//! After `branch_collapse` inlines a `StructuredIfThen{Else}` arm or
//! `loop_unroll` inlines a `StructuredForLoop` body, the child body
//! sits orphaned in `body.child_bodies`  -  no op references it. Without
//! this pass, every emitter still has to walk the orphaned subtree
//! during lowering (and load_forwarding/descriptor_dce/etc all recurse into it
//! every iteration of run_all_once). Stripping eliminates that work
//! AND keeps verify/debug output clean.
//!
//! ## Per-body
//!
//! Each `KernelBody` has its own child-body Vec. This pass operates
//! per-body, recursing first (so children's orphans are stripped
//! before we evaluate the parent).

use crate::{KernelBody, KernelDescriptor, KernelOpKind};

#[must_use]
pub fn drop_unused_child_bodies(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = drop_unused_child_bodies_body(out.body);
    out
}

fn drop_unused_child_bodies_body(mut body: KernelBody) -> KernelBody {
    // Recurse first so children's orphans are stripped first. This
    // doesn't change WHICH children of THIS body are referenced, just
    // their internal cleanliness.
    let recursed_children: Vec<KernelBody> = body
        .child_bodies
        .into_iter()
        .map(drop_unused_child_bodies_body)
        .collect();
    body.child_bodies = Vec::new();

    // Step 1: collect referenced child-body indices.
    let mut referenced = vec![false; recursed_children.len()];
    let mut referenced_count = 0usize;
    for op in &body.ops {
        for (pos, &val) in op.operands.iter().enumerate() {
            if is_child_body_idx(&op.kind, pos) {
                if let Some(slot) = referenced.get_mut(val as usize) {
                    if !*slot {
                        *slot = true;
                        referenced_count += 1;
                    }
                }
            }
        }
    }

    // Early bail when every child is referenced.
    if referenced_count == recursed_children.len() {
        body.child_bodies = recursed_children;
        return body;
    }

    // Step 2: build old_idx → new_idx map.
    let mut remap = vec![u32::MAX; recursed_children.len()];
    let mut new_children = Vec::with_capacity(referenced_count);
    for (old_idx, child) in recursed_children.into_iter().enumerate() {
        if referenced[old_idx] {
            let new_idx = new_children.len() as u32;
            remap[old_idx] = new_idx;
            new_children.push(child);
        }
    }

    // Step 3: rewrite child-body-idx operands.
    let old_ops = std::mem::take(&mut body.ops);
    body.ops = old_ops
        .into_iter()
        .map(|mut op| {
            for pos in 0..op.operands.len() {
                if is_child_body_idx(&op.kind, pos) {
                    let val = &mut op.operands[pos];
                    if let Some(&new) = remap.get(*val as usize) {
                        *val = new;
                    }
                }
            }
            op
        })
        .collect();

    body.child_bodies = new_children;
    body
}

/// Per-kind classifier  -  which positions carry a child-body index.
/// Matches the `ChildBodyIdx` arm in `vyre_lower::verify`.
fn is_child_body_idx(kind: &KernelOpKind, pos: usize) -> bool {
    use KernelOpKind::*;
    match kind {
        StructuredIfThen => pos == 1,
        StructuredIfThenElse => pos == 1 || pos == 2,
        StructuredForLoop { .. } => pos == 2,
        StructuredBlock | Region { .. } => pos == 0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };

    fn empty_desc(ops: Vec<KernelOp>, child_bodies: Vec<KernelBody>) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies,
                literals: vec![LiteralValue::U32(1)],
            },
        }
    }

    fn tiny_body() -> KernelBody {
        KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7)],
        }
    }

    #[test]
    fn no_children_no_op() {
        let desc = empty_desc(vec![], vec![]);
        let out = drop_unused_child_bodies(&desc);
        assert!(out.body.child_bodies.is_empty());
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
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 0], // body 0
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 1], // body 1
                    result: None,
                },
            ],
            vec![tiny_body(), tiny_body()],
        );
        let out = drop_unused_child_bodies(&desc);
        assert_eq!(out.body.child_bodies.len(), 2);
    }

    #[test]
    fn unused_child_body_dropped() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                // Only references child 1  -  child 0 is orphaned.
                KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 1],
                    result: None,
                },
            ],
            vec![tiny_body(), tiny_body()],
        );
        let out = drop_unused_child_bodies(&desc);
        assert_eq!(out.body.child_bodies.len(), 1);
        // The op's child-body operand was 1; should now be 0.
        assert_eq!(out.body.ops[1].operands, vec![0, 0]);
    }

    #[test]
    fn middle_child_dropped_with_renumber() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 0], // body 0  -  keep
                    result: None,
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 2], // body 2  -  keep
                    result: None,
                },
                // body 1 is unused
            ],
            vec![tiny_body(), tiny_body(), tiny_body()],
        );
        let out = drop_unused_child_bodies(&desc);
        assert_eq!(out.body.child_bodies.len(), 2);
        // First If's child-idx 0 stays 0.
        assert_eq!(out.body.ops[1].operands, vec![0, 0]);
        // Second If's child-idx 2 renumbered to 1.
        assert_eq!(out.body.ops[2].operands, vec![0, 1]);
    }

    #[test]
    fn structured_if_then_else_keeps_both_arms() {
        // IfThenElse has child-idx at both pos 1 (then) AND pos 2 (else).
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThenElse,
                    operands: vec![0, 0, 1], // then=child 0, else=child 1
                    result: None,
                },
            ],
            vec![tiny_body(), tiny_body()],
        );
        let out = drop_unused_child_bodies(&desc);
        // Both children referenced  -  neither dropped.
        assert_eq!(out.body.child_bodies.len(), 2);
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
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![0, 0],
                    result: None,
                },
            ],
            vec![tiny_body(), tiny_body()],
        );
        let once = drop_unused_child_bodies(&desc);
        let twice = drop_unused_child_bodies(&once);
        assert_eq!(once.body.child_bodies.len(), twice.body.child_bodies.len());
        assert_eq!(once.body.ops, twice.body.ops);
    }

    #[test]
    fn structured_for_loop_pos_2_recognized() {
        let desc = empty_desc(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: std::sync::Arc::from("i"),
                    },
                    operands: vec![0, 1, 1], // lo=r0, hi=r1, body=child 1
                    result: None,
                },
            ],
            vec![tiny_body(), tiny_body()],
        );
        let out = drop_unused_child_bodies(&desc);
        // child 0 unused  -  dropped; child 1 was the only ref → renumbered to 0.
        assert_eq!(out.body.child_bodies.len(), 1);
        assert_eq!(out.body.ops[2].operands, vec![0, 1, 0]);
    }
}
