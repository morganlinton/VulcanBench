//! Analysis pass: walk a `KernelDescriptor`, identify groups of
//! adjacent loads/stores that can fuse into one packed vector op.
//!
//! ## Algorithm (phase 1)
//!
//! 1. Scan the op stream linearly.
//! 2. When we hit a `LoadGlobal` or `StoreGlobal`, look at the next
//!    1, 2, or 3 ops. If they are loads/stores of the same kind, on
//!    the same buffer, with indices that differ by exactly +1 (i.e.
//!    consecutive offsets), and the same target dtype, they form a
//!    pack group.
//! 3. Greedy maximal grouping: prefer Vec4 over Vec2 when possible.
//! 4. Skip any group that crosses a side-effecting op on the same
//!    buffer (RAW/WAR hazard).
//!
//! Index-difference detection is conservative  -  phase 1 only proves
//! `+1` increment when the index expressions both decompose as
//! `Add(<same base>, LiteralU32(<k>))` and `Add(<same base>, LiteralU32(<k+1>))`,
//! OR when one is `<base>` and the other is `Add(<base>, LiteralU32(1))`.
//! Anything more exotic (multi-term polynomials, runtime base) falls
//! through to "not packable"  -  phase 2 may upgrade.

use super::plan::{PackGroup, PackKind, PackingPlan};
use vyre_foundation::ir::BinOp;
use vyre_lower::analyses::AccessKind;
use vyre_lower::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};

/// Run vec-packing analysis.
#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> PackingPlan {
    let mut groups = Vec::new();
    walk_body(&desc.body, &mut groups);
    PackingPlan {
        kernel_id: desc.id.clone(),
        groups,
    }
}

fn walk_body(body: &KernelBody, groups: &mut Vec<PackGroup>) {
    // Phase 1: collect every Load/Store op with its decomposed index.
    // Upper bound on accesses is body.ops.len() (one record per op at
    // most). Pre-sizing avoids three-to-four reallocations on the
    // typical megakernel body (dozens to hundreds of ops).
    let mut accesses: Vec<AccessRecord> = Vec::with_capacity(body.ops.len());
    for (op_idx, op) in body.ops.iter().enumerate() {
        if let Some(kind) = access_kind(&op.kind) {
            if op.operands.len() < 2 {
                continue;
            }
            let buffer_slot = op.operands[0];
            let index_id = op.operands[1];
            let value_id = op.operands.get(2).copied();
            if let Some((base, offset)) = decompose_index(body, index_id) {
                accesses.push(AccessRecord {
                    op_index: op_idx,
                    kind,
                    buffer_slot,
                    base,
                    offset,
                    value_id,
                });
            }
        }
        // Recurse into structured children.
        if matches!(
            op.kind,
            KernelOpKind::StructuredIfThen | KernelOpKind::StructuredForLoop { .. }
        ) {
            if let Some(child_id) = op.operands.last() {
                if let Some(child) = body.child_bodies.get(*child_id as usize) {
                    walk_body(child, groups);
                }
            }
        }
    }

    // Phase 2: greedily group consecutive accesses (in collection order)
    // that match same kind + same buffer + same base + offsets that
    // form `k, k+1, k+2, ...`. Max group size 4 (Vec4).
    //
    // Hazard barrier: a store between loads to the same buffer breaks
    // the chain (RAW). We track per-buffer "last-store-position" and
    // refuse to grow a group across one.
    let mut i = 0;
    while i < accesses.len() {
        let start = &accesses[i];
        let mut size = 1;
        while size < 4 && i + size < accesses.len() {
            let prev = &accesses[i + size - 1];
            let next = &accesses[i + size];
            if next.kind != start.kind
                || next.buffer_slot != start.buffer_slot
                || next.base != start.base
                || next.offset != prev.offset + 1
            {
                break;
            }
            // Hazard check: if this is a Load group and a Store to the
            // same buffer happened between prev and next, abort.
            if start.kind == AccessKind::Load
                && hazard_store_between(&accesses, i + size - 1, i + size, start.buffer_slot)
            {
                break;
            }
            size += 1;
        }
        if size >= 2 {
            let pack = match size {
                2 => PackKind::Vec2,
                3 => PackKind::Vec3,
                _ => PackKind::Vec4,
            };
            groups.push(PackGroup {
                start_op_index: accesses[i].op_index,
                end_op_index: accesses[i + size - 1].op_index,
                kind: start.kind,
                binding_slot: start.buffer_slot,
                pack,
            });
            i += size;
        } else {
            i += 1;
        }
    }
}

#[derive(Debug)]
struct AccessRecord {
    op_index: usize,
    kind: AccessKind,
    buffer_slot: u32,
    base: u32,
    offset: u32,
    #[allow(dead_code)]
    value_id: Option<u32>,
}

fn access_kind(kind: &KernelOpKind) -> Option<AccessKind> {
    match kind {
        KernelOpKind::LoadGlobal => Some(AccessKind::Load),
        KernelOpKind::StoreGlobal => Some(AccessKind::Store),
        _ => None,
    }
}

/// Check whether a Store to `buffer_slot` exists in `accesses` at any
/// position strictly between `prev_idx` and `next_idx` (exclusive).
fn hazard_store_between(
    accesses: &[AccessRecord],
    prev_idx: usize,
    next_idx: usize,
    buffer_slot: u32,
) -> bool {
    let prev_op_pos = accesses[prev_idx].op_index;
    let next_op_pos = accesses[next_idx].op_index;
    accesses.iter().any(|a| {
        a.kind == AccessKind::Store
            && a.buffer_slot == buffer_slot
            && a.op_index > prev_op_pos
            && a.op_index < next_op_pos
    })
}

/// Decompose an index expression into `(base_operand_id, constant_offset)`.
/// `Some((base, 0))` means the index IS the base.
/// `Some((base, k))` means `Add(base, LiteralU32(k))`.
/// `None` means the index isn't recognizable in phase 1.
fn decompose_index(body: &KernelBody, operand_id: u32) -> Option<(u32, u32)> {
    let producer = body.ops.iter().rfind(|op| op.result == Some(operand_id))?;
    match producer.kind {
        KernelOpKind::BinOpKind(BinOp::Add) => {
            if producer.operands.len() != 2 {
                return None;
            }
            let lhs_const = literal_u32_value(body, producer.operands[0]);
            let rhs_const = literal_u32_value(body, producer.operands[1]);
            match (lhs_const, rhs_const) {
                (Some(k), None) => Some((producer.operands[1], k)),
                (None, Some(k)) => Some((producer.operands[0], k)),
                _ => None,
            }
        }
        // Anything else: treat the operand id itself as the "base"
        // with constant offset 0. This lets us match the case
        // (prev = base, next = Add(base, 1)).
        _ => Some((operand_id, 0)),
    }
}

fn literal_u32_value(body: &KernelBody, operand_id: u32) -> Option<u32> {
    let producer = body.ops.iter().rfind(|op| op.result == Some(operand_id))?;
    if producer.kind != KernelOpKind::Literal {
        return None;
    }
    let pool_idx = producer.operands.first()?;
    match body.literals.get(*pool_idx as usize)? {
        LiteralValue::U32(v) => Some(*v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::DataType;
    use vyre_lower::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };

    fn op(kind: KernelOpKind, operands: Vec<u32>, result: Option<u32>) -> KernelOp {
        KernelOp {
            kind,
            operands,
            result,
        }
    }

    fn binding(slot: u32) -> BindingSlot {
        BindingSlot {
            slot,
            element_type: DataType::F32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: format!("buf{slot}"),
        }
    }

    fn k(
        slots: Vec<BindingSlot>,
        ops: Vec<KernelOp>,
        literals: Vec<LiteralValue>,
    ) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals,
            },
        }
    }

    // ============== Positive truth (group detected) ==============

    #[test]
    fn positive_four_consecutive_loads_form_vec4_group() {
        // Setup: base = LocalInvocationId. Then load(buf, base+0..3).
        let kk = k(
            vec![binding(0)],
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)), // base, id 0
                op(KernelOpKind::Literal, vec![0], Some(10)),         // 0
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 10], Some(11)), // base+0
                op(KernelOpKind::LoadGlobal, vec![0, 11], Some(20)),
                op(KernelOpKind::Literal, vec![1], Some(12)), // 1
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 12], Some(13)), // base+1
                op(KernelOpKind::LoadGlobal, vec![0, 13], Some(21)),
                op(KernelOpKind::Literal, vec![2], Some(14)), // 2
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 14], Some(15)), // base+2
                op(KernelOpKind::LoadGlobal, vec![0, 15], Some(22)),
                op(KernelOpKind::Literal, vec![3], Some(16)), // 3
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 16], Some(17)), // base+3
                op(KernelOpKind::LoadGlobal, vec![0, 17], Some(23)),
            ],
            vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(2),
                LiteralValue::U32(3),
            ],
        );
        let p = analyze(&kk);
        assert_eq!(p.groups.len(), 1);
        assert_eq!(p.groups[0].pack, PackKind::Vec4);
        assert_eq!(p.groups[0].kind, AccessKind::Load);
        assert_eq!(p.groups[0].binding_slot, 0);
        assert_eq!(p.groups[0].op_count(), 4);
        assert_eq!(p.ops_eliminated(), 3);
    }

    #[test]
    fn positive_two_consecutive_loads_form_vec2_group() {
        let kk = k(
            vec![binding(0)],
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(10)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 10], Some(11)),
                op(KernelOpKind::LoadGlobal, vec![0, 11], Some(20)),
                op(KernelOpKind::Literal, vec![1], Some(12)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 12], Some(13)),
                op(KernelOpKind::LoadGlobal, vec![0, 13], Some(21)),
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        );
        let p = analyze(&kk);
        assert_eq!(p.groups.len(), 1);
        assert_eq!(p.groups[0].pack, PackKind::Vec2);
    }

    #[test]
    fn positive_three_consecutive_stores_form_vec3_group() {
        let kk = k(
            vec![binding(0)],
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(10)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 10], Some(11)),
                op(KernelOpKind::Literal, vec![3], Some(99)),
                op(KernelOpKind::StoreGlobal, vec![0, 11, 99], None),
                op(KernelOpKind::Literal, vec![1], Some(12)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 12], Some(13)),
                op(KernelOpKind::StoreGlobal, vec![0, 13, 99], None),
                op(KernelOpKind::Literal, vec![2], Some(14)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 14], Some(15)),
                op(KernelOpKind::StoreGlobal, vec![0, 15, 99], None),
            ],
            vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(2),
                LiteralValue::U32(7),
            ],
        );
        let p = analyze(&kk);
        assert_eq!(p.groups.len(), 1);
        assert_eq!(p.groups[0].pack, PackKind::Vec3);
        assert_eq!(p.groups[0].kind, AccessKind::Store);
    }

    // ============== Negative precision ==============

    #[test]
    fn negative_loads_with_non_consecutive_offsets_not_packed() {
        // Indices base+0 and base+2  -  gap of 1  -  not packable.
        let kk = k(
            vec![binding(0)],
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(10)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 10], Some(11)),
                op(KernelOpKind::LoadGlobal, vec![0, 11], Some(20)),
                op(KernelOpKind::Literal, vec![1], Some(12)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 12], Some(13)),
                op(KernelOpKind::LoadGlobal, vec![0, 13], Some(21)),
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(2)],
        );
        let p = analyze(&kk);
        assert!(p.groups.is_empty());
    }

    #[test]
    fn negative_loads_on_different_buffers_not_packed() {
        let kk = k(
            vec![binding(0), binding(1)],
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(10)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 10], Some(11)),
                op(KernelOpKind::LoadGlobal, vec![0, 11], Some(20)),
                op(KernelOpKind::Literal, vec![1], Some(12)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 12], Some(13)),
                op(KernelOpKind::LoadGlobal, vec![1, 13], Some(21)),
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        );
        let p = analyze(&kk);
        assert!(p.groups.is_empty());
    }

    #[test]
    fn negative_load_then_store_not_packed() {
        let kk = k(
            vec![binding(0)],
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(10)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 10], Some(11)),
                op(KernelOpKind::LoadGlobal, vec![0, 11], Some(20)),
                op(KernelOpKind::Literal, vec![1], Some(12)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 12], Some(13)),
                op(KernelOpKind::StoreGlobal, vec![0, 13, 20], None),
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        );
        let p = analyze(&kk);
        assert!(p.groups.is_empty());
    }

    #[test]
    fn negative_no_global_accesses_yields_empty_plan() {
        let kk = k(
            vec![binding(0)],
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 1], Some(2)),
            ],
            vec![LiteralValue::U32(7)],
        );
        let p = analyze(&kk);
        assert!(p.groups.is_empty());
    }

    // ============== Adversarial ==============

    #[test]
    fn adversarial_five_consecutive_loads_pack_first_four_only() {
        // Vec4 is the max group size in phase 1. Five consecutive
        // loads should pack the first four and leave the fifth alone.
        let kk = k(
            vec![binding(0)],
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(10)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 10], Some(11)),
                op(KernelOpKind::LoadGlobal, vec![0, 11], Some(20)),
                op(KernelOpKind::Literal, vec![1], Some(12)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 12], Some(13)),
                op(KernelOpKind::LoadGlobal, vec![0, 13], Some(21)),
                op(KernelOpKind::Literal, vec![2], Some(14)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 14], Some(15)),
                op(KernelOpKind::LoadGlobal, vec![0, 15], Some(22)),
                op(KernelOpKind::Literal, vec![3], Some(16)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 16], Some(17)),
                op(KernelOpKind::LoadGlobal, vec![0, 17], Some(23)),
                op(KernelOpKind::Literal, vec![4], Some(18)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 18], Some(19)),
                op(KernelOpKind::LoadGlobal, vec![0, 19], Some(24)),
            ],
            vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(2),
                LiteralValue::U32(3),
                LiteralValue::U32(4),
            ],
        );
        let p = analyze(&kk);
        // First 4 pack as Vec4. The 5th load is a singleton.
        assert_eq!(p.groups.len(), 1);
        assert_eq!(p.groups[0].pack, PackKind::Vec4);
    }

    #[test]

    fn adversarial_loads_with_compute_op_between_still_pack() {
        // load(buf, base+0); add(...); load(buf, base+1)
        // The intervening compute op is pure (consumes the loaded
        // value, doesn't touch the buffer)  -  this is exactly the
        // pattern a real lowered op produces. Two-phase analysis
        // (collect-then-group) treats them as adjacent accesses.
        let kk = k(
            vec![binding(0)],
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(10)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 10], Some(11)),
                op(KernelOpKind::LoadGlobal, vec![0, 11], Some(20)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![20, 20], Some(99)), // pure compute, no buffer touch
                op(KernelOpKind::Literal, vec![1], Some(12)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 12], Some(13)),
                op(KernelOpKind::LoadGlobal, vec![0, 13], Some(21)),
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(1)],
        );
        let p = analyze(&kk);
        assert_eq!(p.groups.len(), 1);
        assert_eq!(p.groups[0].pack, PackKind::Vec2);
    }

    #[test]
    fn adversarial_load_then_store_then_load_breaks_group_via_hazard() {
        // load(buf, base+0); store(buf, base+5, ...); load(buf, base+1)
        // The intervening Store to the same buffer creates a RAW hazard.
        // The two loads must NOT pack  -  phase-1 hazard barrier.
        let kk = k(
            vec![binding(0)],
            vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(10)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 10], Some(11)),
                op(KernelOpKind::LoadGlobal, vec![0, 11], Some(20)),
                op(KernelOpKind::Literal, vec![3], Some(98)), // 5
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 98], Some(50)), // base+5
                op(KernelOpKind::Literal, vec![0], Some(99)), // value to store
                op(KernelOpKind::StoreGlobal, vec![0, 50, 99], None),
                op(KernelOpKind::Literal, vec![1], Some(12)),
                op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 12], Some(13)),
                op(KernelOpKind::LoadGlobal, vec![0, 13], Some(21)),
            ],
            vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(7),
                LiteralValue::U32(5),
            ],
        );
        let p = analyze(&kk);
        // RAW hazard barrier prevents packing the two loads.
        let load_groups: Vec<_> = p
            .groups
            .iter()
            .filter(|g| g.kind == AccessKind::Load)
            .collect();
        assert!(load_groups.is_empty(), "RAW hazard must prevent grouping");
    }

    #[test]
    fn adversarial_load_with_no_operands_skipped_safely() {
        let kk = k(
            vec![binding(0)],
            vec![op(KernelOpKind::LoadGlobal, vec![], None)],
            vec![],
        );
        let p = analyze(&kk);
        assert!(p.groups.is_empty());
    }

    #[test]
    fn adversarial_load_inside_loop_body_packs_inner_group() {
        // Phase 1 walks structured-body children, so a 4-load group
        // inside a for-loop should pack.
        let kk = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![binding(0)],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    op(KernelOpKind::Literal, vec![0], Some(0)),
                    op(KernelOpKind::Literal, vec![1], Some(1)),
                    op(
                        KernelOpKind::StructuredForLoop {
                            loop_var: "".into(),
                        },
                        vec![0, 1, 0],
                        None,
                    ),
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![
                        op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                        op(KernelOpKind::Literal, vec![0], Some(10)),
                        op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 10], Some(11)),
                        op(KernelOpKind::LoadGlobal, vec![0, 11], Some(20)),
                        op(KernelOpKind::Literal, vec![1], Some(12)),
                        op(KernelOpKind::BinOpKind(BinOp::Add), vec![0, 12], Some(13)),
                        op(KernelOpKind::LoadGlobal, vec![0, 13], Some(21)),
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(1)],
            },
        };
        let p = analyze(&kk);
        assert_eq!(p.groups.len(), 1);
        assert_eq!(p.groups[0].pack, PackKind::Vec2);
    }

    // ============== Aggregation ==============

    #[test]
    fn empty_kernel_yields_empty_plan() {
        let kk = k(vec![], vec![], vec![]);
        let p = analyze(&kk);
        assert!(p.groups.is_empty());
        assert_eq!(p.ops_eliminated(), 0);
        assert_eq!(p.kernel_id, "k");
    }
}
