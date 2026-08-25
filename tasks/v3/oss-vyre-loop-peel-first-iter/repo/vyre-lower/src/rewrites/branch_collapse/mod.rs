//! Branch collapse  -  `StructuredIfThen`/`StructuredIfThenElse` whose
//! condition is provably constant collapses to the appropriate arm.
//!
//! This pass picks up two sources of provably-constant conditions:
//! 1. Direct `Literal(Bool(_))` ops, the easy case. `descriptor_const_fold`
//!    folds boolean arithmetic chains into these before this pass runs.
//! 2. Comparison `BinOp`s (`Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge`) whose operand
//!    ranges are statically derivable via `analyses::value_range`. This
//!    closes PERF A16 (range → branch elision): an `Lt(x, n)` where x's
//!    proven range is `[0, n-1]` collapses to true at this layer instead
//!    of leaving a runtime compare-and-branch in the kernel.

use crate::analyses::value_range::{analyze_body, IntRange};
use crate::operand_semantics::operand_is_result_reference;
use crate::{KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue};
use rustc_hash::FxHashMap;
use vyre_foundation::ir::BinOp;

#[must_use]
pub fn branch_collapse(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = collapse_body(out.body);
    out
}

fn collapse_body(mut body: KernelBody) -> KernelBody {
    // Map result-id → bool literal value (only when the producing op
    // is a Literal of a Bool value).
    let mut bool_lits: FxHashMap<u32, bool> = body
        .ops
        .iter()
        .filter_map(|op| match (&op.kind, op.result, op.operands.first()) {
            (KernelOpKind::Literal, Some(r), Some(pool_idx)) => {
                match body.literals.get(*pool_idx as usize) {
                    Some(LiteralValue::Bool(v)) => Some((r, *v)),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();

    // Augment with comparisons whose value-range proof yields a constant
    // bool. This is the PERF A16 wire-up: range narrows from prior passes
    // (loop-bound clamps, BitAnd masks, LICM-hoisted Mul ops) flow into
    // structured branch decisions here without needing the comparison
    // operands themselves to be Literal at the IR layer.
    let ranges = analyze_body(&body);
    for op in &body.ops {
        let Some(rid) = op.result else {
            continue;
        };
        if bool_lits.contains_key(&rid) {
            continue;
        }
        let KernelOpKind::BinOpKind(bin_op) = &op.kind else {
            continue;
        };
        if op.operands.len() < 2 {
            continue;
        }
        let lhs = ranges.get(op.operands[0]);
        let rhs = ranges.get(op.operands[1]);
        let (Some(l), Some(r)) = (lhs, rhs) else {
            continue;
        };
        if let Some(verdict) = compare_ranges(*bin_op, l, r) {
            bool_lits.insert(rid, verdict);
        }
    }

    // Pre-compute every result id referenced by any op in this body
    // (including nested children). If a candidate-for-drop body
    // produces an id in this set, we MUST NOT drop the body  -  its
    // result is consumed elsewhere and dropping creates dangling refs.
    let parent_referenced_ids = collect_all_referenced_ids(&body);
    let parent_produced_ids = collect_top_level_produced_ids(&body);
    let original_children = std::mem::take(&mut body.child_bodies);
    let mut new_ops: Vec<KernelOp> = Vec::with_capacity(body.ops.len());
    let mut new_children = original_children.clone();
    let old_ops = std::mem::take(&mut body.ops);

    for op in old_ops {
        match &op.kind {
            KernelOpKind::StructuredIfThen => {
                let cond_id = op.operands.first().copied();
                let body_id = op.operands.get(1).copied();
                if let (Some(cond_id), Some(body_id)) = (cond_id, body_id) {
                    if let Some(cond_lit) = bool_lits.get(&cond_id).copied() {
                        if cond_lit {
                            if let Some(child) = original_children.get(body_id as usize) {
                                if can_collapse_safely(child, &parent_produced_ids) {
                                    inline_child_body(child, &mut new_ops, &mut new_children);
                                    continue;
                                }
                                // Fall through  -  leave the IfThen
                                // intact rather than yank refs out
                                // of scope.
                            }
                        } else {
                            // Drop the if op entirely IF the dropped
                            // body produces no id consumed elsewhere
                            // in the parent body.
                            if let Some(child) = original_children.get(body_id as usize) {
                                if dropping_body_is_safe(child, &parent_referenced_ids) {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        }
                    }
                }
            }
            KernelOpKind::StructuredIfThenElse => {
                let cond_id = op.operands.first().copied();
                let then_id = op.operands.get(1).copied();
                let else_id = op.operands.get(2).copied();
                if let (Some(cond_id), Some(then_id), Some(else_id)) = (cond_id, then_id, else_id) {
                    if let Some(cond_lit) = bool_lits.get(&cond_id).copied() {
                        let pick_id = if cond_lit { then_id } else { else_id };
                        let drop_id = if cond_lit { else_id } else { then_id };
                        let pick = original_children.get(pick_id as usize);
                        let drop = original_children.get(drop_id as usize);
                        if let Some(pick) = pick {
                            if can_collapse_safely(pick, &parent_produced_ids)
                                && drop
                                    .map(|d| dropping_body_is_safe(d, &parent_referenced_ids))
                                    .unwrap_or(true)
                            {
                                inline_child_body(pick, &mut new_ops, &mut new_children);
                                continue;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        // Pass through: also recursively collapse any nested
        // structured-control-flow children even if the outer op isn't
        // collapsable.
        match &op.kind {
            KernelOpKind::StructuredIfThen
            | KernelOpKind::StructuredIfThenElse
            | KernelOpKind::StructuredForLoop { .. }
            | KernelOpKind::StructuredBlock
            | KernelOpKind::Region { .. } => {
                for child_id in op.operands.iter() {
                    if let Some(child) = original_children.get(*child_id as usize) {
                        let recursed = collapse_body(child.clone());
                        new_children[*child_id as usize] = recursed;
                    }
                }
            }
            _ => {}
        }
        new_ops.push(op);
    }

    body.ops = new_ops;
    body.child_bodies = new_children;
    body
}

fn inline_child_body(child: &KernelBody, ops: &mut Vec<KernelOp>, children: &mut Vec<KernelBody>) {
    let inlined = collapse_body(child.clone());
    let child_base = children.len() as u32;
    children.extend(inlined.child_bodies);
    ops.extend(
        inlined
            .ops
            .into_iter()
            .map(|op| rebase_child_body_refs(op, child_base)),
    );
}

/// Conservative pre-collapse safety check used by the IfThen and
/// IfThenElse handlers.
///
/// Inlining a child body into its grandparent is only safe when every
/// SSA result-reference inside the child resolves to an id produced
/// inside the child itself. If the child body references ids defined
/// in the body that contained the if-then (i.e. ids the inlining
/// would yank out of scope), refuse to collapse  -  the IfThen stays
/// intact and the verifier stays clean. This is the fix for
/// `DanglingResultRef { ref_id: 13 }` on shunting_yard descriptors.
fn can_collapse_safely(child: &KernelBody, parent_produced: &rustc_hash::FxHashSet<u32>) -> bool {
    let mut produced = rustc_hash::FxHashSet::default();
    collect_produced_ids_inclusive(child, &mut produced);
    produced.is_disjoint(parent_produced) && body_refs_only(child, &produced)
}

/// Conservative drop safety: dropping the child body is safe only
/// when none of its produced result ids are referenced anywhere in
/// the parent body (which would dangle if the producing ops vanished).
fn dropping_body_is_safe(child: &KernelBody, parent_refs: &rustc_hash::FxHashSet<u32>) -> bool {
    let mut produced = rustc_hash::FxHashSet::default();
    collect_produced_ids_inclusive(child, &mut produced);
    produced.is_disjoint(parent_refs)
}

fn collect_all_referenced_ids(body: &KernelBody) -> rustc_hash::FxHashSet<u32> {
    let mut out = rustc_hash::FxHashSet::default();
    collect_refs(body, &mut out);
    out
}

fn collect_top_level_produced_ids(body: &KernelBody) -> rustc_hash::FxHashSet<u32> {
    body.ops.iter().filter_map(|op| op.result).collect()
}

fn collect_refs(body: &KernelBody, out: &mut rustc_hash::FxHashSet<u32>) {
    for op in &body.ops {
        for (pos, &operand) in op.operands.iter().enumerate() {
            if operand_is_result_reference(&op.kind, pos) {
                out.insert(operand);
            }
        }
    }
    for child in &body.child_bodies {
        collect_refs(child, out);
    }
}

fn collect_produced_ids_inclusive(body: &KernelBody, out: &mut rustc_hash::FxHashSet<u32>) {
    for op in &body.ops {
        if let Some(r) = op.result {
            out.insert(r);
        }
    }
    for child in &body.child_bodies {
        collect_produced_ids_inclusive(child, out);
    }
}

fn body_refs_only(body: &KernelBody, produced: &rustc_hash::FxHashSet<u32>) -> bool {
    for op in &body.ops {
        for (pos, &operand) in op.operands.iter().enumerate() {
            if !operand_is_result_reference(&op.kind, pos) {
                continue;
            }
            if !produced.contains(&operand) {
                return false;
            }
        }
    }
    for child in &body.child_bodies {
        if !body_refs_only(child, produced) {
            return false;
        }
    }
    true
}

/// Try to derive a constant verdict for `op(l, r)` purely from the
/// pre-computed ranges. Returns `Some(true)` if every value-pair in
/// `(l, r)` satisfies the comparison, `Some(false)` if no pair does,
/// and `None` when the ranges overlap on the comparison boundary.
fn compare_ranges(op: BinOp, l: IntRange, r: IntRange) -> Option<bool> {
    match op {
        BinOp::Eq => {
            if l.is_singleton() && r.is_singleton() {
                Some(l.min == r.min)
            } else if l.max < r.min || r.max < l.min {
                Some(false)
            } else {
                None
            }
        }
        BinOp::Ne => {
            if l.is_singleton() && r.is_singleton() {
                Some(l.min != r.min)
            } else if l.max < r.min || r.max < l.min {
                Some(true)
            } else {
                None
            }
        }
        BinOp::Lt => {
            if l.max < r.min {
                Some(true)
            } else if l.min >= r.max {
                Some(false)
            } else {
                None
            }
        }
        BinOp::Le => {
            if l.max <= r.min {
                Some(true)
            } else if l.min > r.max {
                Some(false)
            } else {
                None
            }
        }
        BinOp::Gt => {
            if l.min > r.max {
                Some(true)
            } else if l.max <= r.min {
                Some(false)
            } else {
                None
            }
        }
        BinOp::Ge => {
            if l.min >= r.max {
                Some(true)
            } else if l.max < r.min {
                Some(false)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn rebase_child_body_refs(mut op: KernelOp, child_base: u32) -> KernelOp {
    for (pos, operand) in op.operands.iter_mut().enumerate() {
        if operand_is_child_body_ref(&op.kind, pos) {
            *operand = operand.saturating_add(child_base);
        }
    }
    op
}

fn operand_is_child_body_ref(kind: &KernelOpKind, pos: usize) -> bool {
    match kind {
        KernelOpKind::StructuredIfThen => pos == 1,
        KernelOpKind::StructuredIfThenElse => pos == 1 || pos == 2,
        KernelOpKind::StructuredForLoop { .. } => pos == 2,
        KernelOpKind::StructuredBlock | KernelOpKind::Region { .. } => pos == 0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };

    fn empty_kernel() -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        }
    }

    #[test]
    fn empty_kernel_no_change() {
        let out = branch_collapse(&empty_kernel());
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn if_then_with_true_cond_inlines_body() {
        // Lit(true); if(cond=true) { Lit(7); }
        let desc = KernelDescriptor {
            id: "true_then".into(),
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
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![0, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    }],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(7)],
                }],
                literals: vec![LiteralValue::Bool(true)],
            },
        };
        let out = branch_collapse(&desc);
        // Expected: outer ops = [Lit(true), Lit(7)]  -  IfThen replaced by inlined body.
        assert_eq!(out.body.ops.len(), 2);
        assert!(matches!(out.body.ops[0].kind, KernelOpKind::Literal));
        assert!(matches!(out.body.ops[1].kind, KernelOpKind::Literal));
        assert!(out
            .body
            .ops
            .iter()
            .all(|o| !matches!(o.kind, KernelOpKind::StructuredIfThen)));
    }

    #[test]
    fn if_then_with_false_cond_drops_branch() {
        // Lit(false); if(cond=false) { Lit(7); }
        let desc = KernelDescriptor {
            id: "false_then".into(),
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
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![0, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    }],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(7)],
                }],
                literals: vec![LiteralValue::Bool(false)],
            },
        };
        let out = branch_collapse(&desc);
        // Expected: outer ops = [Lit(false)] only  -  IfThen dropped, body discarded.
        assert_eq!(out.body.ops.len(), 1);
        assert!(matches!(out.body.ops[0].kind, KernelOpKind::Literal));
    }

    #[test]
    fn if_then_with_non_literal_cond_unchanged() {
        // tid; if(cond=tid) { Lit(7); }
        let desc = KernelDescriptor {
            id: "runtime_cond".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![0, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    }],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(7)],
                }],
                literals: vec![],
            },
        };
        let out = branch_collapse(&desc);
        // No change.
        assert_eq!(out.body.ops.len(), 2);
        assert!(out
            .body
            .ops
            .iter()
            .any(|o| matches!(o.kind, KernelOpKind::StructuredIfThen)));
    }

    #[test]
    fn if_then_else_picks_then_arm_for_true() {
        let desc = KernelDescriptor {
            id: "true_pick".into(),
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
                        kind: KernelOpKind::StructuredIfThenElse,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![1],
                            result: Some(1),
                        }],
                        child_bodies: vec![],
                        literals: vec![LiteralValue::U32(99)],
                    },
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![2],
                            result: Some(2),
                        }],
                        child_bodies: vec![],
                        literals: vec![LiteralValue::U32(88)],
                    },
                ],
                literals: vec![LiteralValue::Bool(true)],
            },
        };
        let out = branch_collapse(&desc);
        // Then-arm is at child_bodies[0]; we should see its 1 op survive.
        assert_eq!(out.body.ops.len(), 2); // [Lit(true), inlined_then_op]
        assert!(out
            .body
            .ops
            .iter()
            .all(|o| !matches!(o.kind, KernelOpKind::StructuredIfThenElse)));
    }

    #[test]
    fn if_then_else_picks_else_arm_for_false() {
        let desc = KernelDescriptor {
            id: "false_pick".into(),
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
                        kind: KernelOpKind::StructuredIfThenElse,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![1],
                            result: Some(1),
                        }],
                        child_bodies: vec![],
                        literals: vec![LiteralValue::U32(99)],
                    },
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![2],
                            result: Some(2),
                        }],
                        child_bodies: vec![],
                        literals: vec![LiteralValue::U32(88)],
                    },
                ],
                literals: vec![LiteralValue::Bool(false)],
            },
        };
        let out = branch_collapse(&desc);
        // Else-arm at child_bodies[1]  -  its 1 op survives.
        assert_eq!(out.body.ops.len(), 2);
        assert!(out
            .body
            .ops
            .iter()
            .all(|o| !matches!(o.kind, KernelOpKind::StructuredIfThenElse)));
    }

    #[test]
    fn inlined_nested_control_flow_rebases_child_body_refs() {
        let desc = KernelDescriptor {
            id: "nested".into(),
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
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![0, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::StructuredBlock,
                        operands: vec![0],
                        result: None,
                    }],
                    child_bodies: vec![KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![1],
                            result: Some(1),
                        }],
                        child_bodies: vec![],
                        literals: vec![LiteralValue::U32(7)],
                    }],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::Bool(true)],
            },
        };

        let out = branch_collapse(&desc);
        let block = out
            .body
            .ops
            .iter()
            .find(|op| matches!(op.kind, KernelOpKind::StructuredBlock))
            .expect("Fix: nested block must survive inlined branch");
        let child_id = block.operands[0] as usize;
        assert!(
            out.body.child_bodies.get(child_id).is_some(),
            "inlined nested control-flow child id must point at a reparented child body"
        );
    }

    #[test]
    fn branch_collapse_is_idempotent() {
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
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![0, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    }],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(42)],
                }],
                literals: vec![LiteralValue::Bool(true)],
            },
        };
        let once = branch_collapse(&desc);
        let twice = branch_collapse(&once);
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
    }

    fn lit_kernel(
        ops: Vec<KernelOp>,
        child_ops: Vec<KernelOp>,
        lits: Vec<LiteralValue>,
    ) -> KernelDescriptor {
        KernelDescriptor {
            id: "range_collapse".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![KernelBody {
                    ops: child_ops,
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(123)],
                }],
                literals: lits,
            },
        }
    }

    #[test]
    fn range_proves_lt_true_collapses_then_branch() {
        // Lit(3); Lit(10); cond = Lt(Lit(3), Lit(10)) → true; if(cond) { Lit(123) }
        // Both operands are singletons, range proves 3 < 10 → cond true.
        // Outer body collapses to [Lit(3), Lit(10), Lt, Lit(123)]  -  IfThen replaced.
        let desc = lit_kernel(
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
                    kind: KernelOpKind::BinOpKind(BinOp::Lt),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![2, 0],
                    result: None,
                },
            ],
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(3),
            }],
            vec![LiteralValue::U32(3), LiteralValue::U32(10)],
        );
        let out = branch_collapse(&desc);
        assert!(
            out.body
                .ops
                .iter()
                .all(|o| !matches!(o.kind, KernelOpKind::StructuredIfThen)),
            "range proof should have eliminated the IfThen"
        );
        assert!(
            out.body
                .ops
                .iter()
                .any(|o| matches!(o.kind, KernelOpKind::Literal) && o.result == Some(3)),
            "child Lit(123) should have been inlined into the parent body"
        );
    }

    #[test]
    fn range_proves_eq_false_drops_then_branch() {
        // Lit(7); Lit(11); cond = Eq(Lit(7), Lit(11)) → false; if(cond) { Lit(123) }
        // Disjoint singletons; range proves 7 != 11 → cond false. IfThen drops.
        let desc = lit_kernel(
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
                    kind: KernelOpKind::BinOpKind(BinOp::Eq),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![2, 0],
                    result: None,
                },
            ],
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(3),
            }],
            vec![LiteralValue::U32(7), LiteralValue::U32(11)],
        );
        let out = branch_collapse(&desc);
        assert!(
            out.body
                .ops
                .iter()
                .all(|o| !matches!(o.kind, KernelOpKind::StructuredIfThen)),
            "range proof should have eliminated the IfThen"
        );
        assert!(
            out.body
                .ops
                .iter()
                .all(|o| !(matches!(o.kind, KernelOpKind::Literal) && o.result == Some(3))),
            "dropped branch's body Lit(123) must NOT appear in parent"
        );
    }

    #[test]
    fn range_overlapping_lt_leaves_branch_unchanged() {
        // Lit(3); Lit(BitAnd over Lit(3) → range [0, 3]); cond = Lt(Lit(3), BitAnd…)
        // The BitAnd produces range [0, 3], so Lt(3, [0,3]) is undecidable
        // (true at (3, ?) only when the right value is > 3  -  never). Wait:
        // Lt(3, x in [0,3]) requires x > 3 → always false → range proves false.
        // For an undecidable case, use Lt(BitAnd, BitAnd) which gives [0,3] vs [0,3].
        let desc = lit_kernel(
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
                // a = lit_x & lit_mask (range [0, mask])
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::BitAnd),
                    operands: vec![0, 1],
                    result: Some(2),
                },
                // b = lit_x & lit_mask (range [0, mask]); same shape
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::BitAnd),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Lt),
                    operands: vec![2, 3],
                    result: Some(4),
                },
                KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![4, 0],
                    result: None,
                },
            ],
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(5),
            }],
            vec![LiteralValue::U32(15), LiteralValue::U32(7)],
        );
        let out = branch_collapse(&desc);
        assert!(
            out.body
                .ops
                .iter()
                .any(|o| matches!(o.kind, KernelOpKind::StructuredIfThen)),
            "overlapping ranges must NOT collapse the branch  -  undecidable"
        );
    }

    #[test]
    fn range_proves_ge_true_collapses_if_then_else_then_arm() {
        // Lit(20); Lit(5); cond = Ge(Lit(20), Lit(5)) → true; if-then-else picks then.
        let desc = KernelDescriptor {
            id: "ge_then".into(),
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
                        kind: KernelOpKind::BinOpKind(BinOp::Ge),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThenElse,
                        operands: vec![2, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![0],

                            result: Some(100),
                        }],
                        child_bodies: vec![],
                        literals: vec![LiteralValue::U32(777)],
                    },
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![0],
                            result: Some(200),
                        }],
                        child_bodies: vec![],
                        literals: vec![LiteralValue::U32(888)],
                    },
                ],
                literals: vec![LiteralValue::U32(20), LiteralValue::U32(5)],
            },
        };
        let out = branch_collapse(&desc);
        assert!(
            out.body
                .ops
                .iter()
                .all(|o| !matches!(o.kind, KernelOpKind::StructuredIfThenElse)),
            "Ge(20, 5) is provably true → IfThenElse should be replaced by then-arm"
        );
        assert!(
            out.body.ops.iter().any(|o| o.result == Some(100)),
            "then-arm body op (result 100) must be inlined"
        );
        assert!(
            out.body.ops.iter().all(|o| o.result != Some(200)),
            "else-arm body op (result 200) must NOT appear after collapse"
        );
    }
}
