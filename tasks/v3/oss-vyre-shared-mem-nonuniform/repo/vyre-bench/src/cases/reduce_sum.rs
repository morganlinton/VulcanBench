use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub struct ReduceSumBench;

impl BenchCase for ReduceSumBench {
    fn id(&self) -> BenchId {
        BenchId("foundation.reduce.sum.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Reduce Sum 1M".to_string(),
            description: "Sum reduction over 1M elements using atomic operations".to_string(),
            tags: vec![
                "compute".to_string(),
                "memory-bound".to_string(),
                "reduction".to_string(),
            ],
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
        Some(PerformanceContract::cpu_sota_min_speedup(
            "u32 reduction sum",
            "rayon",
            "rayon CPU reduction baseline",
            1.1,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let size = 1_000_000;
        let prog = Program::wrapped(
            vec![
                BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(size as u32),
                BufferDecl::output("out", 1, DataType::U32).with_count(1),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("idx", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("idx"), Expr::u32(size)),
                    vec![Node::let_bind(
                        "discard",
                        Expr::atomic_add("out", Expr::u32(0), Expr::load("a", Expr::var("idx"))),
                    )],
                ),
            ],
        );
        Ok(Box::new(prog))
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prog = crate::api::case::prepared_program(prepared)?;

        let size = 1_000_000;
        let mut a_bytes = vec![0u8; size * 4];
        for i in 0..size {
            let a_val: u32 = 1; // sum of 1M 1s is 1,000,000
            a_bytes[i * 4..i * 4 + 4].copy_from_slice(&a_val.to_le_bytes());
        }

        let inputs = vec![a_bytes];
        let timed = ctx
            .dispatch_timed(prog, &inputs, &ctx.dispatch_config)
            .map_err(|e| BenchError::BackendFailed(e.to_string()))?;
        let wall = timed.wall_ns;
        let dispatch_ns = timed.device_ns;
        let outputs = timed.outputs;

        let baseline_start = std::time::Instant::now();
        let baseline_outputs = vec![crate::cases::cpu_baselines::reduce_sum_u32_bytes(
            &inputs[0],
        )];
        let baseline_wall = baseline_start.elapsed().as_nanos() as u64;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall),
                dispatch_ns,
                input_bytes: Some(inputs.iter().map(Vec::len).sum::<usize>() as u64),
                output_bytes: Some(outputs.iter().map(Vec::len).sum::<usize>() as u64),
                custom: vec![MetricPoint {
                    name: "flop_count".to_string(),
                    value: size as u64,
                }],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_wall),
                input_bytes: Some(inputs[0].len() as u64),
                output_bytes: Some(baseline_outputs.iter().map(Vec::len).sum::<usize>() as u64),
                custom: vec![MetricPoint {
                    name: "flop_count".to_string(),
                    value: size as u64,
                }],
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(baseline_outputs),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

inventory::submit! {
    &ReduceSumBench as &'static dyn BenchCase
}
