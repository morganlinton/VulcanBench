use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::suite::SuiteKind;
use crate::cases::byte_pack::decode_u64_words;
use std::time::Instant;
use vyre::lower::analyses::{
    analyze_bank_conflict, analyze_coalesce, analyze_layout_aos_to_soa, analyze_shared_mem_promote,
    vec_pack,
};
use vyre::lower::rewrites::{self, OptimizationStats};
use vyre::lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};
use vyre_foundation::ir::{BinOp, DataType};

pub struct LowerRewriteImpact;

const SUITES: &[SuiteKind] = &[SuiteKind::Release, SuiteKind::Deep];
const WORKGROUP_SLOT_BASE: u32 = 1 << 24;

impl BenchCase for LowerRewriteImpact {
    fn id(&self) -> BenchId {
        BenchId("lower.rewrites.impact.corpus".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Lower Rewrite Impact Corpus".to_string(),
            description: "Measures descriptor rewrite impact on hand-built lower-analysis corpuses"
                .to_string(),
            tags: vec![
                "lower".to_string(),
                "rewrites".to_string(),
                "optimizer".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Backend,
            workload: WorkloadClass::Micro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-lower".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: false,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        None
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(corpus()))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        None
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let corpus = prepared
            .downcast_ref::<Vec<KernelDescriptor>>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "lower rewrite impact prepared payload type mismatch".to_string(),
                )
            })?;

        let baseline_start = Instant::now();
        let baseline_totals = analysis_totals(corpus.iter());
        let baseline_score = baseline_totals.issue_score();
        let baseline_ns = baseline_start.elapsed().as_nanos() as u64;

        let optimize_start = Instant::now();
        let mut stats = OptimizationStats::zero();
        let mut optimized = Vec::with_capacity(corpus.len());
        for desc in corpus {
            let (rewritten, current) = rewrites::run_all_with_stats(desc);
            stats.merge(current);
            optimized.push(rewritten);
        }
        let optimized_totals = analysis_totals(optimized.iter());
        let optimized_score = optimized_totals.issue_score();
        let optimize_ns = optimize_start.elapsed().as_nanos() as u64;

        let mut output = Vec::with_capacity(128);
        for value in [
            stats.ops_before as u64,
            stats.ops_after as u64,
            stats.ops_eliminated() as u64,
            stats.bindings_dropped() as u64,
            stats.off_graph_dropped() as u64,
            baseline_score,
            optimized_score,
            baseline_totals.coalesce_problematic,
            optimized_totals.coalesce_problematic,
            baseline_totals.shared_candidates,
            optimized_totals.shared_candidates,
            baseline_totals.bank_critical,
            optimized_totals.bank_critical,
            baseline_totals.vec_pack_chains,
            optimized_totals.vec_pack_chains,
            baseline_totals.layout_candidates,
            optimized_totals.layout_candidates,
        ] {
            output.extend_from_slice(&value.to_le_bytes());
        }

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(optimize_ns),
                optimize_ns: Some(optimize_ns),
                ir_nodes: Some(stats.ops_after as u64),
                custom: vec![
                    MetricPoint {
                        name: "lower_ops_before".to_string(),
                        value: stats.ops_before as u64,
                    },
                    MetricPoint {
                        name: "lower_ops_after".to_string(),
                        value: stats.ops_after as u64,
                    },
                    MetricPoint {
                        name: "lower_ops_eliminated".to_string(),
                        value: stats.ops_eliminated() as u64,
                    },
                    MetricPoint {
                        name: "lower_bindings_dropped".to_string(),
                        value: stats.bindings_dropped() as u64,
                    },
                    MetricPoint {
                        name: "lower_off_graph_dropped".to_string(),
                        value: stats.off_graph_dropped() as u64,
                    },
                    MetricPoint {
                        name: "lower_baseline_issue_score".to_string(),
                        value: baseline_score,
                    },
                    MetricPoint {
                        name: "lower_optimized_issue_score".to_string(),
                        value: optimized_score,
                    },
                    MetricPoint {
                        name: "lower_coalesce_problematic_before".to_string(),
                        value: baseline_totals.coalesce_problematic,
                    },
                    MetricPoint {
                        name: "lower_coalesce_problematic_after".to_string(),
                        value: optimized_totals.coalesce_problematic,
                    },
                    MetricPoint {
                        name: "lower_shared_candidates_before".to_string(),
                        value: baseline_totals.shared_candidates,
                    },
                    MetricPoint {
                        name: "lower_shared_candidates_after".to_string(),
                        value: optimized_totals.shared_candidates,
                    },
                    MetricPoint {
                        name: "lower_bank_critical_before".to_string(),
                        value: baseline_totals.bank_critical,
                    },
                    MetricPoint {
                        name: "lower_bank_critical_after".to_string(),
                        value: optimized_totals.bank_critical,
                    },
                    MetricPoint {
                        name: "lower_vec_pack_chains_before".to_string(),
                        value: baseline_totals.vec_pack_chains,
                    },
                    MetricPoint {
                        name: "lower_vec_pack_chains_after".to_string(),
                        value: optimized_totals.vec_pack_chains,
                    },
                    MetricPoint {
                        name: "lower_vec_pack_ops_eliminable_before".to_string(),
                        value: baseline_totals.vec_pack_ops_eliminable,
                    },
                    MetricPoint {
                        name: "lower_vec_pack_ops_eliminable_after".to_string(),
                        value: optimized_totals.vec_pack_ops_eliminable,
                    },
                    MetricPoint {
                        name: "lower_layout_candidates_before".to_string(),
                        value: baseline_totals.layout_candidates,
                    },
                    MetricPoint {
                        name: "lower_layout_candidates_after".to_string(),
                        value: optimized_totals.layout_candidates,
                    },
                    MetricPoint {
                        name: "lower_converged".to_string(),
                        value: u64::from(stats.converged),
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_ns),
                optimize_ns: Some(baseline_ns),
                ir_nodes: Some(stats.ops_before as u64),
                ..Default::default()
            }),
            outputs: vec![output.clone()],
            baseline_outputs: Some(vec![output]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        let output = run.outputs.first().ok_or_else(|| {
            BenchError::CorrectnessViolation(
                "lower rewrite impact produced no metric output".to_string(),
            )
        })?;
        let words = decode_u64_words(output, "lower rewrite")?;
        let ops_before = words[0];
        let ops_after = words[1];
        let ops_eliminated = words[2];
        let baseline_score = words[5];
        let optimized_score = words[6];
        let coalesce_before = words[7];
        let shared_before = words[9];
        let bank_before = words[11];
        let vec_pack_before = words[13];
        let layout_before = words[15];
        if ops_after > ops_before {
            return Err(BenchError::CorrectnessViolation(format!(
                "lower rewrite corpus grew top-level ops from {ops_before} to {ops_after}"
            )));
        }
        if ops_eliminated == 0 {
            return Err(BenchError::CorrectnessViolation(
                "lower rewrite corpus eliminated zero ops".to_string(),
            ));
        }
        if optimized_score > baseline_score {
            return Err(BenchError::CorrectnessViolation(format!(
                "lower rewrite corpus worsened issue score from {baseline_score} to {optimized_score}"
            )));
        }
        for (name, value) in [
            ("coalesce", coalesce_before),
            ("shared_mem_promote", shared_before),
            ("bank_conflict", bank_before),
            ("vec_pack", vec_pack_before),
            ("layout_aos_to_soa", layout_before),
        ] {
            if value == 0 {
                return Err(BenchError::CorrectnessViolation(format!(
                    "lower rewrite corpus lost {name} coverage; fixture count is zero"
                )));
            }
        }
        Ok(Correctness::Exact)
    }
}

#[derive(Default)]
struct AnalysisTotals {
    coalesce_problematic: u64,
    shared_candidates: u64,
    bank_critical: u64,
    vec_pack_chains: u64,
    vec_pack_ops_eliminable: u64,
    layout_candidates: u64,
}

impl AnalysisTotals {
    fn issue_score(&self) -> u64 {
        self.coalesce_problematic
            .saturating_add(self.shared_candidates)
            .saturating_add(self.bank_critical)
            .saturating_add(self.vec_pack_chains)
            .saturating_add(self.layout_candidates)
    }
}

fn analysis_totals<'a>(descs: impl IntoIterator<Item = &'a KernelDescriptor>) -> AnalysisTotals {
    let mut totals = AnalysisTotals::default();
    for desc in descs {
        let coalesce = analyze_coalesce(desc);
        let shared = analyze_shared_mem_promote(desc);
        let bank = analyze_bank_conflict(desc);
        let pack = vec_pack::analyze(desc);
        let layout = analyze_layout_aos_to_soa(desc);
        totals.coalesce_problematic = totals
            .coalesce_problematic
            .saturating_add(coalesce.problematic_count() as u64);
        totals.shared_candidates = totals
            .shared_candidates
            .saturating_add(shared.candidates.len() as u64);
        totals.bank_critical = totals
            .bank_critical
            .saturating_add(bank.critical_count() as u64);
        totals.vec_pack_chains = totals
            .vec_pack_chains
            .saturating_add(pack.chains.len() as u64);
        totals.vec_pack_ops_eliminable = totals
            .vec_pack_ops_eliminable
            .saturating_add(pack.total_ops_eliminated as u64);
        totals.layout_candidates = totals
            .layout_candidates
            .saturating_add(layout.candidates.len() as u64);
    }
    totals
}

fn corpus() -> Vec<KernelDescriptor> {
    vec![
        dce_corpus(),
        cse_corpus(),
        const_fold_corpus(),
        coalesce_corpus(),
        shared_mem_promote_corpus(),
        bank_conflict_corpus(),
        vec_pack_corpus(),
    ]
}

fn dce_corpus() -> KernelDescriptor {
    descriptor(
        "m_lower_dce",
        vec![global_slot(0, "out", BindingVisibility::ReadWrite)],
        vec![
            literal(0, 1),
            literal(1, 2),
            op(KernelOpKind::BinOpKind(BinOp::Add), vec![1, 2], Some(3)),
            op(KernelOpKind::StoreGlobal, vec![0, 1, 2], None),
        ],
        vec![LiteralValue::U32(0), LiteralValue::U32(1)],
    )
}

fn cse_corpus() -> KernelDescriptor {
    descriptor(
        "m_lower_cse",
        vec![global_slot(0, "out", BindingVisibility::ReadWrite)],
        vec![
            literal(0, 1),
            literal(1, 2),
            op(KernelOpKind::BinOpKind(BinOp::Add), vec![1, 2], Some(3)),
            op(KernelOpKind::BinOpKind(BinOp::Add), vec![1, 2], Some(4)),
            op(KernelOpKind::StoreGlobal, vec![0, 1, 4], None),
        ],
        vec![LiteralValue::U32(0), LiteralValue::U32(7)],
    )
}

fn const_fold_corpus() -> KernelDescriptor {
    descriptor(
        "m_lower_const_fold",
        vec![global_slot(0, "out", BindingVisibility::ReadWrite)],
        vec![
            literal(0, 1),
            literal(1, 2),
            literal(2, 3),
            op(KernelOpKind::BinOpKind(BinOp::Mul), vec![2, 3], Some(4)),
            op(KernelOpKind::StoreGlobal, vec![0, 1, 4], None),
        ],
        vec![
            LiteralValue::U32(0),
            LiteralValue::U32(8),
            LiteralValue::U32(4),
        ],
    )
}

fn bank_conflict_corpus() -> KernelDescriptor {
    descriptor(
        "m_lower_bank_conflict",
        vec![shared_slot(2, "tile")],
        vec![
            local_x(1),
            literal(0, 2),
            op(KernelOpKind::BinOpKind(BinOp::Mul), vec![1, 2], Some(3)),
            op(
                KernelOpKind::LoadShared,
                vec![WORKGROUP_SLOT_BASE + 2, 3],
                Some(4),
            ),
        ],
        vec![LiteralValue::U32(32)],
    )
}

fn coalesce_corpus() -> KernelDescriptor {
    descriptor(
        "m_lower_coalesce_strided",
        vec![global_slot(0, "input", BindingVisibility::ReadOnly)],
        vec![
            local_x(1),
            literal(0, 2),
            op(KernelOpKind::BinOpKind(BinOp::Mul), vec![1, 2], Some(3)),
            op(KernelOpKind::LoadGlobal, vec![0, 3], Some(4)),
        ],
        vec![LiteralValue::U32(4)],
    )
}

fn shared_mem_promote_corpus() -> KernelDescriptor {
    descriptor(
        "m_lower_shared_mem_promote",
        vec![
            global_slot(0, "hot_u32", BindingVisibility::ReadOnly),
            vec4_global_slot(1, "hot_vec4", BindingVisibility::ReadOnly),
        ],
        vec![
            literal(0, 1),
            op(KernelOpKind::LoadGlobal, vec![0, 1], Some(2)),
            op(KernelOpKind::LoadGlobal, vec![0, 1], Some(3)),
            op(KernelOpKind::LoadGlobal, vec![0, 1], Some(4)),
            op(KernelOpKind::LoadGlobal, vec![1, 1], Some(5)),
            op(KernelOpKind::LoadGlobal, vec![1, 1], Some(6)),
        ],
        vec![LiteralValue::U32(0)],
    )
}

fn vec_pack_corpus() -> KernelDescriptor {
    descriptor(
        "m_lower_vec_pack",
        vec![global_slot(0, "input", BindingVisibility::ReadOnly)],
        vec![
            literal(0, 1),
            literal(1, 2),
            literal(2, 3),
            literal(3, 4),
            op(KernelOpKind::LoadGlobal, vec![0, 1], Some(5)),
            op(KernelOpKind::LoadGlobal, vec![0, 2], Some(6)),
            op(KernelOpKind::LoadGlobal, vec![0, 3], Some(7)),
            op(KernelOpKind::LoadGlobal, vec![0, 4], Some(8)),
        ],
        vec![
            LiteralValue::U32(0),
            LiteralValue::U32(1),
            LiteralValue::U32(2),
            LiteralValue::U32(3),
        ],
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

fn vec4_global_slot(slot: u32, name: &str, visibility: BindingVisibility) -> BindingSlot {
    BindingSlot {
        slot,
        element_type: DataType::Vec4U32,
        element_count: Some(4096),
        memory_class: MemoryClass::Global,
        visibility,
        name: name.to_string(),
    }
}

fn shared_slot(slot: u32, name: &str) -> BindingSlot {
    BindingSlot {
        slot: WORKGROUP_SLOT_BASE + slot,
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

inventory::submit! {
    &LowerRewriteImpact as &'static dyn BenchCase
}
