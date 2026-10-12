use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub struct Matmul;

impl BenchCase for Matmul {
    fn id(&self) -> BenchId {
        BenchId("foundation.matmul.256".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "MatMul 256x256".to_string(),
            description: "Dense matrix multiplication 256x256 floats".to_string(),
            tags: vec!["compute".to_string(), "compute-bound".to_string()],
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
            "f32 matmul 256x256",
            "faer",
            "faer CPU matrix multiply baseline",
            3.0,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let m = 256;
        let n = 256;
        let k = 256;

        let prog = Program::wrapped(
            vec![
                BufferDecl::storage("A", 0, BufferAccess::ReadOnly, DataType::F32)
                    .with_count((m * k) as u32),
                BufferDecl::storage("B", 1, BufferAccess::ReadOnly, DataType::F32)
                    .with_count((k * n) as u32),
                BufferDecl::output("C", 2, DataType::F32).with_count((m * n) as u32),
            ],
            [16, 16, 1],
            vec![
                Node::let_bind("row", Expr::gid_x()),
                Node::let_bind("col", Expr::gid_y()),
                Node::if_then(
                    Expr::and(
                        Expr::lt(Expr::var("row"), Expr::u32(m)),
                        Expr::lt(Expr::var("col"), Expr::u32(n)),
                    ),
                    vec![
                        Node::let_bind("sum", Expr::f32(0.0)),
                        Node::loop_(
                            "k",
                            Expr::u32(0),
                            Expr::u32(k),
                            vec![
                                Node::let_bind(
                                    "a_idx",
                                    Expr::add(
                                        Expr::mul(Expr::var("row"), Expr::u32(k)),
                                        Expr::var("k"),
                                    ),
                                ),
                                Node::let_bind(
                                    "b_idx",
                                    Expr::add(
                                        Expr::mul(Expr::var("k"), Expr::u32(n)),
                                        Expr::var("col"),
                                    ),
                                ),
                                Node::assign(
                                    "sum",
                                    Expr::fma(
                                        Expr::load("A", Expr::var("a_idx")),
                                        Expr::load("B", Expr::var("b_idx")),
                                        Expr::var("sum"),
                                    ),
                                ),
                                // Note: Vyre IR `loop_` automatically increments `k`, no need to assign `k` here.
                            ],
                        ),
                        Node::store(
                            "C",
                            Expr::add(Expr::mul(Expr::var("row"), Expr::u32(n)), Expr::var("col")),
                            Expr::var("sum"),
                        ),
                    ],
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

        let m = 256;
        let n = 256;
        let k = 256;

        let mut a_bytes = vec![0u8; m * k * 4];
        let mut b_bytes = vec![0u8; k * n * 4];
        for i in 0..m * k {
            let value = (i % 7) as f32;
            a_bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        for i in 0..k * n {
            let value = (i % 5) as f32;
            b_bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }

        let inputs = vec![a_bytes, b_bytes];

        let timed = ctx
            .dispatch_timed(prog, &inputs, &ctx.dispatch_config)
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
        let elapsed = timed.wall_ns;
        let dispatch_ns = timed.device_ns;
        let outputs = timed.outputs;

        let start_ref = std::time::Instant::now();
        let baseline_outputs = vec![crate::cases::cpu_baselines::matmul_f32_bytes(
            &inputs[0], &inputs[1], m, n, k,
        )];
        let elapsed_ref = start_ref.elapsed().as_nanos() as u64;
        let flop_count = (2 * m * n * k) as u64;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(elapsed),
                dispatch_ns,
                input_bytes: Some(inputs.iter().map(Vec::len).sum::<usize>() as u64),
                output_bytes: Some(outputs.iter().map(Vec::len).sum::<usize>() as u64),
                custom: vec![MetricPoint {
                    name: "flop_count".to_string(),
                    value: flop_count,
                }],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(elapsed_ref),
                input_bytes: Some(inputs[0].len().saturating_add(inputs[1].len()) as u64),
                output_bytes: Some(baseline_outputs.iter().map(Vec::len).sum::<usize>() as u64),
                custom: vec![MetricPoint {
                    name: "flop_count".to_string(),
                    value: flop_count,
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
    &Matmul as &'static dyn BenchCase
}
