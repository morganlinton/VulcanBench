//! `nn.linear_4bit_affine_grouped.1m` - fused grouped INT4 linear inference.
//!
//! This measures the path LLM inference actually needs: packed 4-bit weights,
//! per-group scale/zero-point metadata, and accumulation without materializing
//! an unpacked weight matrix. The CPU oracle performs the same packed read and
//! affine dequantization loop so the benchmark measures dispatch/runtime
//! advantage rather than changing math.

use super::byte_pack::{f32_bytes, u32_bytes};
use super::release_workloads::{
    validate_release_math_nn_kernel_evidence, ReleaseMathNnKernelEvidence,
};
use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::resident::{
    dispatch_compiled_timed, input_bytes_total, resident_output_byte_lengths, transfer_accounting,
    ResidentInputPool,
};
use crate::api::suite::SuiteKind;
use rayon::prelude::*;
use std::sync::Arc;
use vyre_driver::CompiledPipeline;
use vyre_foundation::ir::Program;

const IN_DIM: u32 = 256;
const OUT_DIM: u32 = 4096;
const GROUP_SIZE: u32 = 64;
const PACKED_WORDS: u32 = (IN_DIM / 8) * OUT_DIM;
const GROUP_COUNT: u32 = (IN_DIM + GROUP_SIZE - 1) / GROUP_SIZE;
const SIDECAR_WORDS: u32 = GROUP_COUNT * OUT_DIM;
const MAC_COUNT: u64 = (IN_DIM as u64) * (OUT_DIM as u64);
const CPU_BASELINE_SAMPLES: usize = 9;
const RESIDENT_SAMPLE_SETS: usize = 8;

const SUITES: &[SuiteKind] = &[
    SuiteKind::Release,
    SuiteKind::Gpu,
    SuiteKind::Deep,
    SuiteKind::Honest,
];

fn digest64(bytes: &[u8]) -> u64 {
    let hash = blake3::hash(bytes);
    let mut digest = [0u8; 8];
    digest.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(digest)
}

fn kernel_path_id(path: &str) -> u64 {
    match path {
        "cooperative" => 1,
        "tensor_core_f16_m16n8k16" => 2,
        "tensor_core_bf16_m16n8k16" => 3,
        "tensor_core_tf32_m16n8k4" => 4,
        _ => 0,
    }
}

pub struct QuantizedLinear4BitAffineGrouped;

struct QuantizedLinearPrepared {
    program: Program,
    evidence: vyre_libs::nn::QuantizedLinear4BitPlannerEvidence,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    baseline_output: Vec<u8>,
    baseline_wall_ns: u64,
    compiled: Option<Arc<dyn CompiledPipeline>>,
    resident: Option<ResidentInputPool>,
}

impl BenchCase for QuantizedLinear4BitAffineGrouped {
    fn id(&self) -> BenchId {
        BenchId("nn.linear_4bit_affine_grouped.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Fused grouped INT4 linear 1M MAC".to_string(),
            description:
                "Packed 4-bit linear layer with per-group scale/zero-point applied in the dot-product loop"
                    .to_string(),
            tags: vec![
                "nn".to_string(),
                "quantized".to_string(),
                "resident".to_string(),
                "inference".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Runtime,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: Some(
                u64::from(PACKED_WORDS + SIDECAR_WORDS * 2 + IN_DIM + OUT_DIM * 2) * 4,
            ),
            min_input_bytes: Some(
                u64::from(PACKED_WORDS + SIDECAR_WORDS * 2 + IN_DIM + OUT_DIM) * 4,
            ),
            feature_set: vec![
                "nn.quantized".to_string(),
                "resident".to_string(),
                "packed-int4".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_100x(
            "fused grouped INT4 linear",
            "rayon",
            "Rayon-parallel packed INT4 affine dequantization oracle",
        ))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let spec =
            vyre_libs::nn::QuantizedLinear4BitSpec::affine_grouped(IN_DIM, OUT_DIM, GROUP_SIZE);
        let evidence = vyre_libs::nn::linear_4bit_affine_grouped_planner_evidence(&spec)
            .map_err(BenchError::ExecutionFailed)?;
        let program = vyre_libs::nn::linear_4bit_affine_grouped_typed(
            &spec, "x", "w", "scale", "zp", "b", "out",
        )
        .map_err(BenchError::ExecutionFailed)?;
        ctx.dispatch_config
            .workgroup_override
            .get_or_insert(program.workgroup_size());
        let (x, packed, scale, zero_point, bias) = quantized_inputs();
        let inputs = vec![
            f32_bytes(&x),
            u32_bytes(&packed),
            f32_bytes(&scale),
            u32_bytes(&zero_point),
            f32_bytes(&bias),
        ];
        let input_bytes_total = input_bytes_total(&inputs);
        let (baseline, baseline_wall_ns) =
            measured_cpu_oracle_checked(&x, &packed, &scale, &zero_point, &bias)?;
        let baseline_output = f32_bytes(&baseline);
        let resident_output_sizes = resident_output_byte_lengths(&program, "quantized linear")?;
        let resident = ResidentInputPool::upload_with_zeroed_outputs_optional(
            ctx,
            &inputs,
            &resident_output_sizes,
            RESIDENT_SAMPLE_SETS,
            "quantized linear",
        )?;

        Ok(Box::new(QuantizedLinearPrepared {
            program,
            evidence,
            inputs,
            input_bytes_total,
            baseline_output,
            baseline_wall_ns,
            compiled: None,
            resident,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<QuantizedLinearPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_mut::<QuantizedLinearPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "quantized linear prepared payload type mismatch".to_string(),
                )
            })?;

        if prepared.compiled.is_none() {
            let compiled = vyre_driver::pipeline::compile_with_telemetry(
                Arc::clone(&ctx.preferred_backend),
                &prepared.program,
                &ctx.dispatch_config,
            )
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?
            .pipeline;
            prepared.compiled = Some(compiled);
        }
        let compiled = prepared.compiled.as_ref().ok_or_else(|| {
            BenchError::ExecutionFailed(
                "quantized linear compiled pipeline missing after compile".to_string(),
            )
        })?;

        let dispatch = dispatch_compiled_timed(
            compiled.as_ref(),
            prepared.resident.as_mut(),
            &prepared.inputs,
            &ctx.dispatch_config,
        )?;
        let timed = dispatch.timed;
        let outputs = timed.outputs;
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = transfer_accounting(
            prepared.input_bytes_total,
            output_bytes,
            dispatch.resident_used,
        );
        let cpu_digest = digest64(&prepared.baseline_output);
        let gpu_digest = outputs.first().map_or(0, |output| digest64(output));
        let tolerance_abs_e9 =
            (prepared.evidence.output_drift_abs_tolerance * 1.0e9) as u64;
        let release_evidence = ReleaseMathNnKernelEvidence {
            case_id: "nn.linear_4bit_affine_grouped.1m",
            cpu_digest,
            gpu_digest,
            tolerance_abs_e9,
            active_time_ns: timed.device_ns.unwrap_or(timed.wall_ns),
            transfer_bytes: accounting.bytes_touched,
            selected_kernel_path: prepared.evidence.matmul_selected_path,
        };
        validate_release_math_nn_kernel_evidence(&release_evidence)
            .map_err(BenchError::ExecutionFailed)?;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                custom: vec![
                    MetricPoint {
                        name: "quantized_mac_count".to_string(),
                        value: MAC_COUNT,
                    },
                    MetricPoint {
                        name: "quantized_group_size".to_string(),
                        value: u64::from(prepared.evidence.group_size),
                    },
                    MetricPoint {
                        name: "quantized_sidecar_groups".to_string(),
                        value: u64::from(prepared.evidence.group_count),
                    },
                    MetricPoint {
                        name: "packed_weight_bytes".to_string(),
                        value: prepared.evidence.packed_weight_bytes,
                    },
                    MetricPoint {
                        name: "unpacked_weight_bytes_elided".to_string(),
                        value: prepared.evidence.dequant_bytes_elided,
                    },
                    MetricPoint {
                        name: "dequantized_weight_bytes".to_string(),
                        value: prepared.evidence.dequantized_weight_bytes,
                    },
                    MetricPoint {
                        name: "matmul_planner_m".to_string(),
                        value: u64::from(prepared.evidence.matmul_m),
                    },
                    MetricPoint {
                        name: "matmul_planner_k".to_string(),
                        value: u64::from(prepared.evidence.matmul_k),
                    },
                    MetricPoint {
                        name: "matmul_planner_n".to_string(),
                        value: u64::from(prepared.evidence.matmul_n),
                    },
                    MetricPoint {
                        name: "matmul_planner_tile".to_string(),
                        value: u64::from(prepared.evidence.matmul_tile),
                    },
                    MetricPoint {
                        name: "matmul_planner_tensor_core_eligible".to_string(),
                        value: if prepared.evidence.tensor_core_eligible {
                            1
                        } else {
                            0
                        },
                    },
                    MetricPoint {
                        name: "output_drift_abs_tolerance_e9".to_string(),
                        value: release_evidence.tolerance_abs_e9,
                    },
                    MetricPoint {
                        name: "release_math_nn_cpu_digest".to_string(),
                        value: release_evidence.cpu_digest,
                    },
                    MetricPoint {
                        name: "release_math_nn_gpu_digest".to_string(),
                        value: release_evidence.gpu_digest,
                    },
                    MetricPoint {
                        name: "release_math_nn_active_time_ns".to_string(),
                        value: release_evidence.active_time_ns,
                    },
                    MetricPoint {
                        name: "release_math_nn_transfer_bytes".to_string(),
                        value: release_evidence.transfer_bytes,
                    },
                    MetricPoint {
                        name: "matmul_planner_selected_path_id".to_string(),
                        value: kernel_path_id(release_evidence.selected_kernel_path),
                    },
                    MetricPoint {
                        name: "weight_compression_ratio_x100".to_string(),
                        value: (u64::from(IN_DIM) * u64::from(OUT_DIM) * 4 * 100)
                            / (u64::from(PACKED_WORDS) * 4),
                    },
                    MetricPoint {
                        name: "sidecar_loads_elided_by_group_hoist".to_string(),
                        value: MAC_COUNT
                            .saturating_mul(2)
                            .saturating_sub(u64::from(OUT_DIM) * u64::from(GROUP_COUNT) * 2),
                    },
                    MetricPoint {
                        name: "dequantize_dispatches_elided".to_string(),
                        value: 1,
                    },
                    MetricPoint {
                        name: "resident_buffers".to_string(),
                        value: u64::from(dispatch.resident_used),
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(prepared.baseline_output.len() as u64),
                bytes_touched: Some(
                    prepared
                        .input_bytes_total
                        .saturating_add(prepared.baseline_output.len() as u64),
                ),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(vec![prepared.baseline_output.clone()]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_f32_outputs_with_ulp(8)
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<QuantizedLinearPrepared>()
            .map(|prepared| (prepared.input_bytes_total, u64::from(OUT_DIM) * 4))
            .unwrap_or((
                u64::from(PACKED_WORDS + SIDECAR_WORDS * 2 + IN_DIM + OUT_DIM) * 4,
                u64::from(OUT_DIM) * 4,
            ))
    }
}

fn quantized_inputs() -> (Vec<f32>, Vec<u32>, Vec<f32>, Vec<u32>, Vec<f32>) {
    let x = (0..IN_DIM).map(|k| (k % 17) as f32).collect::<Vec<_>>();
    let mut packed = vec![0u32; PACKED_WORDS as usize];
    for block in 0..(IN_DIM / 8) {
        for out in 0..OUT_DIM {
            let mut word = 0u32;
            for lane in 0..8 {
                let k = block * 8 + lane;
                let nibble = (k
                    .wrapping_mul(3)
                    .wrapping_add(out.wrapping_mul(5))
                    .wrapping_add(7))
                    & 0xF;
                word |= nibble << (lane * 4);
            }
            packed[(block * OUT_DIM + out) as usize] = word;
        }
    }

    let mut scale = vec![0.0f32; SIDECAR_WORDS as usize];
    let mut zero_point = vec![0u32; SIDECAR_WORDS as usize];
    for group in 0..GROUP_COUNT {
        for out in 0..OUT_DIM {
            let idx = (group * OUT_DIM + out) as usize;
            scale[idx] = match (group + out) & 3 {
                0 => 0.25,
                1 => 0.5,
                2 => 1.0,
                _ => 2.0,
            };
            zero_point[idx] = (group.wrapping_mul(3).wrapping_add(out)) & 0x7;
        }
    }

    let bias = (0..OUT_DIM).map(|out| (out & 7) as f32).collect::<Vec<_>>();
    (x, packed, scale, zero_point, bias)
}

fn cpu_oracle_checked(
    x: &[f32],
    packed: &[u32],
    scale: &[f32],
    zero_point: &[u32],
    bias: &[f32],
) -> Result<Vec<f32>, BenchError> {
    if x.len() != IN_DIM as usize {
        return Err(BenchError::ExecutionFailed(format!(
            "quantized linear x length mismatch: got {}, expected {IN_DIM}",
            x.len()
        )));
    }
    if packed.len() != PACKED_WORDS as usize {
        return Err(BenchError::ExecutionFailed(format!(
            "quantized linear packed weight length mismatch: got {}, expected {PACKED_WORDS}",
            packed.len()
        )));
    }
    if scale.len() != SIDECAR_WORDS as usize || zero_point.len() != SIDECAR_WORDS as usize {
        return Err(BenchError::ExecutionFailed(format!(
            "quantized linear sidecar length mismatch: scale={}, zero_point={}, expected {SIDECAR_WORDS}",
            scale.len(),
            zero_point.len()
        )));
    }
    if bias.len() != OUT_DIM as usize {
        return Err(BenchError::ExecutionFailed(format!(
            "quantized linear bias length mismatch: got {}, expected {OUT_DIM}",
            bias.len()
        )));
    }

    Ok((0..OUT_DIM as usize)
        .into_par_iter()
        .map(|out| {
            let mut acc = bias[out];
            for k in 0..IN_DIM as usize {
                let block = k / 8;
                let shift = (k % 8) * 4;
                let word = packed[block * OUT_DIM as usize + out];
                let nibble = ((word >> shift) & 0xF) as f32;
                let group = k / GROUP_SIZE as usize;
                let sidecar_idx = group * OUT_DIM as usize + out;
                let weight = (nibble - zero_point[sidecar_idx] as f32) * scale[sidecar_idx];
                acc += x[k] * weight;
            }
            acc
        })
        .collect())
}

fn measured_cpu_oracle_checked(
    x: &[f32],
    packed: &[u32],
    scale: &[f32],
    zero_point: &[u32],
    bias: &[f32],
) -> Result<(Vec<f32>, u64), BenchError> {
    let mut durations = Vec::with_capacity(CPU_BASELINE_SAMPLES);
    let mut expected_output: Option<Vec<f32>> = None;
    for sample_idx in 0..CPU_BASELINE_SAMPLES {
        let baseline_start = std::time::Instant::now();
        let output = cpu_oracle_checked(x, packed, scale, zero_point, bias)?;
        let elapsed_ns = u64::try_from(baseline_start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        if let Some(expected) = expected_output.as_ref() {
            if output != *expected {
                return Err(BenchError::ExecutionFailed(format!(
                    "quantized linear CPU oracle sample {sample_idx} diverged from the first deterministic baseline"
                )));
            }
        } else {
            expected_output = Some(output);
        }
        durations.push(elapsed_ns);
    }
    durations.sort_unstable();
    let baseline_wall_ns = durations
        .get(durations.len() / 2)
        .copied()
        .ok_or_else(|| {
            BenchError::ExecutionFailed(
                "quantized linear CPU oracle produced no baseline samples".to_string(),
            )
        })?
        .max(1);
    let output = expected_output.ok_or_else(|| {
        BenchError::ExecutionFailed(
            "quantized linear CPU oracle produced no baseline output".to_string(),
        )
    })?;
    Ok((output, baseline_wall_ns))
}

inventory::submit! {
    &QuantizedLinear4BitAffineGrouped as &'static dyn BenchCase
}
