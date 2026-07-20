//! Loop unrolling for small constant-bound loops.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A item A29
//! (loop strip-mining family). This is the unconditional-unroll
//! special case: when both bounds are compile-time constants AND the
//! iteration count is small (≤ 4 by default), inline N copies of the
//! body in sequence and strip the loop.
//!
//! Rules:
//! - Both `lo` and `hi` operands must point at `Literal(U32)` ops.
//! - `hi - lo` must be ≤ `MAX_UNROLL_COUNT` (default 4).
//! - Nested child bodies are duplicated, remapped into the parent
//!   `child_bodies` table, and result ids are freshened across the
//!   whole duplicated subtree.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::operand_semantics::operand_is_result_reference;
use crate::{KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue};

pub const MAX_UNROLL_COUNT: u32 = 4;

#[must_use]
pub fn loop_unroll(desc: &KernelDescriptor) -> KernelDescriptor {
    let mut out = desc.clone();
    out.body = unroll_body(&out.body);
    out
}

fn unroll_body(body: &KernelBody) -> KernelBody {
    // Map result-id → constant U32 value (for ops whose op is Literal(U32)).
    let lit_u32: FxHashMap<u32, u32> = body
        .ops
        .iter()
        .filter_map(|op| match (&op.kind, op.result, op.operands.first()) {
            (KernelOpKind::Literal, Some(r), Some(pool_idx)) => {
                match body.literals.get(*pool_idx as usize) {
                    Some(LiteralValue::U32(v)) => Some((r, *v)),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();

    // Compute the next free result-id (highest + 1) by walking the
    // ENTIRE subtree rooted at this body. This must match the depth of
    // collect_result_renames, which recurses into all child_bodies at
    // all levels. A shallow scan (only body.ops + one level of
    // child.ops) misses result-ids in grandchildren and deeper, so the
    // first renumber_body call could assign a new id that collides with
    // an id already present in a grandchild — silently producing
    // duplicate result-ids in the unrolled output.
    let mut next_id: u32 = max_result_id_in_subtree(body).map(|m| m + 1).unwrap_or(0);

    let mut new_ops: Vec<KernelOp> = Vec::with_capacity(body.ops.len());
    let mut new_children = body.child_bodies.clone();
    // Literal-pool fix: when we inline ops from `child` into `body`,
    // every `Literal` op's first operand is a pool index into the
    // CHILD's literal pool. We merge the child's literals into the
    // parent's pool and rewrite the inlined ops' pool indices on the
    // way through. Without this the inlined `Literal` op points at a
    // pool slot that doesn't exist in the parent (LiteralPoolOutOfRange
    // at verify time). Surfaced on `c11_build_vast_nodes` with nt=1.
    let mut new_literals: Vec<LiteralValue> = body.literals.clone();

    for op in &body.ops {
        if let KernelOpKind::StructuredForLoop { .. } = &op.kind {
            if op.operands.len() != 3 {
                new_ops.push(op.clone());
                continue;
            }
            let lo_id = op.operands[0];
            let hi_id = op.operands[1];
            let body_idx = op.operands[2] as usize;
            let lo = lit_u32.get(&lo_id).copied();
            let hi = lit_u32.get(&hi_id).copied();
            let child = body.child_bodies.get(body_idx).cloned();
            let unroll_target = match (lo, hi, child) {
                (Some(lo), Some(hi), Some(c)) => {
                    let count = hi.saturating_sub(lo);
                    if count <= MAX_UNROLL_COUNT
                        && safe_to_unroll(&c)
                        && literal_pool_refs_valid(&c)
                    {
                        Some((lo, hi, c))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some((lo, hi, child)) = unroll_target {
                // Match the `unrollable` check exactly (saturating_sub
                // also returns 0 when hi < lo). A plain `hi - lo` here
                // would underflow in release mode and produce a near-4B
                // iteration count  -  OOM-killed the fuzz harness on the
                // first generator run that hit this shape.
                let count = hi.saturating_sub(lo);
                // Source-loop variable name carried by the
                // StructuredForLoop op; needed so we replace
                // `LoopIndex { loop_var }` ops in the inlined body
                // with the iteration's literal value rather than
                // copying them verbatim. Without the substitution,
                // every unrolled iteration leaves a stray `LoopIndex`
                // op in the parent body whose loop wrapper no longer
                // exists  -  emit-naga rejects it as
                // `loop index `<var>` was emitted outside its
                // StructuredForLoop`. Surfaced on
                // `c11_build_vast_nodes` with nt=3 where the
                // 3-iteration `cleanup_i` loop is fully unrolled.
                let unroll_loop_var = match &op.kind {
                    KernelOpKind::StructuredForLoop { loop_var } => Some(loop_var.clone()),
                    _ => None,
                };
                // Build a per-iteration literal-pool map: child's
                // pool index → parent's pool index after merging.
                // Two child Literal ops referencing the same child
                // pool slot share a parent pool slot (de-duplicated).
                for iter in 0..count {
                    let iter_value = lo.wrapping_add(iter);
                    let (renumbered, new_next) = renumber_body(&child, next_id);
                    next_id = new_next;
                    let child_offset = new_children.len() as u32;
                    new_children.extend(renumbered.child_bodies);
                    let mut pool_map: FxHashMap<u32, u32> = FxHashMap::default();
                    let child_literals = child.literals.clone();
                    // Allocate one parent literal-pool slot per
                    // unrolled iteration to hold this iteration's
                    // index value, used to substitute LoopIndex
                    // reads with literal loads.
                    let iter_lit_pool_idx = new_literals.len() as u32;
                    new_literals.push(LiteralValue::U32(iter_value));
                    let unroll_loop_var = unroll_loop_var.clone();
                    new_ops.extend(renumbered.ops.into_iter().map(|mut op| {
                        remap_top_level_child_body_operands(&mut op, child_offset);
                        if matches!(op.kind, KernelOpKind::Literal) {
                            if let Some(child_idx) = op.operands.first().copied() {
                                let parent_idx = *pool_map.entry(child_idx).or_insert_with(|| {
                                    let next_pool = new_literals.len() as u32;
                                    if let Some(value) =
                                        child_literals.get(child_idx as usize).cloned()
                                    {
                                        new_literals.push(value);
                                        next_pool
                                    } else {
                                        // Unreachable when `literal_pool_refs_valid`
                                        // admits the child; preserve the operand
                                        // if another mutator violates the invariant
                                        // between admission and rewrite.
                                        child_idx
                                    }
                                });
                                if !op.operands.is_empty() {
                                    op.operands[0] = parent_idx;
                                }
                            }
                        } else if let KernelOpKind::LoopIndex { loop_var } = &op.kind {
                            if unroll_loop_var.as_ref().is_some_and(|v| v == loop_var) {
                                // Replace this iteration's LoopIndex
                                // op with a Literal op carrying the
                                // current iteration value. Result id
                                // and operand-ref shape are preserved
                                // so downstream ops referencing the
                                // LoopIndex's result keep working.
                                op.kind = KernelOpKind::Literal;
                                op.operands = vec![iter_lit_pool_idx];
                            }
                        }
                        op
                    }));
                }
                continue;
            }
        }
        new_ops.push(op.clone());
    }

    // Recursively unroll child bodies that weren't already inlined.
    let mut final_children: Vec<KernelBody> = Vec::with_capacity(new_children.len());
    for c in new_children.drain(..) {
        final_children.push(unroll_body(&c));
    }

    KernelBody {
        ops: new_ops,
        child_bodies: final_children,
        literals: new_literals,
    }
}

/// Walk the entire body subtree (ops + child_bodies recursively) and
/// return the maximum result-id found, or `None` if there are no
/// result-producing ops. Matches the traversal depth of
/// `collect_result_renames` so the `next_id` seed never misses an id
/// that `renumber_body` will encounter.
fn max_result_id_in_subtree(body: &KernelBody) -> Option<u32> {
    let mut max: Option<u32> = None;
    fn walk(b: &KernelBody, max: &mut Option<u32>) {
        for op in &b.ops {
            for r in op.result_ids() {
                *max = Some(max.map_or(r, |m: u32| m.max(r)));
            }
        }
        for child in &b.child_bodies {
            walk(child, max);
        }
    }
    walk(body, &mut max);
    max
}

fn literal_pool_refs_valid(body: &KernelBody) -> bool {
    body.ops.iter().all(|op| {
        if !matches!(op.kind, KernelOpKind::Literal) {
            return true;
        }
        op.operands
            .first()
            .is_some_and(|idx| (*idx as usize) < body.literals.len())
    }) && body.child_bodies.iter().all(literal_pool_refs_valid)
}

/// Unroll-safety check. Rejects (a) malformed child-body references and
/// (b) bodies that reference SSA ids defined in the body that contained
/// the loop. (b) is the scope-leak case: inlining the child into the
/// grandparent yanks the loop variable's outer-scope refs out of scope.
/// On `c11_build_vast_nodes` with nt=1 this fires constantly.
fn safe_to_unroll(child: &KernelBody) -> bool {
    let valid_child_refs = child.ops.iter().all(|op| {
        child_body_operands(&op.kind).all(|pos| {
            op.operands
                .get(pos)
                .is_some_and(|idx| (*idx as usize) < child.child_bodies.len())
        })
    }) && child.child_bodies.iter().all(safe_to_unroll);
    if !valid_child_refs {
        return false;
    }
    let mut produced = FxHashSet::default();
    collect_produced_ids_inclusive(child, &mut produced);
    body_refs_only(child, &produced)
}

fn collect_produced_ids_inclusive(body: &KernelBody, out: &mut FxHashSet<u32>) {
    for op in &body.ops {
        for r in op.result_ids() {
            out.insert(r);
        }
    }
    for c in &body.child_bodies {
        collect_produced_ids_inclusive(c, out);
    }
}

fn body_refs_only(body: &KernelBody, produced: &FxHashSet<u32>) -> bool {
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
    for c in &body.child_bodies {
        if !body_refs_only(c, produced) {
            return false;
        }
    }
    true
}

/// Renumber every result-id in `body` starting at `next_id`. Operand
/// references that match an old result-id are rewritten to the new id.
/// Returns the renumbered body + the next free id after the rename.
fn renumber_body(body: &KernelBody, mut next_id: u32) -> (KernelBody, u32) {
    let mut id_map = FxHashMap::<u32, u32>::default();
    collect_result_renames(body, &mut id_map, &mut next_id);
    (rewrite_body_with_renames(body, &id_map), next_id)
}

fn collect_result_renames(body: &KernelBody, id_map: &mut FxHashMap<u32, u32>, next_id: &mut u32) {
    for op in &body.ops {
        for result in op.result_ids() {
            id_map.insert(result, *next_id);
            *next_id += 1;
        }
    }
    for child in &body.child_bodies {
        collect_result_renames(child, id_map, next_id);
    }
}

fn rewrite_body_with_renames(body: &KernelBody, id_map: &FxHashMap<u32, u32>) -> KernelBody {
    let new_ops: Vec<KernelOp> = body
        .ops
        .iter()
        .map(|op| rewrite_op_with_renames(op, id_map))
        .collect();
    let child_bodies = body
        .child_bodies
        .iter()
        .map(|child| rewrite_body_with_renames(child, id_map))
        .collect();
    KernelBody {
        ops: new_ops,
        child_bodies,
        literals: body.literals.clone(),
    }
}

fn rewrite_op_with_renames(op: &KernelOp, id_map: &FxHashMap<u32, u32>) -> KernelOp {
    let operands: Vec<u32> = op
        .operands
        .iter()
        .enumerate()
        .map(|(pos, val)| {
            if operand_is_result_reference(&op.kind, pos) {
                *id_map.get(val).unwrap_or(val)
            } else {
                *val
            }
        })
        .collect();
    KernelOp {
        kind: op.kind.clone(),
        operands,
        result: op.result.map(|r| *id_map.get(&r).unwrap_or(&r)),
    }
}

fn remap_top_level_child_body_operands(op: &mut KernelOp, child_offset: u32) {
    for pos in child_body_operands(&op.kind) {
        if let Some(operand) = op.operands.get_mut(pos) {
            *operand = operand.saturating_add(child_offset);
        }
    }
}

fn child_body_operands(kind: &KernelOpKind) -> impl Iterator<Item = usize> + '_ {
    use KernelOpKind::*;
    let positions: &'static [usize] = match kind {
        StructuredIfThen => &[1],
        StructuredIfThenElse => &[1, 2],
        StructuredForLoop { .. } => &[2],
        StructuredBlock | Region { .. } => &[0],
        _ => &[],
    };
    positions.iter().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };
    use vyre_foundation::ir::BinOp;

    fn loop_with_body(
        lo: u32,
        hi: u32,
        body_ops: Vec<KernelOp>,
        body_lits: Vec<LiteralValue>,
    ) -> KernelDescriptor {
        KernelDescriptor {
            id: "loop".into(),
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
                    ops: body_ops,
                    child_bodies: vec![],
                    literals: body_lits,
                }],
                literals: vec![LiteralValue::U32(lo), LiteralValue::U32(hi)],
            },
        }
    }

    #[test]
    fn empty_kernel_unchanged() {
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
        let out = loop_unroll(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn loop_with_count_4_unrolled() {
        // for i in 0..4 { lit(99) }  →  4 inlined Literal ops
        let body_op = vec![KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(10),
        }];
        let body_lits = vec![LiteralValue::U32(99)];
        let desc = loop_with_body(0, 4, body_op, body_lits);
        let out = loop_unroll(&desc);
        // Outer ops: [Lit(0), Lit(4)] + 4 inlined Literal copies = 6 total.
        assert_eq!(out.body.ops.len(), 6);
        assert!(out
            .body
            .ops
            .iter()
            .all(|o| !matches!(o.kind, KernelOpKind::StructuredForLoop { .. })));
    }

    #[test]
    fn loop_with_count_above_threshold_not_unrolled() {
        // for i in 0..10 { ... }  →  loop kept (count > 4)
        let body_op = vec![KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(10),
        }];
        let body_lits = vec![LiteralValue::U32(99)];
        let desc = loop_with_body(0, 10, body_op, body_lits);
        let out = loop_unroll(&desc);
        assert_eq!(out.body.ops.len(), 3); // unchanged
        assert!(out
            .body
            .ops
            .iter()
            .any(|o| matches!(o.kind, KernelOpKind::StructuredForLoop { .. })));
    }

    #[test]
    fn loop_with_runtime_bounds_not_unrolled() {
        // for i in tid..hi { ... }  →  loop kept (lo not literal)
        let desc = KernelDescriptor {
            id: "runtime".into(),
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
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
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
                    ops: vec![],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(8)],
            },
        };
        let out = loop_unroll(&desc);
        assert!(out
            .body
            .ops
            .iter()
            .any(|o| matches!(o.kind, KernelOpKind::StructuredForLoop { .. })));
    }

    #[test]
    fn loop_with_zero_count_strips_loop() {
        // for i in 5..5 { ... }  →  empty
        let body_op = vec![KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(10),
        }];
        let body_lits = vec![LiteralValue::U32(99)];
        let desc = loop_with_body(5, 5, body_op, body_lits);
        let out = loop_unroll(&desc);
        // Outer: [Lit(5), Lit(5)] only  -  no inlined body, no loop op.
        assert_eq!(out.body.ops.len(), 2);
        assert!(out
            .body
            .ops
            .iter()
            .all(|o| !matches!(o.kind, KernelOpKind::StructuredForLoop { .. })));
    }

    #[test]
    fn loop_with_count_1_inlines_once() {
        let body_op = vec![KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(10),
        }];
        let body_lits = vec![LiteralValue::U32(99)];
        let desc = loop_with_body(0, 1, body_op, body_lits);
        let out = loop_unroll(&desc);
        // Outer: [Lit(0), Lit(1), inlined Literal] = 3 ops
        assert_eq!(out.body.ops.len(), 3);
    }

    #[test]
    fn loop_with_nested_if_is_unrolled_and_child_indices_are_remapped() {
        let desc = KernelDescriptor {
            id: "nested_if".into(),
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
                    ops: vec![
                        KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![0],
                            result: Some(10),
                        },
                        KernelOp {
                            kind: KernelOpKind::StructuredIfThen,
                            operands: vec![10, 0],
                            result: None,
                        },
                    ],
                    child_bodies: vec![KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![0],
                            result: Some(20),
                        }],
                        child_bodies: vec![],
                        literals: vec![LiteralValue::U32(9)],
                    }],
                    literals: vec![LiteralValue::Bool(true)],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(2)],
            },
        };
        let out = loop_unroll(&desc);
        assert!(out
            .body
            .ops
            .iter()
            .all(|o| !matches!(o.kind, KernelOpKind::StructuredForLoop { .. })));
        let if_indices: Vec<u32> = out
            .body
            .ops
            .iter()
            .filter(|op| matches!(op.kind, KernelOpKind::StructuredIfThen))
            .map(|op| op.operands[1])
            .collect();
        assert_eq!(if_indices.len(), 2);
        assert_ne!(if_indices[0], if_indices[1]);
        assert!(if_indices
            .iter()
            .all(|idx| (*idx as usize) < out.body.child_bodies.len()));
    }

    #[test]
    fn unrolled_body_renumbers_result_ids() {
        // for i in 0..3 { lit; binop }
        // Each iteration produces 2 fresh result-ids; pre-loop produced 0..1.
        let body_ops = vec![
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(10),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![10, 10],
                result: Some(11),
            },
        ];
        let body_lits = vec![LiteralValue::U32(7)];
        let desc = loop_with_body(0, 3, body_ops, body_lits);
        let out = loop_unroll(&desc);
        // 3 iterations × 2 ops each = 6 inlined ops; outer adds 2 → 8 total.
        assert_eq!(out.body.ops.len(), 8);
        // Collect all result-ids of the inlined ops; they should all be distinct.
        let inlined_ids: Vec<u32> = out.body.ops[2..].iter().filter_map(|o| o.result).collect();
        let mut sorted = inlined_ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            inlined_ids.len(),
            sorted.len(),
            "all unrolled result-ids must be distinct: {inlined_ids:?}"
        );
    }

    #[test]
    fn loop_unroll_is_idempotent() {
        let body_op = vec![KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(10),
        }];
        let body_lits = vec![LiteralValue::U32(99)];
        let desc = loop_with_body(0, 3, body_op, body_lits);
        let once = loop_unroll(&desc);
        let twice = loop_unroll(&once);
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
    }

    #[test]
    fn max_unroll_count_constant_is_documented() {
        assert_eq!(MAX_UNROLL_COUNT, 4);
    }

    #[test]
    fn loop_unroll_no_id_collision_with_deeply_nested_grandchild_ids() {
        // Regression for the shallow next_id seed bug.
        //
        // The loop body's direct ops use result-ids up to r10. Inside the
        // loop body there is a StructuredIfThen whose then-body (a
        // grandchild) contains r100. The shallow scan only saw the loop
        // body's direct ops (max=10), missing r100. On the first unroll
        // renumber_body assigned new ids starting at 11, eventually
        // assigning 100 to something — colliding with the grandchild's r100.
        //
        // The fixed scan uses max_result_id_in_subtree which walks all
        // descendants, so next_id starts above 100 and no collision occurs.
        let grandchild = KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(100),
            }],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(9)],
        };
        let loop_child = KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(10),
                },
                // StructuredIfThen: cond=r10 (pos 0), then_body=grandchild at index 0 (pos 1).
                KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![10, 0],
                    result: None,
                },
            ],
            child_bodies: vec![grandchild],
            literals: vec![LiteralValue::Bool(true)],
        };
        let desc = KernelDescriptor {
            id: "unroll_deep".into(),
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
                        // lo=r0 (=0), hi=r1 (=2), body_idx=0
                        operands: vec![0, 1, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![loop_child],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(2)],
            },
        };
        let out = loop_unroll(&desc);
        // Collect every result-id in the entire output descriptor tree.
        fn collect_ids(b: &KernelBody, ids: &mut Vec<u32>) {
            for op in &b.ops {
                if let Some(r) = op.result {
                    ids.push(r);
                }
            }
            for c in &b.child_bodies {
                collect_ids(c, ids);
            }
        }
        let mut ids: Vec<u32> = Vec::new();
        collect_ids(&out.body, &mut ids);
        let total = ids.len();
        let unique: std::collections::HashSet<u32> = ids.iter().cloned().collect();
        assert_eq!(
            total,
            unique.len(),
            "duplicate result-ids after unrolling; duplicates: {:?}",
            {
                let mut counts = std::collections::HashMap::<u32, usize>::new();
                for &id in &ids {
                    *counts.entry(id).or_insert(0) += 1;
                }
                counts
                    .into_iter()
                    .filter(|(_, c)| *c > 1)
                    .map(|(id, c)| format!("r{}×{}", id, c))
                    .collect::<Vec<_>>()
            }
        );
    }
}
