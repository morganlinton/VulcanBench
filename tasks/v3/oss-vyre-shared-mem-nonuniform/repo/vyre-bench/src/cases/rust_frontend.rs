use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use vyre_frontend_rust::pipeline::{RustPipeline, RustPipelineConfig};

mod lexer;
mod lexer_batch;

pub struct RustRangeLoopPipeline;

const LANE_COUNT: usize = 1 << 16;

struct RustRangePrepared {
    source: &'static str,
    program: vyre::ir::Program,
    input: Vec<u8>,
    baseline_output: Vec<u8>,
    baseline_wall_ns: u64,
}

impl BenchCase for RustRangeLoopPipeline {
    fn id(&self) -> BenchId {
        BenchId("frontend.rust.range_loop.ir_execute".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Batched Rust Range Loop IR Execute".to_string(),
            description:
                "Rust nano-subset source with variable half-open range control flow lowered to batched Vyre IR and executed across input buffers"
                    .to_string(),
            tags: vec![
                "frontend-rust".to_string(),
                "parser".to_string(),
                "control-flow".to_string(),
                "range-loop".to_string(),
                "gpu-lexer".to_string(),
                "public-pipeline".to_string(),
                "ir-lowering".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-frontend-rust".to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        &[
            crate::api::suite::SuiteKind::Release,
            crate::api::suite::SuiteKind::Gpu,
            crate::api::suite::SuiteKind::Deep,
            crate::api::suite::SuiteKind::Honest,
        ]
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: Some((LANE_COUNT * std::mem::size_of::<i32>()) as u64),
            feature_set: vec![
                "rust-parser".to_string(),
                "gpu-lexer".to_string(),
                "batched-lowering".to_string(),
                "range-loop".to_string(),
                "ir-lowering".to_string(),
            ],
        }
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let program = lower_rust_source(RUST_RANGE_SOURCE)?;
        let input_values = rust_range_inputs();
        let input = i32s_to_bytes(&input_values);
        let baseline_start = std::time::Instant::now();
        let baseline = cpu_range_loop_batch(&input_values);
        let baseline_wall_ns = baseline_start.elapsed().as_nanos() as u64;
        Ok(Box::new(RustRangePrepared {
            source: RUST_RANGE_SOURCE,
            program,
            input,
            baseline_output: i32s_to_bytes(&baseline),
            baseline_wall_ns,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a vyre::ir::Program> {
        prepared
            .downcast_ref::<RustRangePrepared>()
            .map(|prepared| &prepared.program)
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<RustRangePrepared>()
            .map(|prepared| {
                (
                    prepared.input.len() as u64,
                    prepared.baseline_output.len() as u64,
                )
            })
            .unwrap_or((0, 0))
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<RustRangePrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "Rust range-loop prepared payload type mismatch".to_string(),
                )
            })?;

        let lower_start = std::time::Instant::now();
        let lowered = lower_rust_source(prepared.source)?;
        let lower_ns = lower_start.elapsed().as_nanos() as u64;

        if lowered.fingerprint() != prepared.program.fingerprint() {
            return Err(BenchError::ExecutionFailed(
                "Rust range-loop source lowered to a different Program fingerprint during measured execution".to_string(),
            ));
        }

        let timed = ctx
            .dispatch_timed(
                &prepared.program,
                std::slice::from_ref(&prepared.input),
                &ctx.dispatch_config,
            )
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
        let output_bytes = timed.outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let wall_ns = timed.wall_ns.saturating_add(lower_ns);
        let dispatch_ns = timed.wall_ns;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns),
                lower_ns: Some(lower_ns),
                dispatch_ns: Some(dispatch_ns),
                kernel_execute_ns: timed.device_ns.filter(|ns| *ns > 0),
                input_bytes: Some(prepared.input.len() as u64),
                output_bytes: Some(output_bytes),
                bytes_read: Some(prepared.input.len() as u64),
                bytes_written: Some(output_bytes),
                wire_bytes: Some(prepared.source.len() as u64),
                custom: vec![
                    MetricPoint {
                        name: "rust_frontend_public_pipeline_speedup_x1000".to_string(),
                        value: speedup_x1000(prepared.baseline_wall_ns, wall_ns),
                    },
                    MetricPoint {
                        name: "rust_frontend_dispatch_speedup_x1000".to_string(),
                        value: speedup_x1000(prepared.baseline_wall_ns, dispatch_ns),
                    },
                    MetricPoint {
                        name: "rust_frontend_lanes".to_string(),
                        value: LANE_COUNT as u64,
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(prepared.input.len() as u64),
                output_bytes: Some(prepared.baseline_output.len() as u64),
                bytes_read: Some(prepared.input.len() as u64),
                bytes_written: Some(prepared.baseline_output.len() as u64),
                ..Default::default()
            }),
            outputs: timed.outputs,
            baseline_outputs: ctx
                .include_baseline_outputs
                .then(|| vec![prepared.baseline_output.clone()]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

const RUST_RANGE_SOURCE: &str = "\
fn f(n: i32) -> i32 {
    let mut acc: i32 = 0;
    for i in -32..n {
        if i < 0 {
            acc += i * 2;
        } else {
            acc += i - 1;
        };
    }
    return acc;
}";

fn lower_rust_source(source: &str) -> Result<vyre::ir::Program, BenchError> {
    let pipeline = RustPipeline::new(RustPipelineConfig {
        gpu_lex: true,
        borrow_check: true,
        lower: true,
        lower_lane_count: Some(LANE_COUNT as u32),
    });
    let unit = pipeline.compile_unit(source.as_bytes()).map_err(|error| {
        BenchError::ExecutionFailed(format!(
            "Rust range-loop bench public pipeline compile failed: {error}"
        ))
    })?;
    unit.program.ok_or_else(|| {
        BenchError::ExecutionFailed(
            "Rust range-loop bench public pipeline did not emit a Program despite lower:true"
                .to_string(),
        )
    })
}

fn rust_range_inputs() -> Vec<i32> {
    (0..LANE_COUNT)
        .map(|lane| (mix32(lane as u32) & 0x7f) as i32 - 16)
        .collect()
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

fn i32s_to_bytes(values: &[i32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn cpu_range_loop_batch(inputs: &[i32]) -> Vec<i32> {
    inputs.iter().map(|&input| cpu_range_loop(input)).collect()
}

fn cpu_range_loop(n: i32) -> i32 {
    let mut acc = 0_i32;
    for i in -32..n {
        if i < 0 {
            acc = acc.wrapping_add(i.wrapping_mul(2));
        } else {
            acc = acc.wrapping_add(i.wrapping_sub(1));
        }
    }
    acc
}

fn speedup_x1000(baseline_ns: u64, candidate_ns: u64) -> u64 {
    if candidate_ns == 0 {
        return 0;
    }
    (u128::from(baseline_ns).saturating_mul(1000) / u128::from(candidate_ns))
        .min(u128::from(u64::MAX)) as u64
}

inventory::submit! {
    &RustRangeLoopPipeline as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_range_loop_source_lowers_to_executable_program() {
        let program = lower_rust_source(RUST_RANGE_SOURCE).expect("range-loop source must lower");
        assert!(
            program.stats().node_count > 0,
            "range-loop lowered program must contain executable IR nodes"
        );
        assert!(
            program
                .buffers()
                .iter()
                .all(|buffer| buffer.count() == LANE_COUNT as u32),
            "batched range-loop program must size every input/output buffer to the lane count"
        );
        assert_eq!(cpu_range_loop(4), -1054);
        assert_eq!(speedup_x1000(10_000, 1_000), 10_000);
        assert_eq!(speedup_x1000(10_000, 0), 0);
    }
}
