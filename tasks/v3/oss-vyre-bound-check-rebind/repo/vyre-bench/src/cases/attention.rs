use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub struct Attention;

impl BenchCase for Attention {
    fn id(&self) -> BenchId {
        BenchId("foundation.attention.64".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Attention 64x64".to_string(),
            description: "Self-Attention QKV block (64 seq, 64 dim)".to_string(),
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
        Some(PerformanceContract::cpu_sota_min_speedup(
            "attention proxy 64x64",
            "rayon",
            "rayon CPU attention baseline",
            1.5,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let seq = 64;
        let dim = 64;

        let prog = Program::wrapped(
            vec![
                BufferDecl::storage("Q", 0, BufferAccess::ReadOnly, DataType::F32)
                    .with_count((seq * dim) as u32),
                BufferDecl::storage("K", 1, BufferAccess::ReadOnly, DataType::F32)
                    .with_count((seq * dim) as u32),
                BufferDecl::storage("V", 2, BufferAccess::ReadOnly, DataType::F32)
                    .with_count((seq * dim) as u32),
                BufferDecl::output("out", 3, DataType::F32).with_count((seq * dim) as u32),
            ],
            [16, 16, 1],
            vec![
                Node::let_bind("row", Expr::gid_x()),
                Node::let_bind("col", Expr::gid_y()),
                Node::if_then(
                    Expr::and(
                        Expr::lt(Expr::var("row"), Expr::u32(seq)),
                        Expr::lt(Expr::var("col"), Expr::u32(dim)),
                    ),
                    vec![
                        // In reality attention requires a full softmax across K before multiplying V,
                        // this is a simplified proxy doing Q * K^T * V sequentially for the cell.
                        Node::let_bind("acc", Expr::f32(0.0)),
                        Node::loop_(
                            "k",
                            Expr::u32(0),
                            Expr::u32(seq),
                            vec![
                                Node::let_bind(
                                    "q_val",
                                    Expr::load(
                                        "Q",
                                        Expr::add(
                                            Expr::mul(Expr::var("row"), Expr::u32(dim)),
                                            Expr::var("col"),
                                        ),
                                    ),
                                ),
                                Node::let_bind(
                                    "k_val",
                                    Expr::load(
                                        "K",
                                        Expr::add(
                                            Expr::mul(Expr::var("k"), Expr::u32(dim)),
                                            Expr::var("col"),
                                        ),
                                    ),
                                ),
                                Node::let_bind(
                                    "v_val",
                                    Expr::load(
                                        "V",
                                        Expr::add(
                                            Expr::mul(Expr::var("k"), Expr::u32(dim)),
                                            Expr::var("col"),
                                        ),
                                    ),
                                ),
                                Node::assign(
                                    "acc",
                                    Expr::add(
                                        Expr::var("acc"),
                                        Expr::mul(
                                            Expr::mul(Expr::var("q_val"), Expr::var("k_val")),
                                            Expr::var("v_val"),
                                        ),
                                    ),
                                ),
                                // k is auto-incremented
                            ],
                        ),
                        Node::store(
                            "out",
                            Expr::add(
                                Expr::mul(Expr::var("row"), Expr::u32(dim)),
                                Expr::var("col"),
                            ),
                            Expr::var("acc"),
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

        let seq = 64;
        let dim = 64;

        let mut q_bytes = vec![0u8; seq * dim * 4];
        let mut k_bytes = vec![0u8; seq * dim * 4];
        let mut v_bytes = vec![0u8; seq * dim * 4];
        for i in 0..seq * dim {
            let q = (i % 3) as f32;
            let k = (i % 5) as f32;
            let v = (i % 7) as f32;
            q_bytes[i * 4..i * 4 + 4].copy_from_slice(&q.to_le_bytes());
            k_bytes[i * 4..i * 4 + 4].copy_from_slice(&k.to_le_bytes());
            v_bytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }

        let inputs = vec![q_bytes, k_bytes, v_bytes];

        let timed = ctx
            .dispatch_timed(prog, &inputs, &ctx.dispatch_config)
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
        let elapsed = timed.wall_ns;
        let dispatch_ns = timed.device_ns;
        let outputs = timed.outputs;

        let start_ref = std::time::Instant::now();
        let baseline_outputs = vec![crate::cases::cpu_baselines::attention_proxy_f32_bytes(
            &inputs[0], &inputs[1], &inputs[2], seq, dim,
        )];
        let elapsed_ref = start_ref.elapsed().as_nanos() as u64;
        let flop_count = (3 * seq * dim * seq) as u64;

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
                input_bytes: Some(
                    inputs[0]
                        .len()
                        .saturating_add(inputs[1].len())
                        .saturating_add(inputs[2].len()) as u64,
                ),
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
    &Attention as &'static dyn BenchCase
}
