//! Conservative loop fission for two independent disjoint stores.

use super::dataflow_facts::resolve_reaching_def_id as resolve;
use crate::{KernelBody, KernelDescriptor, KernelOp, KernelOpKind};

/// Split a loop containing exactly two independent disjoint writes into
/// two loops with the same bounds.
#[must_use]
pub fn loop_fission(desc: &KernelDescriptor) -> KernelDescriptor {
    loop_fission_with_optional_dataflow_facts(desc, None, None)
}

#[must_use]
pub fn loop_fission_with_alias_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
) -> KernelDescriptor {
    loop_fission_with_optional_dataflow_facts(desc, Some(alias_facts), None)
}

#[must_use]
pub fn loop_fission_with_weir_alias_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::weir_alias::AliasFactSet,
) -> KernelDescriptor {
    loop_fission_with_alias_facts(desc, alias_facts)
}

#[must_use]
pub fn loop_fission_with_dataflow_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
    reaching_defs: &crate::analyses::reaching_def_facts::ReachingDefFactSet,
) -> KernelDescriptor {
    loop_fission_with_optional_dataflow_facts(desc, Some(alias_facts), Some(reaching_defs))
}

#[must_use]
pub fn loop_fission_with_dataflow_analysis_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::weir_alias::AliasFactSet,
    reaching_defs: &crate::analyses::weir_reaching_def::ReachingDefFactSet,
) -> KernelDescriptor {
    loop_fission_with_dataflow_facts(desc, alias_facts, reaching_defs)
}

fn loop_fission_with_optional_dataflow_facts(
    desc: &KernelDescriptor,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = fission_body(out.body, alias_facts, reaching_defs);
    out
}

fn fission_body(
    mut body: KernelBody,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> KernelBody {
    body.child_bodies = body
        .child_bodies
        .into_iter()
        .map(|child| fission_body(child, alias_facts, reaching_defs))
        .collect();
    let old_ops = std::mem::take(&mut body.ops);
    let mut new_ops = Vec::with_capacity(old_ops.len());
    for op in old_ops {
        if let Some((left_loop, right_loop)) =
            try_fission_loop(&mut body, &op, alias_facts, reaching_defs)
        {
            new_ops.push(left_loop);
            new_ops.push(right_loop);
        } else {
            new_ops.push(op);
        }
    }
    body.ops = new_ops;
    body
}

fn try_fission_loop(
    body: &mut KernelBody,
    op: &KernelOp,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> Option<(KernelOp, KernelOp)> {
    let KernelOpKind::StructuredForLoop { .. } = &op.kind else {
        return None;
    };
    if op.operands.len() != 3 {
        return None;
    }
    let child = body.child_bodies.get(op.operands[2] as usize)?.clone();
    if !child.child_bodies.is_empty() || child.ops.len() != 2 {
        return None;
    }
    let left_write = write_target(&child.ops[0], reaching_defs)?;
    let right_write = write_target(&child.ops[1], reaching_defs)?;
    if !write_targets_are_independent(left_write, right_write, alias_facts) {
        return None;
    }
    let left_idx = body.child_bodies.len() as u32;
    body.child_bodies.push(KernelBody {
        ops: vec![child.ops[0].clone()],
        child_bodies: vec![],
        literals: child.literals.clone(),
    });
    let right_idx = body.child_bodies.len() as u32;
    body.child_bodies.push(KernelBody {
        ops: vec![child.ops[1].clone()],
        child_bodies: vec![],
        literals: child.literals,
    });
    let mut left = op.clone();
    left.operands[2] = left_idx;
    let mut right = op.clone();
    right.operands[2] = right_idx;
    Some((left, right))
}

fn write_target(
    op: &KernelOp,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> Option<(u8, u32, u32)> {
    match op.kind {
        KernelOpKind::StoreGlobal if op.operands.len() == 3 => {
            Some((0, op.operands[0], resolve(op.operands[1], reaching_defs)))
        }
        KernelOpKind::StoreShared if op.operands.len() == 3 => {
            Some((1, op.operands[0], resolve(op.operands[1], reaching_defs)))
        }
        _ => None,
    }
}

fn write_targets_are_independent(
    left: (u8, u32, u32),
    right: (u8, u32, u32),
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
) -> bool {
    left.0 != right.0
        || alias_facts.is_some_and(|facts| facts.proves_no_alias(left.1, left.2, right.1, right.2))
}

#[cfg(test)]
mod tests {
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };

    use super::*;

    #[test]
    fn splits_two_disjoint_store_loop() {
        let desc = KernelDescriptor {
            id: "fission".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
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
                        kind: KernelOpKind::StructuredForLoop {
                            loop_var: "i".into(),
                        },
                        operands: vec![0, 1, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![store(0), shared_store_with_index(0, 0)],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(4)],
            },
        };
        let out = loop_fission(&desc);
        let loops = out
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. }))
            .count();
        assert_eq!(loops, 2);
    }

    fn store(binding: u32) -> KernelOp {
        store_with_index(binding, 0)
    }

    #[test]
    fn does_not_split_different_global_bindings_without_external_no_alias_fact() {
        let desc = KernelDescriptor {
            id: "fission_cross_binding_conservative".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
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
                        kind: KernelOpKind::StructuredForLoop {
                            loop_var: "i".into(),
                        },
                        operands: vec![0, 1, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![store(0), store(1)],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(4)],
            },
        };
        let conservative = loop_fission(&desc);
        let conservative_loops = conservative
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. }))
            .count();
        assert_eq!(conservative_loops, 1);

        let mut facts = crate::analyses::alias_facts::AliasFactSet::default();
        facts.insert_no_alias(crate::analyses::alias_facts::NoAliasFact {
            left_binding: 0,
            left_index: 0,
            right_binding: 1,
            right_index: 0,
        });
        let alias_aware = loop_fission_with_alias_facts(&desc, &facts);
        let alias_aware_loops = alias_aware
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. }))
            .count();
        assert_eq!(alias_aware_loops, 2);
    }

    #[test]
    fn splits_same_binding_loop_when_analysis_proves_indices_no_alias() {
        let desc = KernelDescriptor {
            id: "fission_alias".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
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
                        kind: KernelOpKind::StructuredForLoop {
                            loop_var: "i".into(),
                        },
                        operands: vec![0, 1, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![store_with_index(0, 10), store_with_index(0, 11)],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(4)],
            },
        };
        let conservative = loop_fission(&desc);
        let conservative_loops = conservative
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. }))
            .count();
        assert_eq!(conservative_loops, 1);
        let mut facts = crate::analyses::alias_facts::AliasFactSet::default();
        facts.insert_no_alias(crate::analyses::alias_facts::NoAliasFact {
            left_binding: 0,
            left_index: 10,
            right_binding: 0,
            right_index: 11,
        });
        let alias_aware = loop_fission_with_alias_facts(&desc, &facts);
        let alias_aware_loops = alias_aware
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. }))
            .count();
        assert_eq!(alias_aware_loops, 2);
    }

    #[test]
    fn splits_same_slot_global_and_shared_stores_without_alias_facts() {
        let desc = KernelDescriptor {
            id: "fission_address_space".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
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
                        kind: KernelOpKind::StructuredForLoop {
                            loop_var: "i".into(),
                        },
                        operands: vec![0, 1, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![store_with_index(0, 10), shared_store_with_index(0, 10)],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(4)],
            },
        };
        let out = loop_fission(&desc);
        let loops = out
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StructuredForLoop { .. }))
            .count();
        assert_eq!(loops, 2);
    }

    fn store_with_index(binding: u32, index: u32) -> KernelOp {
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![binding, index, 0],
            result: None,
        }
    }

    fn shared_store_with_index(binding: u32, index: u32) -> KernelOp {
        KernelOp {
            kind: KernelOpKind::StoreShared,
            operands: vec![binding, index, 0],
            result: None,
        }
    }
}
