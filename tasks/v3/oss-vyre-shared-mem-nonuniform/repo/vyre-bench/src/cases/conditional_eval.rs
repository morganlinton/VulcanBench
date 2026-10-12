//! `conditions.yara_like.eval.1m`  -  branchy rule-condition evaluation.
//!
//! This is the release proof workload for vyre's core claim: evaluate a large
//! set of conventional rule conditions faster than an optimized CPU path.
//! The CPU baseline is deliberately ordinary and strong: Rayon parallelism plus
//! scalar short-circuiting over pattern match flags, counts, offsets, filesize,
//! and entropy-style metadata. The GPU path executes the same condition graph as
//! vyre IR, one invocation per rule.

use super::byte_pack::u32_bytes;
use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, transfer_accounting, u32_counter_reset_program,
    ResidentInputSet,
};
use crate::api::suite::SuiteKind;
use rayon::prelude::*;
use vyre_driver::{ResidentDispatchStep, ResidentReadRange};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const RULE_COUNT: u32 = 1 << 20;
const PATTERN_COUNT: u32 = 1 << 14;
const FILESIZE_BYTES: u32 = 10 * 1024 * 1024;
const ENTROPY_MILLIBITS: u32 = 712;
const FIRED_COUNT_RESOURCE_INDEX: usize = 12;
const FIRED_RULES_RESOURCE_INDEX: usize = 13;
const RESET_RESOURCE_INDICES: [usize; 1] = [FIRED_COUNT_RESOURCE_INDEX];
const CONDITIONAL_RESOURCE_INDICES: [usize; 14] = [
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    10,
    11,
    12,
    FIRED_RULES_RESOURCE_INDEX,
];

const HONEST_SUITES: &[SuiteKind] = &[
    SuiteKind::Honest,
    SuiteKind::Deep,
    SuiteKind::Release,
    SuiteKind::Smoke,
];

pub struct ConditionalEval;

struct ConditionalEvalPrepared {
    program: Program,
    reset_program: Program,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    baseline_output: Vec<Vec<u8>>,
    baseline_wall_ns: u64,
    resident: Option<ResidentInputSet>,
}

impl BenchCase for ConditionalEval {
    fn id(&self) -> BenchId {
        BenchId("conditions.yara_like.eval.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "YARA-like Conditional Eval 1M".to_string(),
            description: "Evaluate 1M branchy rule conditions over pattern flags, counts, offsets, filesize, and entropy metadata"
                .to_string(),
            tags: vec![
                "honest".to_string(),
                "conditions".to_string(),
                "rule-engine".to_string(),
                "cpu-favorable".to_string(),
                "dataflow-adjacent".to_string(),
            ],
            layer: BenchLayer::Honest,
            workload: WorkloadClass::Honest,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        HONEST_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: Some(u64::from(PATTERN_COUNT) * 12 + u64::from(RULE_COUNT) * 40 + 4),
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_100x(
            "YARA-like boolean rule-condition evaluation",
            "rayon",
            "Rayon-parallel scalar short-circuit rule loop",
        ))
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        let read = prepared
            .downcast_ref::<ConditionalEvalPrepared>()
            .map(|prepared| prepared.input_bytes_total)
            .unwrap_or_else(|| u64::from(PATTERN_COUNT) * 12 + u64::from(RULE_COUNT) * 36 + 8);
        let write = u64::from(RULE_COUNT) * 4;
        (read, write)
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("matched", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(PATTERN_COUNT),
                BufferDecl::storage("counts", 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(PATTERN_COUNT),
                BufferDecl::storage("offsets", 2, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(PATTERN_COUNT),
                BufferDecl::storage("rule_a", 3, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(RULE_COUNT),
                BufferDecl::storage("rule_b", 4, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(RULE_COUNT),
                BufferDecl::storage("rule_c", 5, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(RULE_COUNT),
                BufferDecl::storage("rule_d", 6, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(RULE_COUNT),
                BufferDecl::storage("min_count", 7, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(RULE_COUNT),
                BufferDecl::storage("max_offset", 8, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(RULE_COUNT),
                BufferDecl::storage("min_size", 9, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(RULE_COUNT),
                BufferDecl::storage("max_size", 10, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(RULE_COUNT),
                BufferDecl::storage("entropy_limit", 11, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(RULE_COUNT),
                BufferDecl::read_write("fired_count", 12, DataType::U32).with_count(1),
                BufferDecl::output("fired_rules", 13, DataType::U32).with_count(RULE_COUNT),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("tid", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("tid"), Expr::u32(RULE_COUNT)),
                    vec![
                        Node::let_bind("pa", Expr::load("rule_a", Expr::var("tid"))),
                        Node::let_bind("pb", Expr::load("rule_b", Expr::var("tid"))),
                        Node::let_bind("pc", Expr::load("rule_c", Expr::var("tid"))),
                        Node::let_bind("pd", Expr::load("rule_d", Expr::var("tid"))),
                        Node::let_bind(
                            "both_literals",
                            Expr::and(
                                Expr::ne(Expr::load("matched", Expr::var("pa")), Expr::u32(0)),
                                Expr::ne(Expr::load("matched", Expr::var("pb")), Expr::u32(0)),
                            ),
                        ),
                        Node::let_bind(
                            "count_ok",
                            Expr::ge(
                                Expr::load("counts", Expr::var("pc")),
                                Expr::load("min_count", Expr::var("tid")),
                            ),
                        ),
                        Node::let_bind(
                            "offset_ok",
                            Expr::le(
                                Expr::load("offsets", Expr::var("pd")),
                                Expr::load("max_offset", Expr::var("tid")),
                            ),
                        ),
                        Node::let_bind(
                            "size_ok",
                            Expr::and(
                                Expr::ge(
                                    Expr::u32(FILESIZE_BYTES),
                                    Expr::load("min_size", Expr::var("tid")),
                                ),
                                Expr::le(
                                    Expr::u32(FILESIZE_BYTES),
                                    Expr::load("max_size", Expr::var("tid")),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "entropy_ok",
                            Expr::le(
                                Expr::u32(ENTROPY_MILLIBITS),
                                Expr::load("entropy_limit", Expr::var("tid")),
                            ),
                        ),
                        Node::let_bind(
                            "fired",
                            Expr::and(
                                Expr::and(Expr::var("both_literals"), Expr::var("count_ok")),
                                Expr::and(
                                    Expr::var("offset_ok"),
                                    Expr::and(Expr::var("size_ok"), Expr::var("entropy_ok")),
                                ),
                            ),
                        ),
                        Node::if_then(
                            Expr::var("fired"),
                            vec![
                                Node::let_bind(
                                    "slot",
                                    Expr::atomic_add("fired_count", Expr::u32(0), Expr::u32(1)),
                                ),
                                Node::store("fired_rules", Expr::var("slot"), Expr::var("tid")),
                            ],
                        ),
                    ],
                ),
            ],
        );
        let reset_program = u32_counter_reset_program("fired_count");

        let matched: Vec<u32> = (0..PATTERN_COUNT)
            .map(|index| u32::from((mix32(index) & 7) != 0))
            .collect();
        let counts: Vec<u32> = (0..PATTERN_COUNT)
            .map(|index| (mix32(index ^ 0xA5A5_5A5A) & 7) + 1)
            .collect();
        let offsets: Vec<u32> = (0..PATTERN_COUNT)
            .map(|index| mix32(index ^ 0x517C_C1B7) % FILESIZE_BYTES)
            .collect();

        let mut rule_a = Vec::with_capacity(RULE_COUNT as usize);
        let mut rule_b = Vec::with_capacity(RULE_COUNT as usize);
        let mut rule_c = Vec::with_capacity(RULE_COUNT as usize);
        let mut rule_d = Vec::with_capacity(RULE_COUNT as usize);
        let mut min_count = Vec::with_capacity(RULE_COUNT as usize);
        let mut max_offset = Vec::with_capacity(RULE_COUNT as usize);
        let mut min_size = Vec::with_capacity(RULE_COUNT as usize);
        let mut max_size = Vec::with_capacity(RULE_COUNT as usize);
        let mut entropy_limit = Vec::with_capacity(RULE_COUNT as usize);

        for rule in 0..RULE_COUNT {
            let seed = mix32(rule);
            rule_a.push(seed & (PATTERN_COUNT - 1));
            rule_b.push(mix32(seed ^ 0x9E37_79B9) & (PATTERN_COUNT - 1));
            rule_c.push(mix32(seed ^ 0x85EB_CA6B) & (PATTERN_COUNT - 1));
            rule_d.push(mix32(seed ^ 0xC2B2_AE35) & (PATTERN_COUNT - 1));
            min_count.push((seed >> 5) % 7 + 1);
            max_offset.push(FILESIZE_BYTES - ((seed >> 11) % (FILESIZE_BYTES / 2)));
            min_size.push(FILESIZE_BYTES - ((seed >> 17) & 4095));
            max_size.push(FILESIZE_BYTES + ((seed >> 3) & 8191));
            entropy_limit.push(600 + ((seed >> 9) % 320));
        }

        let inputs = vec![
            u32_bytes(&matched),
            u32_bytes(&counts),
            u32_bytes(&offsets),
            u32_bytes(&rule_a),
            u32_bytes(&rule_b),
            u32_bytes(&rule_c),
            u32_bytes(&rule_d),
            u32_bytes(&min_count),
            u32_bytes(&max_offset),
            u32_bytes(&min_size),
            u32_bytes(&max_size),
            u32_bytes(&entropy_limit),
        ];
        let input_bytes_total = input_bytes_total(&inputs);

        let resident = ResidentInputSet::upload_with_zeroed_outputs_optional(
            ctx,
            &inputs,
            &[4, RULE_COUNT as usize * 4],
            "conditional eval bench",
        )?;

        let baseline_start = std::time::Instant::now();
        let baseline_output = cpu_conditional_eval_raw(
            &matched,
            &counts,
            &offsets,
            &rule_a,
            &rule_b,
            &rule_c,
            &rule_d,
            &min_count,
            &max_offset,
            &min_size,
            &max_size,
            &entropy_limit,
        );
        let baseline_wall_ns = baseline_start.elapsed().as_nanos() as u64;

        Ok(Box::new(ConditionalEvalPrepared {
            program,
            reset_program,
            inputs,
            input_bytes_total,
            baseline_output,
            baseline_wall_ns,
            resident,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<ConditionalEvalPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<ConditionalEvalPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "conditional eval prepared payload type mismatch".to_string(),
                )
            })?;

        let (outputs, wall_ns, dispatch_ns, resident_used, device_reset_sequence) =
            if let Some(resident) = &prepared.resident {
                let sequence = dispatch_resident_conditional_sequence(ctx, prepared, resident)?;
                (
                    sequence.outputs,
                    sequence.wall_ns,
                    sequence.dispatch_ns,
                    true,
                    true,
                )
            } else {
                let dispatch = dispatch_program_timed(
                    ctx,
                    &prepared.program,
                    None,
                    &prepared.inputs,
                    &ctx.dispatch_config,
                )?;
                let timed = dispatch.timed;
                (
                    timed.outputs,
                    timed.wall_ns,
                    timed.device_ns,
                    dispatch.resident_used,
                    false,
                )
            };
        let input_bytes = prepared.input_bytes_total;
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = transfer_accounting(input_bytes, output_bytes, resident_used);
        let resident_reset_bytes = 0;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns),
                dispatch_ns,
                input_bytes: Some(input_bytes),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                custom: conditional_eval_metric_points(
                    resident_used,
                    device_reset_sequence,
                    resident_reset_bytes,
                ),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(input_bytes),
                output_bytes: Some(
                    prepared.baseline_output.iter().map(Vec::len).sum::<usize>() as u64
                ),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: ctx
                .include_baseline_outputs
                .then(|| prepared.baseline_output.clone()),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        verify_sparse_outputs(&run.outputs, run.baseline_outputs.as_deref())
    }
}

fn verify_sparse_outputs(
    outputs: &[Vec<u8>],
    baseline_outputs: Option<&[Vec<u8>]>,
) -> Result<Correctness, BenchError> {
    let baseline = baseline_outputs.ok_or_else(|| {
        BenchError::CorrectnessViolation(
            "conditional eval did not capture baseline sparse fired-rule output".to_string(),
        )
    })?;
    if outputs.len() != 2 || baseline.len() != 2 {
        return Err(BenchError::CorrectnessViolation(format!(
            "sparse output count mismatch: backend returned {}, baseline returned {}",
            outputs.len(),
            baseline.len()
        )));
    }
    let backend_count = read_le_u32(&outputs[0], 0)? as usize;
    let baseline_count = read_le_u32(&baseline[0], 0)? as usize;
    if backend_count != baseline_count {
        return Err(BenchError::CorrectnessViolation(format!(
            "fired-rule count mismatch: backend returned {backend_count}, baseline returned {baseline_count}"
        )));
    }
    if outputs[1].len() < backend_count.saturating_mul(4)
        || baseline[1].len() < baseline_count.saturating_mul(4)
    {
        return Err(BenchError::CorrectnessViolation(
            "fired-rule output buffer shorter than reported count".to_string(),
        ));
    }
    let mut backend_rules = read_u32_prefix(&outputs[1], backend_count)?;
    let mut baseline_rules = read_u32_prefix(&baseline[1], baseline_count)?;
    backend_rules.sort_unstable();
    baseline_rules.sort_unstable();
    if backend_rules == baseline_rules {
        Ok(Correctness::Exact)
    } else {
        Err(BenchError::CorrectnessViolation(
            "fired-rule set differs between backend and baseline".to_string(),
        ))
    }
}

fn read_le_u32(bytes: &[u8], word_index: usize) -> Result<u32, BenchError> {
    vyre_primitives::wire::read_u32_le_word(bytes, word_index, "conditional-eval output")
        .map_err(BenchError::CorrectnessViolation)
}

fn read_u32_prefix(bytes: &[u8], count: usize) -> Result<Vec<u32>, BenchError> {
    (0..count).map(|index| read_le_u32(bytes, index)).collect()
}

#[allow(clippy::too_many_arguments)]
fn cpu_conditional_eval_raw(
    matched: &[u32],
    counts: &[u32],
    offsets: &[u32],
    rule_a: &[u32],
    rule_b: &[u32],
    rule_c: &[u32],
    rule_d: &[u32],
    min_count: &[u32],
    max_offset: &[u32],
    min_size: &[u32],
    max_size: &[u32],
    entropy_limit: &[u32],
) -> Vec<Vec<u8>> {
    let mut fired_rules: Vec<u32> = (0..RULE_COUNT as usize)
        .into_par_iter()
        .map(|rule| {
            if matched[rule_a[rule] as usize] == 0 {
                return None;
            }
            if matched[rule_b[rule] as usize] == 0 {
                return None;
            }
            if counts[rule_c[rule] as usize] < min_count[rule] {
                return None;
            }
            if offsets[rule_d[rule] as usize] > max_offset[rule] {
                return None;
            }
            if FILESIZE_BYTES < min_size[rule] || FILESIZE_BYTES > max_size[rule] {
                return None;
            }
            if ENTROPY_MILLIBITS > entropy_limit[rule] {
                return None;
            }
            Some(rule as u32)
        })
        .flatten()
        .collect();
    fired_rules.sort_unstable();
    let count = fired_rules.len() as u32;
    fired_rules.resize(RULE_COUNT as usize, 0);
    vec![u32_bytes(&[count]), u32_bytes(&fired_rules)]
}

struct ConditionalResidentSequenceRun {
    outputs: Vec<Vec<u8>>,
    wall_ns: u64,
    dispatch_ns: Option<u64>,
}

fn dispatch_resident_conditional_sequence(
    ctx: &BenchContext,
    prepared: &ConditionalEvalPrepared,
    resident: &ResidentInputSet,
) -> Result<ConditionalResidentSequenceRun, BenchError> {
    let workgroup = prepared.program.workgroup_size();
    if let Some(override_workgroup) = ctx.dispatch_config.workgroup_override {
        if override_workgroup != workgroup {
            return Err(BenchError::ExecutionFailed(format!(
                "conditional eval resident sequence uses program workgroup {:?}, but received override {:?}. Fix: run the resident condition sequence without a workgroup override or rebuild the resident sequence program.",
                workgroup, override_workgroup
            )));
        }
    }

    let reset_resources = resident
        .resources_for_indices(&RESET_RESOURCE_INDICES, "conditional eval reset sequence")?;
    let conditional_resources = resident.resources_for_indices(
        &CONDITIONAL_RESOURCE_INDICES,
        "conditional eval resident sequence",
    )?;
    let reset_step = ResidentDispatchStep {
        program: &prepared.reset_program,
        resources: &reset_resources,
        grid_override: Some([1, 1, 1]),
        workgroup_override: None,
    };
    let conditional_step = ResidentDispatchStep {
        program: &prepared.program,
        resources: &conditional_resources,
        grid_override: Some([RULE_COUNT.div_ceil(workgroup[0]).max(1), 1, 1]),
        workgroup_override: None,
    };
    let read_ranges = [
        ResidentReadRange {
            resource: &conditional_resources[FIRED_COUNT_RESOURCE_INDEX],
            byte_offset: 0,
            byte_len: prepared.baseline_output[0].len(),
        },
        ResidentReadRange {
            resource: &conditional_resources[FIRED_RULES_RESOURCE_INDEX],
            byte_offset: 0,
            byte_len: prepared.baseline_output[1].len(),
        },
    ];

    let mut count_output = Vec::with_capacity(prepared.baseline_output[0].len());
    let mut rules_output = Vec::with_capacity(prepared.baseline_output[1].len());
    let timing = ctx
        .preferred_backend
        .dispatch_resident_sequence_read_ranges_timed_into(
            &[reset_step, conditional_step],
            &read_ranges,
            &mut [&mut count_output, &mut rules_output],
        )
        .map_err(|error| BenchError::BackendFailed(error.to_string()))?;

    Ok(ConditionalResidentSequenceRun {
        outputs: vec![count_output, rules_output],
        wall_ns: timing.wall_ns,
        dispatch_ns: timing.device_ns,
    })
}

fn conditional_eval_metric_points(
    resident_used: bool,
    device_reset_sequence: bool,
    resident_reset_bytes: u64,
) -> Vec<MetricPoint> {
    vec![
        MetricPoint {
            name: "conditional_eval_resident_buffers".to_string(),
            value: u64::from(resident_used),
        },
        MetricPoint {
            name: "conditional_eval_device_reset_sequence".to_string(),
            value: u64::from(device_reset_sequence),
        },
        MetricPoint {
            name: "conditional_eval_resident_reset_bytes".to_string(),
            value: resident_reset_bytes,
        },
    ]
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

inventory::submit! {
    &ConditionalEval as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resident_sequence_indices_keep_sparse_outputs_in_binding_order() {
        assert_eq!(
            CONDITIONAL_RESOURCE_INDICES[FIRED_COUNT_RESOURCE_INDEX],
            FIRED_COUNT_RESOURCE_INDEX
        );
        assert_eq!(
            CONDITIONAL_RESOURCE_INDICES[FIRED_RULES_RESOURCE_INDEX],
            FIRED_RULES_RESOURCE_INDEX
        );
        assert_eq!(RESET_RESOURCE_INDICES, [FIRED_COUNT_RESOURCE_INDEX]);
    }

    #[test]
    fn metric_points_expose_device_reset_and_zero_host_reset_bytes() {
        let metrics = conditional_eval_metric_points(true, true, 0);

        assert_eq!(
            metrics
                .iter()
                .find(|metric| metric.name == "conditional_eval_resident_buffers")
                .map(|metric| metric.value),
            Some(1)
        );
        assert_eq!(
            metrics
                .iter()
                .find(|metric| metric.name == "conditional_eval_device_reset_sequence")
                .map(|metric| metric.value),
            Some(1)
        );
        assert_eq!(
            metrics
                .iter()
                .find(|metric| metric.name == "conditional_eval_resident_reset_bytes")
                .map(|metric| metric.value),
            Some(0)
        );
    }
}
