//! Loop-invariant code motion.
//!
//! Source-of-truth: `PERF_ROADMAP_2026-05-01.md` section A item A17.
//!
//! For each `StructuredForLoop` in the kernel body, walk the loop body
//! and identify ops whose operand chain doesn't depend on the loop
//! variable. Hoist them out of the loop body into the parent body
//! immediately before the loop op.
//!
//! Hoisted ops are assigned fresh parent-body ids, loop-body operands
//! are rewritten to those fresh ids, and child literal-pool references
//! are merged into the parent literal pool. That keeps the per-body id
//! invariant intact while still removing pure loop-invariant work from
//! hot loop bodies.

use rustc_hash::FxHashSet;
use std::collections::BTreeMap;

use super::dataflow_facts::resolve_reaching_def_id as resolve;
use super::memory_address::{
    locations_may_alias, AddressKey, MemoryLocation, MemoryTarget, SlotAliasPolicy,
};
use crate::operand_semantics::operand_is_result_reference;
use crate::{BindingVisibility, KernelBody, KernelDescriptor, KernelOp, KernelOpKind};

/// Returns an iterator over the operand positions that carry child-body
/// indices for `kind`. This matches the authoritative classification in
/// `verify::classify_operand` (OperandClass::ChildBodyIdx).
fn child_body_index_positions(kind: &KernelOpKind) -> &'static [usize] {
    match kind {
        KernelOpKind::StructuredIfThen => &[1],
        KernelOpKind::StructuredIfThenElse => &[1, 2],
        KernelOpKind::StructuredForLoop { .. } => &[2],
        KernelOpKind::StructuredBlock | KernelOpKind::Region { .. } => &[0],
        _ => &[],
    }
}

/// LICM with a correct cross-body id rewrite.
///
/// For each loop, invariant ops are moved to the parent body before
/// the loop op. Hoisted ops receive fresh result ids that do not
/// collide with any parent-body id. Operand references inside the
/// remaining loop body are rewritten to use the new ids. Literal
/// pool entries referenced by hoisted `Literal` ops are merged into
/// the parent body's literal pool.
#[must_use]
pub fn licm(desc: &KernelDescriptor) -> KernelDescriptor {
    licm_with_optional_dataflow_facts(desc, None, None)
}

#[must_use]
pub fn licm_with_alias_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
) -> KernelDescriptor {
    licm_with_optional_dataflow_facts(desc, Some(alias_facts), None)
}

#[must_use]
pub fn licm_with_weir_alias_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::weir_alias::AliasFactSet,
) -> KernelDescriptor {
    licm_with_alias_facts(desc, alias_facts)
}

#[must_use]
pub fn licm_with_dataflow_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
    reaching_defs: &crate::analyses::reaching_def_facts::ReachingDefFactSet,
) -> KernelDescriptor {
    licm_with_optional_dataflow_facts(desc, Some(alias_facts), Some(reaching_defs))
}

#[must_use]
pub fn licm_with_dataflow_analysis_facts(
    desc: &KernelDescriptor,
    alias_facts: &crate::analyses::weir_alias::AliasFactSet,
    reaching_defs: &crate::analyses::weir_reaching_def::ReachingDefFactSet,
) -> KernelDescriptor {
    licm_with_dataflow_facts(desc, alias_facts, reaching_defs)
}

fn licm_with_optional_dataflow_facts(
    desc: &KernelDescriptor,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> KernelDescriptor {
    let mut out = desc.clone();
    let read_only_bindings = desc
        .bindings
        .slots
        .iter()
        .filter(|slot| matches!(slot.visibility, BindingVisibility::ReadOnly))
        .map(|slot| slot.slot)
        .collect::<FxHashSet<_>>();
    // Compute the global next-free-id ONCE from the entire descriptor
    // and thread it through every recursive `licm_body` call. Without
    // this, each recursive call computed its own `next_free_id` from
    // its local body's tree, and two sibling recursions (e.g. inner
    // loops in different branches of an outer if) would independently
    // pick the SAME fresh id for their hoisted ops, producing
    // duplicate-result-id collisions that the cleanup pipeline carried
    // forward into emit. Discovered via off-by-one in
    // `loop_carry_smoke::region_body_let_bind_with_inner_loop_increment_then_store`
    // where a hoisted U32(1) literal in the outer-if body collided with
    // a Bool(true) literal hoisted in a nested if's body.
    let mut next_free_id = max_result_id(&out.body)
        .map(|m| m.wrapping_add(1))
        .unwrap_or(0);
    out.body = licm_body(
        &out.body,
        &mut next_free_id,
        &read_only_bindings,
        alias_facts,
        reaching_defs,
    );
    out
}

fn max_result_id(body: &KernelBody) -> Option<u32> {
    let mut max: Option<u32> = None;
    fn walk(b: &KernelBody, max: &mut Option<u32>) {
        for op in &b.ops {
            for r in op.result_ids() {
                *max = Some(max.map_or(r, |m| m.max(r)));
            }
        }
        for child in &b.child_bodies {
            walk(child, max);
        }
    }
    walk(body, &mut max);
    max
}

fn licm_body(
    body: &KernelBody,
    next_free_id: &mut u32,
    read_only_bindings: &FxHashSet<u32>,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> KernelBody {
    let mut new_ops = Vec::with_capacity(body.ops.len());
    let mut new_children = body.child_bodies.clone();
    let mut new_literals = body.literals.clone();
    // Soundness: when a hoisted op produces a result, ALL downstream
    // operands in the SAME parent body that referenced the original
    // (loop-body) id need to point at the new parent-body id. Without
    // this, a `StoreGlobal` (or any op) appearing after the loop in
    // the parent body keeps referencing the dead loop-body id and
    // descriptor verify rejects with `DanglingResultRef`.
    //
    // Note that `remap_body_ids` already rewrites the LOOP body and
    // any ops we push AFTER the loop op need this same treatment in
    // the parent  -  that's what `parent_id_map` captures.
    let mut parent_id_map = BTreeMap::<u32, u32>::new();
    // Track child-body indices that have already been recursed in the
    // per-op phase, so the final sweep skips them and does not apply
    // licm_body a second time to the same child.
    let mut already_recursed: FxHashSet<usize> = FxHashSet::default();

    for op in &body.ops {
        if let KernelOpKind::StructuredForLoop { .. } = &op.kind {
            // Find the body child id (operand 2 per descriptor contract).
            if let Some(body_idx) = op.operands.get(2).copied() {
                if let Some(child) = body.child_bodies.get(body_idx as usize).cloned() {
                    // Hoist invariant ops from the child body.
                    let (hoisted, remaining) =
                        hoist_invariants(&child, read_only_bindings, alias_facts, reaching_defs);
                    if !hoisted.is_empty() {
                        // 1. Allocate fresh parent-body ids for every
                        //    hoisted result so we never collide.
                        let mut id_map = BTreeMap::<u32, u32>::new();
                        for h_op in &hoisted {
                            if let Some(r) = h_op.result {
                                id_map.insert(r, *next_free_id);
                                *next_free_id = next_free_id.wrapping_add(1);
                            }
                        }

                        // 2. Merge literals referenced by hoisted ops
                        //    into the parent literal pool and build a
                        //    child-index → parent-index map.
                        let mut lit_map = BTreeMap::<u32, u32>::new();
                        let mut renumbered_hoisted = Vec::with_capacity(hoisted.len());
                        for mut h_op in hoisted {
                            if matches!(h_op.kind, KernelOpKind::Literal) {
                                if let Some(&old_idx) = h_op.operands.first() {
                                    // Soundness: skip the rewrite when
                                    // the source literal is missing.
                                    // Previously the captured `idx` was
                                    // committed to `lit_map` even when
                                    // the conditional `push` was a no-op,
                                    // so the rewritten Literal op pointed
                                    // at a pool slot that was never
                                    // populated → `LiteralPoolOutOfRange`
                                    // at descriptor verify time.
                                    if let Some(val) = child.literals.get(old_idx as usize) {
                                        let new_idx =
                                            *lit_map.entry(old_idx).or_insert_with(|| {
                                                let idx = new_literals.len() as u32;
                                                new_literals.push(val.clone());
                                                idx
                                            });
                                        h_op.operands[0] = new_idx;
                                    }
                                    // When the source literal is missing
                                    // we leave the operand alone  -  the
                                    // op was already invalid in the
                                    // source body; LICM does not have to
                                    // fabricate a slot for it.
                                }
                            }
                            // Rewrite result-id refs inside the hoisted op.
                            h_op.operands = h_op
                                .operands
                                .iter()
                                .enumerate()
                                .map(|(pos, val)| {
                                    if operand_is_result_reference(&h_op.kind, pos) {
                                        *id_map.get(val).unwrap_or(val)
                                    } else {
                                        *val
                                    }
                                })
                                .collect();
                            h_op.result = h_op.result.map(|r| *id_map.get(&r).unwrap_or(&r));
                            renumbered_hoisted.push(h_op);
                        }

                        // 3. Recurse into the remaining body, then remap
                        //    any references to the old hoisted ids so the
                        //    child body still points at the parent results.
                        let recursed = licm_body(
                            &remaining,
                            next_free_id,
                            read_only_bindings,
                            alias_facts,
                            reaching_defs,
                        );
                        let remapped = remap_body_ids(&recursed, &id_map);

                        new_children[body_idx as usize] = remapped;
                        already_recursed.insert(body_idx as usize);
                        new_ops.extend(renumbered_hoisted);
                        new_ops.push(op.clone());
                        // Carry the id remap forward so any parent-body
                        // op AFTER this loop that referenced the old
                        // (now-hoisted) child id picks up the new
                        // parent-body result id. Without this, the
                        // post-loop reader (e.g. a `StoreGlobal`
                        // consuming the loop accumulator) would point
                        // at a result id that no longer exists in the
                        // descriptor → `DanglingResultRef` at verify.
                        for (&old, &new) in id_map.iter() {
                            parent_id_map.insert(old, new);
                        }
                        continue;
                    }
                    // No invariants to hoist  -  just recurse into child.
                    let recursed = licm_body(
                        &remaining,
                        next_free_id,
                        read_only_bindings,
                        alias_facts,
                        reaching_defs,
                    );
                    new_children[body_idx as usize] = recursed;
                    already_recursed.insert(body_idx as usize);
                    new_ops.push(op.clone());
                    continue;
                }
            }
        }
        // Recurse into other structured-control-flow children too.
        // CORRECTNESS: use child_body_index_positions() to find which
        // operand positions carry child-body indices. Iterating ALL
        // operands was wrong for StructuredIfThen/IfThenElse: operand[0]
        // is the condition result-id, NOT a child-body index. Treating it
        // as one would corrupt an unrelated child body if the condition
        // result-id happened to be a valid child-body index.
        for &pos in child_body_index_positions(&op.kind) {
            if let Some(&child_id) = op.operands.get(pos) {
                if let Some(child) = body.child_bodies.get(child_id as usize) {
                    let recursed = licm_body(
                        child,
                        next_free_id,
                        read_only_bindings,
                        alias_facts,
                        reaching_defs,
                    );
                    new_children[child_id as usize] = recursed;
                    already_recursed.insert(child_id as usize);
                }
            }
        }
        // Apply the accumulated parent_id_map to this op's
        // result-reference operands before pushing it. This rewrites
        // post-loop readers to point at the new parent-body result
        // ids that LICM created when it hoisted invariants out of an
        // earlier loop in this same parent body.
        let mut rewritten = op.clone();
        if !parent_id_map.is_empty() {
            for (pos, val) in rewritten.operands.iter_mut().enumerate() {
                if operand_is_result_reference(&op.kind, pos) {
                    if let Some(&new) = parent_id_map.get(val) {
                        *val = new;
                    }
                }
            }
        }
        new_ops.push(rewritten);
    }

    // Recurse into all children NOT already processed in the per-op
    // phase above. Children that were already processed (StructuredForLoop
    // bodies and StructuredIfThen/IfThenElse/Block/Region children) must
    // NOT be recursed again or LICM is applied twice to the same subtree,
    // causing spurious transformations and potential id collisions.
    let final_children: Vec<KernelBody> = new_children
        .into_iter()
        .enumerate()
        .map(|(idx, c)| {
            if already_recursed.contains(&idx) {
                c
            } else {
                licm_body(
                    &c,
                    next_free_id,
                    read_only_bindings,
                    alias_facts,
                    reaching_defs,
                )
            }
        })
        .collect();

    KernelBody {
        ops: new_ops,
        child_bodies: final_children,
        literals: new_literals,
    }
}

/// Recursively apply `id_map` to every result-reference operand in
/// `body` and all nested child bodies. Result ids of ops themselves
/// are left unchanged  -  only operand refs are rewritten.
fn remap_body_ids(body: &KernelBody, id_map: &BTreeMap<u32, u32>) -> KernelBody {
    let new_ops: Vec<KernelOp> = body
        .ops
        .iter()
        .map(|op| {
            let new_operands: Vec<u32> = op
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
                operands: new_operands,
                result: op.result.map(|r| *id_map.get(&r).unwrap_or(&r)),
            }
        })
        .collect();
    let new_children: Vec<KernelBody> = body
        .child_bodies
        .iter()
        .map(|c| remap_body_ids(c, id_map))
        .collect();
    KernelBody {
        ops: new_ops,
        child_bodies: new_children,
        literals: body.literals.clone(),
    }
}

/// Split a loop body into (invariant_ops_to_hoist, loop_dependent_ops).
/// Invariants don't depend (transitively) on any value produced inside
/// the body, and have no side effects.
fn hoist_invariants(
    body: &KernelBody,
    read_only_bindings: &FxHashSet<u32>,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> (Vec<KernelOp>, KernelBody) {
    // Phase-1 conservative rule: an op is loop-dependent if any of its
    // result-id-position operands references a result-id produced
    // earlier in this body. The loop variable doesn't have a result-id
    // (it's implicit) so we treat ANY operand whose value is produced
    // inside the body as loop-dependent.
    // Include result ids produced by descendant child bodies. Hoisting
    // an op that references a child-body-local id (e.g. a phi-Select
    // after a StructuredIfThen) would dangle the reference outside the
    // loop. Top-level straight-line ids are handled by `dependent_ids`
    // below so invariant chains can hoist as a batch.
    fn collect_descendant_ids(body: &KernelBody, out: &mut FxHashSet<u32>) {
        for child in &body.child_bodies {
            for op in &child.ops {
                for r in op.result_ids() {
                    out.insert(r);
                }
            }
            collect_descendant_ids(child, out);
        }
    }
    let mut descendant_ids: FxHashSet<u32> = FxHashSet::default();
    collect_descendant_ids(body, &mut descendant_ids);
    let mut dependent_ids = FxHashSet::<u32>::default();

    let mut invariants = Vec::new();
    let mut remaining_ops = Vec::new();

    for op in &body.ops {
        if !is_hoistable(op, body, read_only_bindings, alias_facts, reaching_defs) {
            // Side effects → never hoist.
            dependent_ids.extend(op.result_ids());
            remaining_ops.push(op.clone());
            continue;
        }
        let depends = op.operands.iter().enumerate().any(|(pos, val)| {
            operand_is_result_reference(&op.kind, pos)
                && (dependent_ids.contains(val) || descendant_ids.contains(val))
        });
        if !depends {
            // Op is invariant. Hoist.
            invariants.push(op.clone());
            // Its result-id is now produced OUTSIDE the loop body, so
            // it's NOT in the dependent set.
        } else {
            // Loop-dependent.
            dependent_ids.extend(op.result_ids());
            remaining_ops.push(op.clone());
        }
    }

    let new_body = KernelBody {
        ops: remaining_ops,
        child_bodies: body.child_bodies.clone(),
        literals: body.literals.clone(),
    };
    (invariants, new_body)
}

fn is_hoistable(
    op: &KernelOp,
    body: &KernelBody,
    read_only_bindings: &FxHashSet<u32>,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> bool {
    if is_pure(&op.kind) {
        return true;
    }
    match op.kind {
        KernelOpKind::LoadConstant => true,
        KernelOpKind::LoadGlobal | KernelOpKind::LoadShared => {
            load_is_loop_invariant_memory(op, body, read_only_bindings, alias_facts, reaching_defs)
        }
        _ => false,
    }
}

fn load_is_loop_invariant_memory(
    load: &KernelOp,
    body: &KernelBody,
    read_only_bindings: &FxHashSet<u32>,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    reaching_defs: Option<&crate::analyses::reaching_def_facts::ReachingDefFactSet>,
) -> bool {
    if !body.child_bodies.is_empty() || load.operands.len() < 2 {
        return false;
    }
    let load_slot = load.operands[0];
    let load_index = resolve(load.operands[1], reaching_defs);
    let load_target = match load.kind {
        KernelOpKind::LoadGlobal => MemoryTarget::global(load_slot),
        KernelOpKind::LoadShared => MemoryTarget::shared(load_slot),
        _ => return false,
    };
    if matches!(load.kind, KernelOpKind::LoadGlobal) {
        let is_read_only = read_only_bindings.contains(&load_slot);
        let has_alias_facts = alias_facts.is_some();
        let has_reaching_defs = reaching_defs.is_some();
        // Conservative path: read-only slot + alias facts → safe.
        // Dataflow path: reaching-defs + alias facts → safe
        // (external facts can prove non-aliasing even on ReadWrite slots).
        // Otherwise: reject the hoist.
        if !(is_read_only && has_alias_facts) && !(has_reaching_defs && has_alias_facts) {
            return false;
        }
    }
    body.ops.iter().all(|candidate| {
        let writes_same_space = matches!(
            (&load.kind, &candidate.kind),
            (KernelOpKind::LoadGlobal, KernelOpKind::StoreGlobal)
                | (KernelOpKind::LoadShared, KernelOpKind::StoreShared)
                | (KernelOpKind::LoadGlobal, KernelOpKind::Atomic { .. })
        );
        if !writes_same_space {
            return !matches!(
                candidate.kind,
                KernelOpKind::Barrier { .. }
                    | KernelOpKind::AsyncLoad { .. }
                    | KernelOpKind::AsyncStore { .. }
                    | KernelOpKind::AsyncWait { .. }
                    | KernelOpKind::Call { .. }
                    | KernelOpKind::OpaqueExpr(..)
                    | KernelOpKind::OpaqueNode(..)
                    | KernelOpKind::Trap { .. }
                    | KernelOpKind::Resume { .. }
                    | KernelOpKind::Return
            );
        }
        if candidate.operands.len() < 2 {
            return false;
        }
        let store_slot = candidate.operands[0];
        let store_index = resolve(candidate.operands[1], reaching_defs);
        let store_target = match candidate.kind {
            KernelOpKind::StoreGlobal | KernelOpKind::Atomic { .. } => {
                MemoryTarget::global(store_slot)
            }
            KernelOpKind::StoreShared => MemoryTarget::shared(store_slot),
            _ => return false,
        };
        !locations_may_alias(
            MemoryLocation::new(load_target, load_index, AddressKey::Result(load_index)),
            MemoryLocation::new(store_target, store_index, AddressKey::Result(store_index)),
            alias_facts,
            SlotAliasPolicy::DistinctSlotsMayAlias,
        )
    })
}

/// Hoist-safety classifier: `true` only for ops that may be lifted out of a
/// loop (no memory write, no sync/control/dispatch effect, not a loop-varying
/// value). EXHAUSTIVE ON PURPOSE (no `_` wildcard): a future `KernelOpKind`
/// with a side effect must be classified here rather than silently defaulting
/// to "pure" and being hoisted out of the loop — a miscompile.
fn is_pure(kind: &KernelOpKind) -> bool {
    match kind {
        // Pure value / builtin / arithmetic ops: no memory or control effect.
        KernelOpKind::Literal
        | KernelOpKind::Copy
        | KernelOpKind::LocalInvocationId
        | KernelOpKind::GlobalInvocationId
        | KernelOpKind::WorkgroupId
        | KernelOpKind::SubgroupLocalId
        | KernelOpKind::SubgroupSize
        | KernelOpKind::BufferLength
        | KernelOpKind::BinOpKind(_)
        | KernelOpKind::UnOpKind(_)
        | KernelOpKind::Fma
        | KernelOpKind::MatrixMma { .. }
        | KernelOpKind::Select
        | KernelOpKind::Cast { .. }
        | KernelOpKind::SubgroupBallot
        | KernelOpKind::SubgroupShuffle
        | KernelOpKind::SubgroupBroadcast
        | KernelOpKind::SubgroupReduce { .. } => true,
        // Writes / atomics / barriers / async / traps / dispatch / calls /
        // opaque: a side or sync effect, never hoist.
        KernelOpKind::StoreGlobal
        | KernelOpKind::StoreShared
        | KernelOpKind::Barrier { .. }
        | KernelOpKind::Atomic { .. }
        | KernelOpKind::AsyncLoad { .. }
        | KernelOpKind::AsyncStore { .. }
        | KernelOpKind::AsyncWait { .. }
        | KernelOpKind::Trap { .. }
        | KernelOpKind::Resume { .. }
        | KernelOpKind::IndirectDispatch { .. }
        | KernelOpKind::Return
        | KernelOpKind::StructuredIfThen
        | KernelOpKind::StructuredIfThenElse
        | KernelOpKind::StructuredForLoop { .. }
        | KernelOpKind::StructuredBlock
        | KernelOpKind::Region { .. }
        | KernelOpKind::Call { .. }
        | KernelOpKind::OpaqueExpr(..)
        | KernelOpKind::OpaqueNode(..)
        // Loop induction / carrier values are not loop-invariant.
        | KernelOpKind::LoopIndex { .. }
        | KernelOpKind::LoopCarrierInit { .. }
        | KernelOpKind::LoopCarrier { .. }
        | KernelOpKind::LoopCarrierEnd { .. }
        // Loads aren't safely hoistable here  -  the underlying buffer could be
        // written by another thread between iterations (the dedicated
        // `load_is_loop_invariant_memory` path handles the provable cases).
        | KernelOpKind::LoadGlobal
        | KernelOpKind::LoadShared
        | KernelOpKind::LoadConstant => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };
    use vyre_foundation::ir::BinOp;

    fn empty_kernel_with_loop(loop_body: KernelBody) -> KernelDescriptor {
        KernelDescriptor {
            id: "loopy".into(),
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
                child_bodies: vec![loop_body],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(64)],
            },
        }
    }

    #[test]
    fn licm_on_empty_kernel_is_noop() {
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
        let out = licm(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn licm_kernel_with_no_loop_is_noop() {
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
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(3), LiteralValue::U32(4)],
            },
        };
        let out = licm(&desc);
        assert_eq!(out.body.ops.len(), 3);
    }

    #[test]
    fn licm_hoists_constant_out_of_loop() {
        // for i in 0..64 { lit(99); /* nothing else */ }
        // The Literal in the loop body has no operand dependencies; it's
        // a constant, hoistable.
        let desc = empty_kernel_with_loop(KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(2),
            }],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(99)],
        });
        let out = licm(&desc);
        // Outer body started with 3 ops. After LICM, the hoisted Literal
        // appears before the StructuredForLoop, making it 4.
        assert_eq!(out.body.ops.len(), 4);
        // Hoisted Literal is at position 2 (before the loop op which is now at 3).
        assert!(matches!(out.body.ops[2].kind, KernelOpKind::Literal));
        assert!(matches!(
            out.body.ops[3].kind,
            KernelOpKind::StructuredForLoop { .. }
        ));
        // Loop body should now be empty (the hoisted op moved out).
        let loop_body = out.body.child_bodies[0].clone();
        assert!(loop_body.ops.is_empty());
    }

    #[test]
    fn licm_hoists_straight_line_invariant_chain() {
        // Loop body: lit(5) (invariant); add(lit, lit) (invariant);
        // The Add uses earlier ops from the same invariant chain, so
        // LICM hoists the whole straight-line chain as one batch.
        let desc = empty_kernel_with_loop(KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(10),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(11),
                },
                // This Add USES the prior Literals  -  but those are now
                // hoisted, so the Add itself can also be hoisted.
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![10, 11],
                    result: Some(12),
                },
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(5), LiteralValue::U32(7)],
        });
        let out = licm(&desc);
        // After: outer ops = init lits (0,1) + 3 hoisted ops + loop op.
        assert_eq!(out.body.ops.len(), 6);
        let loop_body = &out.body.child_bodies[0];
        assert!(loop_body.ops.is_empty());
        assert!(matches!(
            out.body.ops[4].kind,
            KernelOpKind::BinOpKind(BinOp::Add)
        ));
    }

    #[test]
    fn licm_does_not_hoist_load() {
        // Loads are unsafely hoistable (other threads may write between
        // iterations). Phase 1 forbids hoisting all loads.
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        use vyre_foundation::ir::DataType;
        let desc = KernelDescriptor {
            id: "load_loop".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "buf".into(),
                }],
            },
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
                        // LoadGlobal  -  should NOT hoist.
                        KernelOp {
                            kind: KernelOpKind::LoadGlobal,
                            operands: vec![0, 10],
                            result: Some(11),
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(0)],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(8)],
            },
        };
        let out = licm(&desc);
        // The Literal hoists; the Load stays.
        let loop_body = &out.body.child_bodies[0];
        let has_load = loop_body
            .ops
            .iter()
            .any(|o| matches!(o.kind, KernelOpKind::LoadGlobal));
        assert!(has_load, "Load must stay inside the loop");
    }

    #[test]
    fn alias_aware_licm_hoists_read_only_loop_invariant_load() {
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        use vyre_foundation::ir::DataType;
        let desc = KernelDescriptor {
            id: "alias_readonly_load_loop".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadOnly,
                    name: "buf".into(),
                }],
            },
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
                            kind: KernelOpKind::LoadGlobal,
                            operands: vec![0, 10],
                            result: Some(11),
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(0)],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(8)],
            },
        };
        let facts = crate::analyses::alias_facts::AliasFactSet::default();
        let out = licm_with_alias_facts(&desc, &facts);
        let loop_body = &out.body.child_bodies[0];
        assert!(
            !loop_body
                .ops
                .iter()
                .any(|op| matches!(op.kind, KernelOpKind::LoadGlobal)),
            "read-only loop-invariant load should hoist through fact-aware LICM"
        );
    }

    #[test]
    fn different_binding_store_blocks_licm_load_hoist_without_external_no_alias_fact() {
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        use vyre_foundation::ir::DataType;
        let desc = KernelDescriptor {
            id: "licm_cross_binding_alias_conservative".into(),
            bindings: BindingLayout {
                slots: vec![
                    BindingSlot {
                        slot: 0,
                        element_type: DataType::U32,
                        element_count: None,
                        memory_class: MemoryClass::Global,
                        visibility: BindingVisibility::ReadOnly,
                        name: "input".into(),
                    },
                    BindingSlot {
                        slot: 1,
                        element_type: DataType::U32,
                        element_count: None,
                        memory_class: MemoryClass::Global,
                        visibility: BindingVisibility::ReadWrite,
                        name: "scratch".into(),
                    },
                ],
            },
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
                            kind: KernelOpKind::LoadGlobal,
                            operands: vec![0, 0],
                            result: Some(10),
                        },
                        KernelOp {
                            kind: KernelOpKind::StoreGlobal,
                            operands: vec![1, 1, 10],
                            result: None,
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(64)],
            },
        };

        let conservative = licm(&desc);
        assert!(
            conservative.body.child_bodies[0]
                .ops
                .iter()
                .any(|op| matches!(op.kind, KernelOpKind::LoadGlobal)),
            "cross-binding stores may alias the load without an external proof, so LICM must keep the load in-loop"
        );

        let mut facts = crate::analyses::alias_facts::AliasFactSet::default();
        facts.insert_no_alias(crate::analyses::alias_facts::NoAliasFact {
            left_binding: 0,
            left_index: 0,
            right_binding: 1,
            right_index: 1,
        });
        let alias_aware = licm_with_alias_facts(&desc, &facts);
        assert!(
            !alias_aware.body.child_bodies[0]
                .ops
                .iter()
                .any(|op| matches!(op.kind, KernelOpKind::LoadGlobal)),
            "external no-alias proof should recover cross-binding LICM load hoisting"
        );
    }

    #[test]
    fn licm_does_not_hoist_store() {
        // Stores are side-effecting. Hoisting changes semantics.
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        use vyre_foundation::ir::DataType;
        let desc = KernelDescriptor {
            id: "store_loop".into(),
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
                            kind: KernelOpKind::Literal,
                            operands: vec![1],
                            result: Some(11),
                        },
                        KernelOp {
                            kind: KernelOpKind::StoreGlobal,
                            operands: vec![0, 10, 11],
                            result: None,
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(8)],
            },
        };
        let out = licm(&desc);
        let loop_body = &out.body.child_bodies[0];
        let has_store = loop_body
            .ops
            .iter()
            .any(|o| matches!(o.kind, KernelOpKind::StoreGlobal));
        assert!(has_store, "Store must stay inside the loop");
    }

    #[test]
    fn licm_is_idempotent() {
        let desc = empty_kernel_with_loop(KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(2),
            }],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(99)],
        });
        let once = licm(&desc);
        let twice = licm(&once);
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
        assert_eq!(
            once.body.child_bodies[0].ops.len(),
            twice.body.child_bodies[0].ops.len()
        );
    }

    #[test]
    fn licm_handles_no_for_loop_op_gracefully() {
        // Body with StructuredIfThen but no for-loop  -  should be a noop on the loop side.
        let desc = KernelDescriptor {
            id: "if_only".into(),
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
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![0, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![LiteralValue::Bool(true)],
            },
        };
        let out = licm(&desc);
        // Outer ops stay at 2; nothing to hoist.
        assert_eq!(out.body.ops.len(), 2);
    }

    #[test]
    fn public_licm_hoists_invariants() {
        // The public `licm` API now performs a correct cross-body
        // hoist with id renumbering and literal-pool merging.
        let desc = empty_kernel_with_loop(KernelBody {
            ops: vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(2),
            }],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(99)],
        });
        let out = licm(&desc);
        // Outer body started with 3 ops. After LICM, the hoisted Literal
        // appears before the StructuredForLoop, making it 4.
        assert_eq!(out.body.ops.len(), 4, "hoisted literal adds one parent op");
        assert!(
            matches!(out.body.ops[2].kind, KernelOpKind::Literal),
            "hoisted op is at position 2"
        );
        assert!(
            matches!(out.body.ops[3].kind, KernelOpKind::StructuredForLoop { .. }),
            "loop op follows hoisted literal"
        );
        // Loop body should now be empty (the hoisted op moved out).
        assert!(
            out.body.child_bodies[0].ops.is_empty(),
            "child body empty after hoist"
        );
        // The parent literal pool gained the child's literal.
        assert_eq!(
            out.body.literals.len(),
            desc.body.literals.len() + 1,
            "parent literals grew by one"
        );
    }

    #[test]
    fn licm_does_not_corrupt_sibling_child_body_when_cond_id_aliases_body_index() {
        // Regression for the bug where LICM iterated ALL operands of
        // StructuredIfThen as child-body indices. For StructuredIfThen,
        // operand[0] is the condition result-id, NOT a child-body index.
        // When cond_result_id == some valid child_bodies index, the old
        // code would silently recurse into and overwrite that unrelated
        // child body.
        //
        // Layout:
        //   parent has two child bodies (indices 0 and 1).
        //   child_bodies[0] = a body containing a sentinel literal  — must NOT be touched.
        //   child_bodies[1] = the actual if-then body.
        //   StructuredIfThen: operands = [cond=r0, then_body_idx=1].
        //   cond r0 has result-id 0, which is also a valid child-body index (0).
        //   Buggy code: treats operand[0]=0 as a child-body index → recurses into
        //   child_bodies[0] and overwrites it → sentinel literal lost.
        //   Fixed code: only operand[1] is a child-body index → child_bodies[0] intact.
        let sentinel_val = LiteralValue::U32(0xDEAD_BEEF);
        let desc = KernelDescriptor {
            id: "licm_cond_alias".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    // r0 = Lit(true) — condition; its result-id=0 also == child_bodies[0] index.
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    // StructuredIfThen: cond=r0 (operand[0]), then_body=child_bodies[1] (operand[1]).
                    KernelOp {
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![
                    // child_bodies[0]: the unrelated body with the sentinel.
                    // Must NOT be touched by the StructuredIfThen LICM arm.
                    KernelBody {
                        ops: vec![KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![0],
                            result: Some(10),
                        }],
                        child_bodies: vec![],
                        literals: vec![sentinel_val.clone()],
                    },
                    // child_bodies[1]: the actual then-body (empty).
                    KernelBody {
                        ops: vec![],
                        child_bodies: vec![],
                        literals: vec![],
                    },
                ],
                literals: vec![LiteralValue::Bool(true)],
            },
        };
        let out = licm(&desc);
        // child_bodies[0]'s literal pool must be UNCHANGED: sentinel value present.
        assert_eq!(
            out.body.child_bodies[0].literals.first().cloned(),
            Some(sentinel_val),
            "child_bodies[0] must not be corrupted; \
             LICM must not walk cond_result_id=0 as a child-body index"
        );
        // Outer ops stay at 2: no loop to hoist from.
        assert_eq!(
            out.body.ops.len(),
            2,
            "no loop ops present; outer op count must not change"
        );
    }

    #[test]
    fn dataflow_licm_hoists_load_from_readwrite_buffer() {
        // Reproduces the `dataflow-licm.equivalent_alias_indices` corpus:
        // A ReadWrite buffer with a LoadGlobal(idx=20) and StoreGlobal(idx=40)
        // inside a loop. Reaching-defs: 20→11, 40→12. Alias facts: (0,11)≠(0,12).
        // LICM should hoist the Load because the store's resolved index provably
        // doesn't alias the load's resolved index.
        use crate::{BindingSlot, BindingVisibility, MemoryClass};
        use vyre_foundation::ir::DataType;

        let desc = KernelDescriptor {
            id: "rw_licm".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: Some(4096),
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "buf".into(),
                }],
            },
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
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(11),
                    },
                    KernelOp {
                        kind: KernelOpKind::Copy,
                        operands: vec![11],
                        result: Some(20),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![3],
                        result: Some(12),
                    },
                    KernelOp {
                        kind: KernelOpKind::Copy,
                        operands: vec![12],
                        result: Some(40),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![4],
                        result: Some(31),
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
                            kind: KernelOpKind::LoadGlobal,
                            operands: vec![0, 20],
                            result: Some(50),
                        },
                        KernelOp {
                            kind: KernelOpKind::StoreGlobal,
                            operands: vec![0, 40, 31],
                            result: None,
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![],
                }],
                literals: vec![
                    LiteralValue::U32(0),
                    LiteralValue::U32(64),
                    LiteralValue::U32(42),
                    LiteralValue::U32(13),
                    LiteralValue::U32(9),
                ],
            },
        };

        let mut alias_facts = crate::analyses::alias_facts::AliasFactSet::default();
        alias_facts.insert_no_alias(crate::analyses::alias_facts::NoAliasFact {
            left_binding: 0,
            left_index: 11,
            right_binding: 0,
            right_index: 12,
        });

        let mut reaching_defs = crate::analyses::reaching_def_facts::ReachingDefFactSet::default();
        reaching_defs.set_reaching_defs(20, vec![11]);
        reaching_defs.set_reaching_defs(40, vec![12]);

        let out = licm_with_dataflow_facts(&desc, &alias_facts, &reaching_defs);

        // After hoist, LoadGlobal should be in the parent body, not the loop body.
        let parent_loads = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::LoadGlobal))
            .count();
        let child_loads = out.body.child_bodies[0]
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::LoadGlobal))
            .count();
        assert_eq!(
            child_loads,
            0,
            "LoadGlobal should be hoisted out of loop body; child ops: {:?}",
            out.body.child_bodies[0]
                .ops
                .iter()
                .map(|o| format!("{:?} operands={:?}", o.kind, o.operands))
                .collect::<Vec<_>>()
        );
        assert!(
            parent_loads > 0,
            "LoadGlobal should appear in parent body after hoist"
        );
    }
}
