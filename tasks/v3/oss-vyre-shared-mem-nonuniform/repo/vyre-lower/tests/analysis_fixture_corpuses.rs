//! Test: analysis fixture corpuses.
use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::analyses::{
    analyze_bank_conflict, analyze_coalesce, analyze_shared_mem_promote, vec_pack,
};
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn global_slot(slot: u32, name: &str, visibility: BindingVisibility) -> BindingSlot {
    BindingSlot {
        slot,
        element_type: DataType::U32,
        element_count: Some(4096),
        memory_class: MemoryClass::Global,
        visibility,
        name: name.to_string(),
    }
}

fn shared_slot(slot: u32, name: &str) -> BindingSlot {
    BindingSlot {
        slot,
        element_type: DataType::U32,
        element_count: Some(4096),
        memory_class: MemoryClass::Shared,
        visibility: BindingVisibility::ReadWrite,
        name: name.to_string(),
    }
}

fn op(kind: KernelOpKind, operands: Vec<u32>, result: Option<u32>) -> KernelOp {
    KernelOp {
        kind,
        operands,
        result,
    }
}

fn literal(pool_index: u32, result: u32) -> KernelOp {
    op(KernelOpKind::Literal, vec![pool_index], Some(result))
}

fn local_x(result: u32) -> KernelOp {
    op(KernelOpKind::LocalInvocationId, vec![0], Some(result))
}

fn mul(left: u32, right: u32, result: u32) -> KernelOp {
    op(
        KernelOpKind::BinOpKind(BinOp::Mul),
        vec![left, right],
        Some(result),
    )
}

fn descriptor(
    id: &str,
    slots: Vec<BindingSlot>,
    ops: Vec<KernelOp>,
    literals: Vec<LiteralValue>,
) -> KernelDescriptor {
    KernelDescriptor {
        id: id.to_string(),
        bindings: BindingLayout { slots },
        dispatch: Dispatch::new(256, 1, 1),
        body: KernelBody {
            ops,
            child_bodies: vec![],
            literals,
        },
    }
}

#[test]
fn a13_coalesce_corpus_classifies_unit_stride_strided_and_broadcast() {
    let desc = descriptor(
        "a13_coalesce_fixture",
        vec![global_slot(0, "input", BindingVisibility::ReadOnly)],
        vec![
            local_x(1),
            literal(0, 2),
            literal(1, 3),
            mul(1, 2, 4),
            op(KernelOpKind::LoadGlobal, vec![0, 1], Some(10)),
            op(KernelOpKind::LoadGlobal, vec![0, 4], Some(11)),
            op(KernelOpKind::LoadGlobal, vec![0, 3], Some(12)),
        ],
        vec![LiteralValue::U32(4), LiteralValue::U32(7)],
    );

    let report = analyze_coalesce(&desc);
    assert_eq!(report.sites.len(), 3);
    assert_eq!(
        report.sites[0].pattern,
        vyre_lower::analyses::coalesce::AccessPattern::CoalescedUnitStride
    );
    assert_eq!(
        report.sites[1].pattern,
        vyre_lower::analyses::coalesce::AccessPattern::Strided { stride: 4 }
    );
    assert_eq!(
        report.sites[2].pattern,
        vyre_lower::analyses::coalesce::AccessPattern::Broadcast
    );
    assert_eq!(report.problematic_count(), 1);
}

#[test]
fn a14_shared_mem_promote_corpus_finds_reused_global_tile() {
    let desc = descriptor(
        "a14_shared_mem_promote_fixture",
        vec![global_slot(0, "hot", BindingVisibility::ReadOnly)],
        vec![
            local_x(1),
            op(KernelOpKind::LoadGlobal, vec![0, 1], Some(10)),
            op(KernelOpKind::LoadGlobal, vec![0, 1], Some(11)),
            op(KernelOpKind::LoadGlobal, vec![0, 1], Some(12)),
        ],
        vec![],
    );

    let plan = analyze_shared_mem_promote(&desc);
    assert_eq!(plan.candidates.len(), 1);
    let candidate = &plan.candidates[0];
    assert_eq!(candidate.binding_slot, 0);
    assert_eq!(candidate.access_count, 3);
    assert_eq!(candidate.tile_bytes, 1024);
    assert!(plan.fits_in_budget());
}

#[test]
fn a15_bank_conflict_corpus_detects_full_warp_serialization() {
    let desc = descriptor(
        "a15_bank_conflict_fixture",
        vec![shared_slot(2, "tile")],
        vec![
            local_x(1),
            literal(0, 2),
            mul(1, 2, 3),
            op(KernelOpKind::LoadShared, vec![2, 3], Some(10)),
        ],
        vec![LiteralValue::U32(32)],
    );

    let report = analyze_bank_conflict(&desc);
    assert_eq!(report.sites.len(), 1);
    assert_eq!(
        report.sites[0].conflict,
        vyre_lower::analyses::bank_conflict::BankConflictKind::Conflict { way_count: 32 }
    );
    assert_eq!(report.critical_count(), 1);
}

#[test]
fn a16_vec_pack_corpus_detects_adjacent_load_chain() {
    let desc = descriptor(
        "a16_vec_pack_fixture",
        vec![global_slot(0, "input", BindingVisibility::ReadOnly)],
        vec![
            literal(0, 1),
            literal(1, 2),
            literal(2, 3),
            literal(3, 4),
            op(KernelOpKind::LoadGlobal, vec![0, 1], Some(10)),
            op(KernelOpKind::LoadGlobal, vec![0, 2], Some(11)),
            op(KernelOpKind::LoadGlobal, vec![0, 3], Some(12)),
            op(KernelOpKind::LoadGlobal, vec![0, 4], Some(13)),
        ],
        vec![
            LiteralValue::U32(64),
            LiteralValue::U32(65),
            LiteralValue::U32(66),
            LiteralValue::U32(67),
        ],
    );

    let report = vec_pack::analyze(&desc);
    assert!(report.has_chains());
    assert_eq!(report.chains.len(), 1);
    assert_eq!(report.chains[0].slot, 0);
    assert_eq!(report.chains[0].start_index, 64);
    assert_eq!(report.chains[0].pack_width(), 4);
    assert_eq!(report.total_ops_eliminated, 3);
}
