use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::resident::{input_bytes_total, transfer_accounting, ResidentInputSet};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const CPU_BASELINE_REPEATS: u32 = 32;

pub struct ElementwiseBench;

struct ElementwisePrepared {
    program: Program,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    baseline_output: Vec<u8>,
    baseline_wall_ns: u64,
    resident: Option<ResidentInputSet>,
}

impl BenchCase for ElementwiseBench {
    fn id(&self) -> BenchId {
        BenchId("foundation.elementwise.add.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Elementwise Add 1M".to_string(),
            description: "Elementwise f32 addition over 1M elements".to_string(),
            tags: vec!["compute".to_string(), "memory-bound".to_string()],
            layer: BenchLayer::Foundation,
            workload: WorkloadClass::Micro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        None
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let size = 1_000_000usize;
        let size_u32 = size as u32;
        let prog = Program::wrapped(
            vec![
                BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::F32)
                    .with_count(size_u32),
                BufferDecl::storage("b", 1, BufferAccess::ReadOnly, DataType::F32)
                    .with_count(size_u32),
                BufferDecl::output("out", 2, DataType::F32).with_count(size_u32),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("idx", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("idx"), Expr::u32(size_u32)),
                    vec![Node::store(
                        "out",
                        Expr::var("idx"),
                        Expr::add(
                            Expr::load("a", Expr::var("idx")),
                            Expr::load("b", Expr::var("idx")),
                        ),
                    )],
                ),
            ],
        );

        let inputs = elementwise_inputs(size);
        let input_bytes_total = input_bytes_total(&inputs);
        let resident = ResidentInputSet::upload_with_zeroed_outputs_optional(
            ctx,
            &inputs,
            &[size * 4],
            "elementwise bench",
        )?;

        let mut baseline_output = vec![0u8; size * 4];
        let baseline_start = std::time::Instant::now();
        for _ in 0..CPU_BASELINE_REPEATS {
            crate::cases::cpu_baselines::elementwise_add_f32_bytes_into(
                &inputs[0],
                &inputs[1],
                &mut baseline_output,
            );
        }
        let baseline_wall_ns =
            (baseline_start.elapsed().as_nanos() / u128::from(CPU_BASELINE_REPEATS)) as u64;

        Ok(Box::new(ElementwisePrepared {
            program: prog,
            baseline_output,
            baseline_wall_ns,
            inputs,
            input_bytes_total,
            resident,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<ElementwisePrepared>()
            .map(|prepared| &prepared.program)
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        if let Some(p) = prepared.downcast_ref::<ElementwisePrepared>() {
            let read = p.inputs[0].len() as u64 + p.inputs[1].len() as u64;
            let written = p.baseline_output.len() as u64;
            (read, written)
        } else {
            (0, 0)
        }
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_mut::<ElementwisePrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "elementwise prepared payload type mismatch".to_string(),
                )
            })?;
        let size = 1_000_000;

        let timed = if let Some(resident) = &prepared.resident {
            let driver_result = resident
                .dispatch_timed(&prepared.program, &ctx.dispatch_config)
                .map_err(|error| BenchError::BackendFailed(error.to_string()))?;

            crate::probes::cuda_events::CudaEventResult {
                outputs: driver_result.outputs,
                wall_ns: driver_result.wall_ns,
                device_ns: driver_result.device_ns,
                kernel_queue_submit_ns: driver_result.enqueue_ns,
                kernel_execute_ns: driver_result.device_ns,
                device_sync_ns: driver_result.wait_ns,
            }
        } else {
            crate::probes::cuda_events::dispatch_with_events(
                ctx,
                &prepared.program,
                &prepared.inputs,
                &ctx.dispatch_config,
            )
            .map_err(|e| BenchError::BackendFailed(e.to_string()))?
        };
        let wall = timed.wall_ns;
        let dispatch_ns = timed.device_ns;
        let kernel_queue_submit_ns = timed.kernel_queue_submit_ns;
        let kernel_execute_ns = timed.kernel_execute_ns;
        let device_sync_ns = timed.device_sync_ns;
        let outputs = timed.outputs;

        let input_bytes = prepared.input_bytes_total;
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting =
            transfer_accounting(input_bytes, output_bytes, prepared.resident.is_some());

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall),
                dispatch_ns,
                input_bytes: Some(input_bytes),
                output_bytes: Some(output_bytes),
                bytes_touched: Some(accounting.bytes_touched),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                kernel_queue_submit_ns,
                kernel_execute_ns,
                device_sync_ns,
                custom: vec![MetricPoint {
                    name: "flop_count".to_string(),
                    value: size as u64,
                }],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(
                    prepared.inputs[0]
                        .len()
                        .saturating_add(prepared.inputs[1].len()) as u64,
                ),
                output_bytes: Some(prepared.baseline_output.len() as u64),
                custom: vec![MetricPoint {
                    name: "flop_count".to_string(),
                    value: size as u64,
                }],
                ..Default::default()
            }),
            outputs,
            baseline_outputs: ctx
                .include_baseline_outputs
                .then(|| vec![prepared.baseline_output.clone()]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

fn elementwise_inputs(size: usize) -> Vec<Vec<u8>> {
    let mut a_bytes = vec![0u8; size * 4];
    let mut b_bytes = vec![0u8; size * 4];
    for i in 0..size {
        let a_val: f32 = i as f32;
        let b_val: f32 = (i * 2) as f32;
        a_bytes[i * 4..i * 4 + 4].copy_from_slice(&a_val.to_le_bytes());
        b_bytes[i * 4..i * 4 + 4].copy_from_slice(&b_val.to_le_bytes());
    }
    vec![a_bytes, b_bytes]
}

inventory::submit! {
    &ElementwiseBench as &'static dyn BenchCase
}
