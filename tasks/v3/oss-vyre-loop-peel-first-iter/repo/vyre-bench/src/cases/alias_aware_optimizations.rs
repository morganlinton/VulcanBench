use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::suite::SuiteKind;
use crate::cases::byte_pack::decode_u64_words;
use std::collections::BTreeSet;
use std::time::Instant;
use vyre_foundation::ir::DataType;
use vyre_lower::analyses::alias_facts::{AliasFactSet, NoAliasFact};
use vyre_lower::analyses::reaching_def_facts::{
    import_descriptor_reaching_defs, ReachingDefFactSet,
};
use vyre_lower::rewrites::{
    dead_store, dead_store_with_alias_facts, licm, licm_with_dataflow_facts, load_forwarding,
    load_forwarding_with_dataflow_facts, loop_fission, loop_fission_with_alias_facts, loop_fusion,
    loop_fusion_with_alias_facts,
};
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

pub struct AliasAwareOptimizations;

const SUITES: &[SuiteKind] = &[SuiteKind::Release, SuiteKind::Deep];
const REPEATS: usize = 2_048;

impl BenchCase for AliasAwareOptimizations {
    fn id(&self) -> BenchId {
        BenchId("lower.alias_aware_optimizations".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Alias-Aware Lower Optimizations".to_string(),
            description: "Compares conservative descriptor rewrites against dataflow fact-aware DSE, STLF, LICM, loop fusion, and loop fission".to_string(),
            tags: vec![
                "lower".to_string(),
                "dataflow".to_string(),
                "alias".to_string(),
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

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(PreparedAliasCorpus::new()))
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
            .downcast_ref::<PreparedAliasCorpus>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "alias-aware optimization prepared payload type mismatch".to_string(),
                )
            })?;

        let baseline_start = Instant::now();
        let mut baseline = conservative_eval(corpus);
        for _ in 1..REPEATS {
            baseline = conservative_eval(corpus);
        }
        let baseline_ns = baseline_start.elapsed().as_nanos() as u64;

        let alias_start = Instant::now();
        let mut alias_aware = alias_aware_eval(corpus);
        for _ in 1..REPEATS {
            alias_aware = alias_aware_eval(corpus);
        }
        let alias_ns = alias_start.elapsed().as_nanos() as u64;

        let pass_wins = pass_wins(&baseline, &alias_aware);
        let mut output = Vec::with_capacity(16 * 8);
        for value in [
            baseline.total_ops,
            alias_aware.total_ops,
            baseline.dse_stores,
            alias_aware.dse_stores,
            baseline.stlf_final_value_id,
            alias_aware.stlf_final_value_id,
            baseline.licm_loop_loads,
            alias_aware.licm_loop_loads,
            baseline.fusion_loops,
            alias_aware.fusion_loops,
            baseline.fission_loops,
            alias_aware.fission_loops,
            corpus.alias_facts.len() as u64,
            corpus.cross_binding_fact_count,
            corpus.reaching_fact_count,
            pass_wins,
        ] {
            output.extend_from_slice(&value.to_le_bytes());
        }
        let baseline_output = encode_eval(&baseline);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(alias_ns),
                optimize_ns: Some(alias_ns),
                ir_nodes: Some(alias_aware.total_ops),
                custom: vec![
                    MetricPoint {
                        name: "alias_pass_wins".to_string(),
                        value: pass_wins,
                    },
                    MetricPoint {
                        name: "alias_fact_count".to_string(),
                        value: corpus.alias_facts.len() as u64,
                    },
                    MetricPoint {
                        name: "alias_cross_binding_fact_count".to_string(),
                        value: corpus.cross_binding_fact_count,
                    },
                    MetricPoint {
                        name: "reaching_def_fact_count".to_string(),
                        value: corpus.reaching_fact_count,
                    },
                    MetricPoint {
                        name: "alias_total_ops_after".to_string(),
                        value: alias_aware.total_ops,
                    },
                    MetricPoint {
                        name: "conservative_total_ops_after".to_string(),
                        value: baseline.total_ops,
                    },
                    MetricPoint {
                        name: "alias_dse_store_count".to_string(),
                        value: alias_aware.dse_stores,
                    },
                    MetricPoint {
                        name: "conservative_dse_store_count".to_string(),
                        value: baseline.dse_stores,
                    },
                    MetricPoint {
                        name: "alias_stlf_final_value_id".to_string(),
                        value: alias_aware.stlf_final_value_id,
                    },
                    MetricPoint {
                        name: "conservative_stlf_final_value_id".to_string(),
                        value: baseline.stlf_final_value_id,
                    },
                    MetricPoint {
                        name: "alias_licm_loop_loads".to_string(),
                        value: alias_aware.licm_loop_loads,
                    },
                    MetricPoint {
                        name: "conservative_licm_loop_loads".to_string(),
                        value: baseline.licm_loop_loads,
                    },
                    MetricPoint {
                        name: "alias_fusion_loop_count".to_string(),
                        value: alias_aware.fusion_loops,
                    },
                    MetricPoint {
                        name: "conservative_fusion_loop_count".to_string(),
                        value: baseline.fusion_loops,
                    },
                    MetricPoint {
                        name: "alias_fission_loop_count".to_string(),
                        value: alias_aware.fission_loops,
                    },
                    MetricPoint {
                        name: "conservative_fission_loop_count".to_string(),
                        value: baseline.fission_loops,
                    },
                    MetricPoint {
                        name: "benchmark_repeats".to_string(),
                        value: REPEATS as u64,
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_ns),
                optimize_ns: Some(baseline_ns),
                ir_nodes: Some(baseline.total_ops),
                custom: vec![
                    MetricPoint {
                        name: "conservative_total_ops_after".to_string(),
                        value: baseline.total_ops,
                    },
                    MetricPoint {
                        name: "benchmark_repeats".to_string(),
                        value: REPEATS as u64,
                    },
                ],
                ..Default::default()
            }),
            outputs: vec![output],
            baseline_outputs: Some(vec![baseline_output]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        let output = run.outputs.first().ok_or_else(|| {
            BenchError::CorrectnessViolation(
                "alias-aware optimization benchmark produced no structural output".to_string(),
            )
        })?;
        let words = decode_u64_words(output, "alias-aware")?;
        if words.len() != 16 {
            return Err(BenchError::CorrectnessViolation(format!(
                "alias-aware optimization benchmark emitted {} metric words, expected 16",
                words.len()
            )));
        }
        let conservative_total_ops = words[0];
        let alias_total_ops = words[1];
        let conservative_dse_stores = words[2];
        let alias_dse_stores = words[3];
        let conservative_stlf_value = words[4];
        let alias_stlf_value = words[5];
        let conservative_licm_loads = words[6];
        let alias_licm_loads = words[7];
        let conservative_fusion_loops = words[8];
        let alias_fusion_loops = words[9];
        let conservative_fission_loops = words[10];
        let alias_fission_loops = words[11];
        let fact_count = words[12];
        let cross_binding_fact_count = words[13];
        let reaching_fact_count = words[14];
        let pass_wins = words[15];

        if fact_count == 0 {
            return Err(BenchError::CorrectnessViolation(
                "alias-aware optimization benchmark imported zero alias facts".to_string(),
            ));
        }
        if cross_binding_fact_count == 0 {
            return Err(BenchError::CorrectnessViolation(
                "alias-aware optimization benchmark imported zero cross-binding no-alias facts"
                    .to_string(),
            ));
        }
        if reaching_fact_count == 0 {
            return Err(BenchError::CorrectnessViolation(
                "alias-aware optimization benchmark imported zero reaching-def facts".to_string(),
            ));
        }
        if pass_wins < 5 {
            return Err(BenchError::CorrectnessViolation(format!(
                "alias-aware optimization benchmark proved only {pass_wins} pass win(s), expected all five pass families: dse {conservative_dse_stores}->{alias_dse_stores}, stlf value {conservative_stlf_value}->{alias_stlf_value}, licm loop loads {conservative_licm_loads}->{alias_licm_loads}, fusion loops {conservative_fusion_loops}->{alias_fusion_loops}, fission loops {conservative_fission_loops}->{alias_fission_loops}"
            )));
        }
        if alias_total_ops > conservative_total_ops {
            return Err(BenchError::CorrectnessViolation(format!(
                "alias-aware rewrites grew total descriptor ops from {conservative_total_ops} to {alias_total_ops}"
            )));
        }
        if alias_dse_stores >= conservative_dse_stores {
            return Err(BenchError::CorrectnessViolation(format!(
                "alias-aware DSE did not reduce stores: conservative={conservative_dse_stores}, alias={alias_dse_stores}"
            )));
        }
        if alias_stlf_value == conservative_stlf_value {
            return Err(BenchError::CorrectnessViolation(format!(
                "alias-aware STLF did not forward through a proven no-alias store; final value id stayed {alias_stlf_value}"
            )));
        }
        if alias_licm_loads >= conservative_licm_loads {
            return Err(BenchError::CorrectnessViolation(format!(
                "alias-aware LICM did not hoist loop load: conservative={conservative_licm_loads}, alias={alias_licm_loads}"
            )));
        }
        if alias_fusion_loops >= conservative_fusion_loops {
            return Err(BenchError::CorrectnessViolation(format!(
                "alias-aware fusion did not reduce loop count: conservative={conservative_fusion_loops}, alias={alias_fusion_loops}"
            )));
        }
        if alias_fission_loops <= conservative_fission_loops {
            return Err(BenchError::CorrectnessViolation(format!(
                "alias-aware fission did not split loop: conservative={conservative_fission_loops}, alias={alias_fission_loops}"
            )));
        }

        Ok(Correctness::Exact)
    }
}

#[derive(Debug, Clone)]
struct PreparedAliasCorpus {
    dse: KernelDescriptor,
    stlf: KernelDescriptor,
    licm: KernelDescriptor,
    fusion: KernelDescriptor,
    fission: KernelDescriptor,
    alias_facts: AliasFactSet,
    stlf_reaching_facts: ReachingDefFactSet,
    licm_reaching_facts: ReachingDefFactSet,
    cross_binding_fact_count: u64,
    reaching_fact_count: u64,
}

impl PreparedAliasCorpus {
    fn new() -> Self {
        let mut alias_facts = AliasFactSet::default();
        for fact in [
            NoAliasFact {
                left_binding: 0,
                left_index: 0,
                right_binding: 0,
                right_index: 1,
            },
            NoAliasFact {
                left_binding: 0,
                left_index: 0,
                right_binding: 1,
                right_index: 1,
            },
            NoAliasFact {
                left_binding: 0,
                left_index: 10,
                right_binding: 1,
                right_index: 11,
            },
            NoAliasFact {
                left_binding: 0,
                left_index: 10,
                right_binding: 1,
                right_index: 12,
            },
        ] {
            alias_facts.insert_no_alias(fact);
        }
        let dse = dse_descriptor();
        let stlf = stlf_descriptor();
        let licm = licm_descriptor();
        let fusion = fusion_descriptor();
        let fission = fission_descriptor();
        let stlf_reaching_facts = import_descriptor_reaching_defs(&stlf);
        let licm_reaching_facts = import_descriptor_reaching_defs(&licm);
        let reaching_fact_count = (stlf_reaching_facts.len() + licm_reaching_facts.len()) as u64;
        Self {
            dse,
            stlf,
            licm,
            fusion,
            fission,
            alias_facts,
            stlf_reaching_facts,
            licm_reaching_facts,
            cross_binding_fact_count: 3,
            reaching_fact_count,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct EvalSummary {
    total_ops: u64,
    dse_stores: u64,
    stlf_final_value_id: u64,
    licm_loop_loads: u64,
    fusion_loops: u64,
    fission_loops: u64,
}

fn conservative_eval(corpus: &PreparedAliasCorpus) -> EvalSummary {
    let dse = dead_store(&corpus.dse);
    let stlf = load_forwarding(&corpus.stlf);
    let licm_out = licm(&corpus.licm);
    let fusion = loop_fusion(&corpus.fusion);
    let fission = loop_fission(&corpus.fission);
    summarize(&dse, &stlf, &licm_out, &fusion, &fission)
}

fn alias_aware_eval(corpus: &PreparedAliasCorpus) -> EvalSummary {
    let dse = dead_store_with_alias_facts(&corpus.dse, &corpus.alias_facts);
    let stlf = load_forwarding_with_dataflow_facts(
        &corpus.stlf,
        &corpus.alias_facts,
        &corpus.stlf_reaching_facts,
    );
    let licm_out = licm_with_dataflow_facts(
        &corpus.licm,
        &corpus.alias_facts,
        &corpus.licm_reaching_facts,
    );
    let fusion = loop_fusion_with_alias_facts(&corpus.fusion, &corpus.alias_facts);
    let fission = loop_fission_with_alias_facts(&corpus.fission, &corpus.alias_facts);
    summarize(&dse, &stlf, &licm_out, &fusion, &fission)
}

fn summarize(
    dse: &KernelDescriptor,
    stlf: &KernelDescriptor,
    licm: &KernelDescriptor,
    fusion: &KernelDescriptor,
    fission: &KernelDescriptor,
) -> EvalSummary {
    EvalSummary {
        total_ops: total_ops(dse)
            + total_ops(stlf)
            + total_ops(licm)
            + total_ops(fusion)
            + total_ops(fission),
        dse_stores: count_kind(dse, |kind| matches!(kind, KernelOpKind::StoreGlobal)),
        stlf_final_value_id: final_store_value(stlf),
        licm_loop_loads: first_child_count(licm, |kind| matches!(kind, KernelOpKind::LoadGlobal)),
        fusion_loops: count_kind(fusion, |kind| {
            matches!(kind, KernelOpKind::StructuredForLoop { .. })
        }),
        fission_loops: count_kind(fission, |kind| {
            matches!(kind, KernelOpKind::StructuredForLoop { .. })
        }),
    }
}

fn pass_wins(baseline: &EvalSummary, alias_aware: &EvalSummary) -> u64 {
    u64::from(alias_aware.dse_stores < baseline.dse_stores)
        + u64::from(alias_aware.stlf_final_value_id != baseline.stlf_final_value_id)
        + u64::from(alias_aware.licm_loop_loads < baseline.licm_loop_loads)
        + u64::from(alias_aware.fusion_loops < baseline.fusion_loops)
        + u64::from(alias_aware.fission_loops > baseline.fission_loops)
}

fn encode_eval(eval: &EvalSummary) -> Vec<u8> {
    let mut output = Vec::with_capacity(6 * 8);
    for value in [
        eval.total_ops,
        eval.dse_stores,
        eval.stlf_final_value_id,
        eval.licm_loop_loads,
        eval.fusion_loops,
        eval.fission_loops,
    ] {
        output.extend_from_slice(&value.to_le_bytes());
    }
    output
}

fn total_ops(desc: &KernelDescriptor) -> u64 {
    reachable_body_ops(&desc.body)
}

fn reachable_body_ops(body: &KernelBody) -> u64 {
    let mut total = body.ops.len() as u64;
    let mut children = BTreeSet::new();
    for op in &body.ops {
        collect_referenced_children(op, &mut children);
    }
    for child in children {
        if let Some(body) = body.child_bodies.get(child as usize) {
            total = total.saturating_add(reachable_body_ops(body));
        }
    }
    total
}

fn collect_referenced_children(op: &KernelOp, out: &mut BTreeSet<u32>) {
    match &op.kind {
        KernelOpKind::StructuredIfThen if op.operands.len() >= 2 => {
            out.insert(op.operands[1]);
        }
        KernelOpKind::StructuredIfThenElse if op.operands.len() >= 3 => {
            out.insert(op.operands[1]);
            out.insert(op.operands[2]);
        }
        KernelOpKind::StructuredForLoop { .. } if op.operands.len() >= 3 => {
            out.insert(op.operands[2]);
        }
        KernelOpKind::StructuredBlock | KernelOpKind::Region { .. } => {
            out.extend(op.operands.iter().copied());
        }
        _ => {}
    }
}

fn count_kind(desc: &KernelDescriptor, predicate: impl Fn(&KernelOpKind) -> bool + Copy) -> u64 {
    count_body_kind(&desc.body, predicate)
}

fn count_body_kind(body: &KernelBody, predicate: impl Fn(&KernelOpKind) -> bool + Copy) -> u64 {
    body.ops.iter().filter(|op| predicate(&op.kind)).count() as u64
        + body
            .child_bodies
            .iter()
            .map(|child| count_body_kind(child, predicate))
            .sum::<u64>()
}

fn first_child_count(desc: &KernelDescriptor, predicate: impl Fn(&KernelOpKind) -> bool) -> u64 {
    desc.body
        .child_bodies
        .first()
        .map(|body| body.ops.iter().filter(|op| predicate(&op.kind)).count() as u64)
        .unwrap_or(0)
}

fn final_store_value(desc: &KernelDescriptor) -> u64 {
    desc.body
        .ops
        .iter()
        .rev()
        .find(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
        .and_then(|op| op.operands.get(2).copied())
        .unwrap_or(u32::MAX) as u64
}

fn dse_descriptor() -> KernelDescriptor {
    KernelDescriptor {
        id: "alias_dse".into(),
        bindings: rw_binding_layout(),
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::GlobalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(1),
                },
                literal_op(0, 2),
                literal_op(1, 3),
                store_op(0, 0, 2),
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![1, 1],
                    result: Some(4),
                },
                store_op(0, 0, 3),
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7), LiteralValue::U32(9)],
        },
    }
}

fn stlf_descriptor() -> KernelDescriptor {
    KernelDescriptor {
        id: "alias_stlf".into(),
        bindings: rw_binding_layout(),
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::GlobalInvocationId,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::LocalInvocationId,
                    operands: vec![0],
                    result: Some(1),
                },
                literal_op(0, 2),
                literal_op(1, 3),
                store_op(0, 0, 2),
                store_op(0, 1, 3),
                KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![0, 0],
                    result: Some(4),
                },
                store_op(0, 0, 4),
            ],
            child_bodies: vec![],
            literals: vec![LiteralValue::U32(7), LiteralValue::U32(9)],
        },
    }
}

fn licm_descriptor() -> KernelDescriptor {
    KernelDescriptor {
        id: "alias_licm".into(),
        bindings: ro_rw_binding_layout(),
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![literal_op(0, 0), literal_op(1, 1), loop_op("i", 0)],
            child_bodies: vec![KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::GlobalInvocationId,
                        operands: vec![0],
                        result: Some(10),
                    },
                    KernelOp {
                        kind: KernelOpKind::LocalInvocationId,
                        operands: vec![0],
                        result: Some(12),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 10],
                        result: Some(11),
                    },
                    store_op(1, 12, 11),
                ],
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(8)],
        },
    }
}

fn fusion_descriptor() -> KernelDescriptor {
    KernelDescriptor {
        id: "alias_fusion".into(),
        bindings: rw_binding_layout(),
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                literal_op(0, 0),
                literal_op(1, 1),
                loop_op("i", 0),
                loop_op("i", 1),
            ],
            child_bodies: vec![store_body(0, 10), store_body(1, 11)],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(8)],
        },
    }
}

fn fission_descriptor() -> KernelDescriptor {
    KernelDescriptor {
        id: "alias_fission".into(),
        bindings: rw_binding_layout(),
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![literal_op(0, 0), literal_op(1, 1), loop_op("i", 0)],
            child_bodies: vec![KernelBody {
                ops: vec![store_op(0, 10, 0), store_op(1, 11, 0)],
                child_bodies: vec![],
                literals: vec![],
            }],
            literals: vec![LiteralValue::U32(0), LiteralValue::U32(8)],
        },
    }
}

fn rw_binding_layout() -> BindingLayout {
    BindingLayout {
        slots: vec![rw_binding(0), rw_binding(1)],
    }
}

fn ro_rw_binding_layout() -> BindingLayout {
    let mut input = rw_binding(0);
    input.visibility = BindingVisibility::ReadOnly;
    input.name = "input".into();
    let mut scratch = rw_binding(1);
    scratch.name = "scratch".into();
    BindingLayout {
        slots: vec![input, scratch],
    }
}

fn rw_binding(slot: u32) -> BindingSlot {
    BindingSlot {
        slot,
        element_type: DataType::U32,
        element_count: None,
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: format!("buf{slot}"),
    }
}

fn literal_op(pool_index: u32, result: u32) -> KernelOp {
    KernelOp {
        kind: KernelOpKind::Literal,
        operands: vec![pool_index],
        result: Some(result),
    }
}

fn store_op(binding: u32, index: u32, value: u32) -> KernelOp {
    KernelOp {
        kind: KernelOpKind::StoreGlobal,
        operands: vec![binding, index, value],
        result: None,
    }
}

fn loop_op(loop_var: &str, body_index: u32) -> KernelOp {
    KernelOp {
        kind: KernelOpKind::StructuredForLoop {
            loop_var: loop_var.into(),
        },
        operands: vec![0, 1, body_index],
        result: None,
    }
}

fn store_body(binding: u32, index: u32) -> KernelBody {
    KernelBody {
        ops: vec![store_op(binding, index, 0)],
        child_bodies: vec![],
        literals: vec![],
    }
}

inventory::submit! {
    &AliasAwareOptimizations as &'static dyn BenchCase
}
