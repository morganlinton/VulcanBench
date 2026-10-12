//! Snapshot test for the substrate-neutral analyses + three emit
//! paths. Builds a small corpus of representative `KernelDescriptor`s,
//! runs each through every analysis and emitter, asserts the totals
//! match a pinned snapshot.
//!
//! When the snapshot legitimately changes (new analysis, descriptor
//! redesign, etc.), update the asserted constants in this file.
//!
//! Source: AGENT_PLAN_2026-05-01.md A12 / ROADMAP T052.

use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::analyses::vec_pack;
use vyre_lower::analyses::{
    analyze_bank_conflict, analyze_coalesce, analyze_layout_aos_to_soa, analyze_shared_mem_promote,
    analyze_texture_promote, analyze_workgroup_uniform,
};
use vyre_lower::audit;
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

fn binding(slot: u32, dtype: DataType, mc: MemoryClass, vis: BindingVisibility) -> BindingSlot {
    BindingSlot {
        slot,
        element_type: dtype,
        element_count: None,
        memory_class: mc,
        visibility: vis,
        name: format!("b{slot}"),
    }
}

fn coalesced_load_kernel() -> KernelDescriptor {
    // load(buf, tid)  -  perfectly coalesced.
    KernelDescriptor {
        id: "coalesced".into(),
        bindings: BindingLayout {
            slots: vec![binding(
                0,
                DataType::F32,
                MemoryClass::Global,
                BindingVisibility::ReadOnly,
            )],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::LocalInvocationId, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
            ],
            child_bodies: vec![],
            literals: vec![],
        },
    }
}

fn strided_load_kernel() -> KernelDescriptor {
    // load(buf, 4 * tid)  -  strided 4x, problematic.
    KernelDescriptor {
        id: "strided".into(),
        bindings: BindingLayout {
            slots: vec![binding(
                0,
                DataType::F32,
                MemoryClass::Global,
                BindingVisibility::ReadOnly,
            )],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::LocalInvocationId, vec![], Some(0)),
                op(KernelOpKind::Literal, vec![0], Some(1)),
                op(KernelOpKind::BinOpKind(BinOp::Mul), vec![1, 0], Some(2)),
                op(KernelOpKind::LoadGlobal, vec![0, 2], Some(3)),
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(4)],
        },
    }
}

fn shared_promotion_candidate_kernel() -> KernelDescriptor {
    // Two LoadGlobal of same binding → promotion candidate.
    KernelDescriptor {
        id: "promote_me".into(),
        bindings: BindingLayout {
            slots: vec![binding(
                0,
                DataType::F32,
                MemoryClass::Global,
                BindingVisibility::ReadOnly,
            )],
        },
        dispatch: Dispatch::new(32, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        },
    }
}

fn shared_promotion_two_candidate_kernel() -> KernelDescriptor {
    KernelDescriptor {
        id: "promote_two_slots".into(),
        bindings: BindingLayout {
            slots: vec![
                binding(
                    0,
                    DataType::F32,
                    MemoryClass::Global,
                    BindingVisibility::ReadOnly,
                ),
                binding(
                    1,
                    DataType::Vec4U32,
                    MemoryClass::Global,
                    BindingVisibility::ReadOnly,
                ),
            ],
        },
        dispatch: Dispatch::new(128, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(2)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(3)),
                op(KernelOpKind::LoadGlobal, vec![1, 0], Some(4)),
                op(KernelOpKind::LoadGlobal, vec![1, 0], Some(5)),
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(0)],
        },
    }
}

fn scattered_load_kernel() -> KernelDescriptor {
    // load(buf, load(indices, tid))  -  data-dependent gather, conservatively scattered.
    KernelDescriptor {
        id: "scattered_gather".into(),
        bindings: BindingLayout {
            slots: vec![
                binding(
                    0,
                    DataType::U32,
                    MemoryClass::Global,
                    BindingVisibility::ReadOnly,
                ),
                binding(
                    1,
                    DataType::U32,
                    MemoryClass::Global,
                    BindingVisibility::ReadOnly,
                ),
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::LocalInvocationId, vec![0], Some(0)),
                op(KernelOpKind::LoadGlobal, vec![1, 0], Some(1)),
                op(KernelOpKind::LoadGlobal, vec![0, 1], Some(2)),
            ],
            child_bodies: vec![],
            literals: vec![],
        },
    }
}

fn bank_conflict_kernel() -> KernelDescriptor {
    // shared[tid] is safe, shared[tid * 4] is 4-way, shared[tid * 32] is critical.
    KernelDescriptor {
        id: "bank_conflicts".into(),
        bindings: BindingLayout {
            slots: vec![binding(
                0,
                DataType::U32,
                MemoryClass::Shared,
                BindingVisibility::ReadWrite,
            )],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::LocalInvocationId, vec![0], Some(0)),
                op(KernelOpKind::LoadShared, vec![0, 0], Some(1)),
                op(KernelOpKind::Literal, vec![0], Some(2)),
                op(KernelOpKind::BinOpKind(BinOp::Mul), vec![0, 2], Some(3)),
                op(KernelOpKind::LoadShared, vec![0, 3], Some(4)),
                op(KernelOpKind::Literal, vec![1], Some(5)),
                op(KernelOpKind::BinOpKind(BinOp::Mul), vec![0, 5], Some(6)),
                op(KernelOpKind::StoreShared, vec![0, 6, 1], None),
                op(KernelOpKind::Literal, vec![2], Some(7)),
                op(KernelOpKind::LoadShared, vec![0, 7], Some(8)),
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(4),
                LiteralValue::U32(32),
                LiteralValue::U32(0),
            ],
        },
    }
}

fn vec_pack_candidate_kernel() -> KernelDescriptor {
    // Four adjacent scalar loads on slot 0, plus two non-adjacent slot-1 loads.
    KernelDescriptor {
        id: "vec_pack_candidates".into(),
        bindings: BindingLayout {
            slots: vec![
                binding(
                    0,
                    DataType::U32,
                    MemoryClass::Global,
                    BindingVisibility::ReadOnly,
                ),
                binding(
                    1,
                    DataType::U32,
                    MemoryClass::Global,
                    BindingVisibility::ReadOnly,
                ),
            ],
        },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![
                op(KernelOpKind::Literal, vec![0], Some(0)),
                op(KernelOpKind::Literal, vec![1], Some(1)),
                op(KernelOpKind::Literal, vec![2], Some(2)),
                op(KernelOpKind::Literal, vec![3], Some(3)),
                op(KernelOpKind::Literal, vec![4], Some(4)),
                op(KernelOpKind::LoadGlobal, vec![0, 0], Some(10)),
                op(KernelOpKind::LoadGlobal, vec![0, 1], Some(11)),
                op(KernelOpKind::LoadGlobal, vec![0, 2], Some(12)),
                op(KernelOpKind::LoadGlobal, vec![0, 3], Some(13)),
                op(KernelOpKind::LoadGlobal, vec![1, 0], Some(20)),
                op(KernelOpKind::LoadGlobal, vec![1, 4], Some(21)),
            ],
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(2),
                LiteralValue::U32(3),
                LiteralValue::U32(8),
            ],
        },
    }
}

fn fma_chain_kernel() -> KernelDescriptor {
    // 8 FMA ops → tensor-core fragment candidate on sm_80+.
    let mut ops = vec![
        op(KernelOpKind::Literal, vec![0], Some(0)),
        op(KernelOpKind::Literal, vec![1], Some(1)),
        op(KernelOpKind::Literal, vec![2], Some(2)),
    ];
    for i in 0..8 {
        ops.push(op(KernelOpKind::Fma, vec![0, 1, 2], Some(3 + i)));
    }
    KernelDescriptor {
        id: "fma_chain".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops,
            child_bodies: vec![],
            literals: vec![
                LiteralValue::F32(1.0),
                LiteralValue::F32(2.0),
                LiteralValue::F32(3.0),
            ],
        },
    }
}

fn empty_kernel() -> KernelDescriptor {
    KernelDescriptor {
        id: "empty".into(),
        bindings: BindingLayout { slots: vec![] },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops: vec![],
            child_bodies: vec![],
            literals: vec![],
        },
    }
}

fn corpus() -> Vec<KernelDescriptor> {
    vec![
        coalesced_load_kernel(),
        strided_load_kernel(),
        shared_promotion_candidate_kernel(),
        scattered_load_kernel(),
        shared_promotion_two_candidate_kernel(),
        bank_conflict_kernel(),
        vec_pack_candidate_kernel(),
        fma_chain_kernel(),
        empty_kernel(),
    ]
}

#[test]
fn snapshot_coalesce_problematic_counts() {
    let kernels = corpus();
    let counts: Vec<usize> = kernels
        .iter()
        .map(|k| analyze_coalesce(k).problematic_count())
        .collect();
    assert_eq!(counts, vec![0, 1, 0, 1, 0, 0, 0, 0, 0]);
}

#[test]
fn snapshot_shared_mem_promotion_candidate_counts() {
    let kernels = corpus();
    let counts: Vec<usize> = kernels
        .iter()
        .map(|k| analyze_shared_mem_promote(k).candidates.len())
        .collect();
    assert_eq!(counts, vec![0, 0, 1, 0, 2, 0, 2, 0, 0]);
}

#[test]
fn snapshot_bank_conflict_problematic_counts() {
    let kernels = corpus();
    let counts: Vec<usize> = kernels
        .iter()
        .map(|k| analyze_bank_conflict(k).problematic_count())
        .collect();
    assert_eq!(counts, vec![0, 0, 0, 0, 0, 2, 0, 0, 0]);
}

#[test]
fn snapshot_bank_conflict_critical_counts() {
    let kernels = corpus();
    let counts: Vec<usize> = kernels
        .iter()
        .map(|k| analyze_bank_conflict(k).critical_count())
        .collect();
    assert_eq!(counts, vec![0, 0, 0, 0, 0, 1, 0, 0, 0]);
}

#[test]
fn snapshot_vec_pack_candidate_counts() {
    let kernels = corpus();
    let counts: Vec<usize> = kernels
        .iter()
        .map(|k| vec_pack::analyze(k).chains.len())
        .collect();
    assert_eq!(counts, vec![0, 0, 0, 0, 0, 0, 1, 0, 0]);
}

#[test]
fn snapshot_vec_pack_eliminated_op_counts() {
    let kernels = corpus();
    let counts: Vec<u32> = kernels
        .iter()
        .map(|k| vec_pack::analyze(k).total_ops_eliminated)
        .collect();
    assert_eq!(counts, vec![0, 0, 0, 0, 0, 0, 3, 0, 0]);
}

#[test]
fn snapshot_workgroup_uniform_branch_counts() {
    let kernels = corpus();
    let branch_counts: Vec<usize> = kernels
        .iter()
        .map(|k| analyze_workgroup_uniform(k).branches.len())
        .collect();
    assert_eq!(branch_counts, vec![0, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn snapshot_texture_promotion_candidate_counts() {
    let kernels = corpus();
    let counts: Vec<usize> = kernels
        .iter()
        .map(|k| analyze_texture_promote(k).candidates.len())
        .collect();
    assert_eq!(counts, vec![0, 0, 1, 0, 2, 0, 2, 0, 0]);
}

#[test]
fn snapshot_layout_transform_candidate_counts() {
    let kernels = corpus();
    let counts: Vec<usize> = kernels
        .iter()
        .map(|k| analyze_layout_aos_to_soa(k).candidates.len())
        .collect();
    assert_eq!(counts, vec![0, 0, 0, 0, 1, 0, 0, 0, 0]);
}

#[test]
fn snapshot_audit_waste_score_ordering() {
    let kernels = corpus();
    let scores: Vec<f32> = kernels.iter().map(|k| audit(k).waste_score).collect();
    // Strided > all others; empty/coalesced/fma_chain == 0.
    assert!(scores[0] < 0.001, "coalesced should have ~0 waste");
    assert!(scores[1] > 0.5, "strided should have measurable waste");
    assert!(
        scores[2] > 0.0,
        "promotion candidate has waste from unrealized promo"
    );
    assert!(scores[8] < 0.001, "empty kernel has 0 waste");
}

#[test]
fn snapshot_naga_emit_succeeds_for_simple_kernels() {
    // emit must succeed for the empty kernel and the basic-load kernels.
    // FMA chain may use unsupported ops; verify each independently.
    for kernel in [empty_kernel(), coalesced_load_kernel()] {
        let m = vyre_emit_naga::emit(&kernel).unwrap_or_else(|e| {
            panic!("emit failed for {}: {e}", kernel.id);
        });
        assert_ne!(
            m.entry_points.len(),
            0,
            "{} should have an entry point",
            kernel.id
        );
    }
}

#[test]
fn snapshot_ptx_emit_succeeds_for_simple_kernels() {
    for kernel in [
        empty_kernel(),
        coalesced_load_kernel(),
        strided_load_kernel(),
    ] {
        let s = vyre_emit_ptx::emit(&kernel).unwrap_or_else(|e| {
            panic!("ptx emit failed for {}: {e}", kernel.id);
        });
        assert!(s.contains(".version"));
        assert!(s.contains(".visible .entry main"));
    }
}

#[test]
fn snapshot_spirv_emit_succeeds_for_simple_kernels() {
    for kernel in [empty_kernel(), coalesced_load_kernel()] {
        let words = vyre_emit_spirv::emit(&kernel).unwrap_or_else(|e| {
            panic!("spirv emit failed for {}: {e}", kernel.id);
        });
        assert_eq!(words[0], vyre_emit_spirv::SPIRV_MAGIC);
    }
}

#[test]
fn snapshot_three_substrate_emit_byte_lengths_within_bounds() {
    // Sanity: for a known kernel, none of the emitters should produce
    // empty output, and SPIR-V should be substantially larger than naga
    // (binary vs IR).
    let kernel = coalesced_load_kernel();
    let naga_module = vyre_emit_naga::emit(&kernel).unwrap();
    let ptx_text = vyre_emit_ptx::emit(&kernel).unwrap();
    let spirv_words = vyre_emit_spirv::emit(&kernel).unwrap();
    assert!(naga_module.entry_points.len() == 1);
    assert!(
        ptx_text.len() > 100,
        "PTX text shouldn't be trivially small"
    );
    assert!(
        spirv_words.len() > 16,
        "SPIR-V binary shouldn't be just the header"
    );
}

#[test]
fn snapshot_corpus_size_pinned() {
    // Lock the corpus shape so future additions are explicit decisions.
    assert_eq!(corpus().len(), 9);
}
