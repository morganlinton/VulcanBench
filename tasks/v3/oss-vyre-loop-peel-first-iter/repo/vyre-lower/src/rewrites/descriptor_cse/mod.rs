//! Common-subexpression elimination rewrite.
//!
//! Uses `analyses::common_subexpr` to find equivalence groups, picks
//! the canonical (lowest op-index) of each group, rewrites references
//! inside the same body to point at the canonical, then strips duplicate
//! producers whose result is not visible across a structured-body
//! boundary.

use crate::analyses::common_subexpr;
use crate::operand_semantics::operand_is_result_reference;
use crate::{KernelBody, KernelDescriptor, KernelOp};
use rustc_hash::{FxHashMap, FxHashSet};

#[must_use]
pub fn descriptor_cse(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = cse_body(out.body, &FxHashSet::default());
    out
}

fn cse_body(mut body: KernelBody, externally_protected: &FxHashSet<u32>) -> KernelBody {
    let report = common_subexpr::analyze_body_shallow(String::new(), &body);
    let protected = protected_result_ids(&body, externally_protected);

    // For each equivalence group, build a "duplicate result-id →
    // canonical result-id" map.
    let mut id_remap = FxHashMap::<u32, u32>::default();
    let mut duplicates_to_strip = FxHashSet::<usize>::default();
    for group in &report.groups {
        let canonical_idx = group.op_indices[0];
        // Only collapse groups whose ops are within this body's
        // top-level ops (the analysis returns op-indices spanning
        // child bodies via offset). Filter to top-level.
        if canonical_idx >= body.ops.len() {
            continue;
        }
        let canonical_result = match body.ops[canonical_idx].result {
            Some(r) => r,
            None => continue,
        };
        for dup_idx in group.op_indices.iter().skip(1) {
            if *dup_idx >= body.ops.len() {
                continue;
            }
            if let Some(dup_result) = body.ops[*dup_idx].result {
                if protected.contains(&dup_result) {
                    continue;
                }
                id_remap.insert(dup_result, canonical_result);
                duplicates_to_strip.insert(*dup_idx);
            }
        }
    }

    // Strip duplicates and rewrite operand refs.
    let child_external_refs: Vec<FxHashSet<u32>> = body
        .child_bodies
        .iter()
        .map(|child| external_refs_for_child(&body, child))
        .collect();
    let mut surviving: Vec<KernelOp> = Vec::with_capacity(body.ops.len());
    let old_ops = std::mem::take(&mut body.ops);
    for (idx, mut op) in old_ops.into_iter().enumerate() {
        if duplicates_to_strip.contains(&idx) {
            continue;
        }
        for pos in 0..op.operands.len() {
            if operand_is_result_reference(&op.kind, pos) {
                if let Some(canonical) = id_remap.get(&op.operands[pos]) {
                    op.operands[pos] = *canonical;
                }
            }
        }
        surviving.push(op);
    }

    body.child_bodies = body
        .child_bodies
        .into_iter()
        .zip(child_external_refs.iter())
        .map(|(child, refs)| cse_body(child, refs))
        .collect();

    body.ops = surviving;
    body
}

fn protected_result_ids(
    body: &KernelBody,
    externally_protected: &FxHashSet<u32>,
) -> FxHashSet<u32> {
    let produced: FxHashSet<u32> = body.ops.iter().flat_map(KernelOp::result_ids).collect();
    let mut protected: FxHashSet<u32> = externally_protected
        .intersection(&produced)
        .copied()
        .collect();
    for child in &body.child_bodies {
        for result_ref in collect_result_refs(child) {
            if produced.contains(&result_ref) {
                protected.insert(result_ref);
            }
        }
    }
    protected
}

fn external_refs_for_child(parent: &KernelBody, child: &KernelBody) -> FxHashSet<u32> {
    let child_results = collect_results(child);
    let mut refs = FxHashSet::default();
    for op in &parent.ops {
        for (pos, operand) in op.operands.iter().enumerate() {
            if operand_is_result_reference(&op.kind, pos) && child_results.contains(operand) {
                refs.insert(*operand);
            }
        }
    }
    refs
}

fn collect_results(body: &KernelBody) -> FxHashSet<u32> {
    let mut results = FxHashSet::default();
    for op in &body.ops {
        for result in op.result_ids() {
            results.insert(result);
        }
    }
    for child in &body.child_bodies {
        results.extend(collect_results(child));
    }
    results
}

fn collect_result_refs(body: &KernelBody) -> FxHashSet<u32> {
    let mut refs = FxHashSet::default();
    for op in &body.ops {
        for (pos, operand) in op.operands.iter().enumerate() {
            if operand_is_result_reference(&op.kind, pos) {
                refs.insert(*operand);
            }
        }
    }
    for child in &body.child_bodies {
        refs.extend(collect_result_refs(child));
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };
    use vyre_foundation::ir::{BinOp, DataType};

    #[test]
    fn cse_collapses_duplicate_literals() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::WriteOnly,
                    name: "out".into(),
                }],
            },
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
                        operands: vec![0],
                        result: Some(1),
                    }, // dup of r0
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1], // index uses r0; value uses r1 (dup)
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7)],
            },
        };
        let out = descriptor_cse(&desc);
        // Duplicate r1 should be stripped. 3 ops → 2 ops.
        assert_eq!(out.body.ops.len(), 2);
        // Store should now reference r0 in BOTH index and value positions
        // (both point at the canonical literal).
        assert_eq!(out.body.ops[1].kind, KernelOpKind::StoreGlobal);
        assert_eq!(out.body.ops[1].operands, vec![0, 0, 0]);
    }

    #[test]
    fn cse_collapses_duplicate_arithmetic() {
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
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(3),
                    }, // dup of op 2
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(3), LiteralValue::U32(4)],
            },
        };
        let out = descriptor_cse(&desc);
        // 4 ops → 3 ops (one Add stripped).
        assert_eq!(out.body.ops.len(), 3);
    }

    #[test]
    fn cse_on_empty_kernel_is_noop() {
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
        let out = descriptor_cse(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn cse_preserves_unique_ops() {
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
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(1),
                    LiteralValue::U32(2),
                    LiteralValue::U32(3),
                ],
            },
        };
        let out = descriptor_cse(&desc);
        assert_eq!(out.body.ops.len(), 3);
    }

    #[test]
    fn cse_is_idempotent() {
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
                        operands: vec![0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7)],
            },
        };
        let once = descriptor_cse(&desc);
        let twice = descriptor_cse(&once);
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
    }

    #[test]
    fn cse_preserves_parent_result_used_by_child_body() {
        let desc = KernelDescriptor {
            id: "cross_body_parent".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::WriteOnly,
                    name: "out".into(),
                }],
            },
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
                        operands: vec![0],
                        result: Some(9),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredBlock,
                        operands: vec![0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 9, 0],
                        result: None,
                    }],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(7)],
            },
        };

        assert_eq!(crate::verify::verify(&desc), Ok(()));
        let out = descriptor_cse(&desc);
        assert_eq!(crate::verify::verify(&out), Ok(()));
        assert!(
            out.body.ops.iter().any(|op| op.result == Some(9)),
            "result 9 is read by the child body and cannot be stripped"
        );
    }

    #[test]
    fn cse_preserves_child_result_used_by_parent_body() {
        let desc = KernelDescriptor {
            id: "cross_body_child".into(),
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
                        kind: KernelOpKind::StructuredBlock,
                        operands: vec![0],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(BinOp::Mul),
                        operands: vec![9, 0],
                        result: Some(10),
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![
                        KernelOp {
                            kind: KernelOpKind::BinOpKind(BinOp::Add),
                            operands: vec![0, 0],
                            result: Some(1),
                        },
                        KernelOp {
                            kind: KernelOpKind::BinOpKind(BinOp::Add),
                            operands: vec![0, 0],
                            result: Some(9),
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(7)],
            },
        };

        assert_eq!(crate::verify::verify(&desc), Ok(()));
        let out = descriptor_cse(&desc);
        assert_eq!(crate::verify::verify(&out), Ok(()));
        assert!(
            out.body.child_bodies[0]
                .ops
                .iter()
                .any(|op| op.result == Some(9)),
            "result 9 is read by the parent body and cannot be stripped"
        );
    }
}
