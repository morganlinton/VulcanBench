use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub struct OptimizerImpact;

impl BenchCase for OptimizerImpact {
    fn id(&self) -> BenchId {
        BenchId("foundation.optimizer.impact".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Optimizer Impact Analysis".to_string(),
            description: "Measures GPU speedup from CSE and constant folding".to_string(),
            tags: vec!["compute".to_string(), "optimizer".to_string()],
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

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let size = 1_000_000;

        let prog = Program::wrapped(
            vec![
                BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::F32)
                    .with_count(size as u32),
                BufferDecl::storage("b", 1, BufferAccess::ReadOnly, DataType::F32)
                    .with_count(size as u32),
                BufferDecl::output("out", 2, DataType::F32).with_count(size as u32),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("idx", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("idx"), Expr::u32(size)),
                    vec![
                        // Highly redundant program: CSE and Constant Folding opportunity
                        Node::let_bind("val_a", Expr::load("a", Expr::var("idx"))),
                        Node::let_bind("val_b", Expr::load("b", Expr::var("idx"))),
                        Node::let_bind("t1", Expr::add(Expr::var("val_a"), Expr::var("val_b"))),
                        Node::let_bind("t2", Expr::add(Expr::var("val_a"), Expr::var("val_b"))),
                        Node::let_bind("t3", Expr::add(Expr::var("val_a"), Expr::var("val_b"))),
                        Node::let_bind("t4", Expr::add(Expr::var("val_a"), Expr::var("val_b"))),
                        // DSL target: BitOr with 0 (should be eliminated by StrengthReduce peephole)
                        Node::let_bind("idx_bitor", Expr::bitor(Expr::var("idx"), Expr::u32(0))),
                        // DSL target: BitAnd with 0 (should become 0)
                        Node::let_bind("zero_mask", Expr::bitand(Expr::var("idx"), Expr::u32(0))),
                        Node::let_bind("c1", Expr::add(Expr::f32(1.0), Expr::f32(2.0))),
                        Node::let_bind("c2", Expr::mul(Expr::f32(1.0), Expr::f32(2.0))),
                        Node::let_bind("sum1", Expr::add(Expr::var("t1"), Expr::var("t2"))),
                        Node::let_bind("sum2", Expr::add(Expr::var("t3"), Expr::var("t4"))),
                        Node::let_bind(
                            "final",
                            Expr::add(
                                Expr::add(Expr::var("sum1"), Expr::var("sum2")),
                                Expr::add(Expr::var("c1"), Expr::var("c2")),
                            ),
                        ),
                        // Use the dummy vars so they aren't DCE'd before we can test strength reduction
                        Node::let_bind(
                            "final_masked",
                            Expr::add(
                                Expr::var("final"),
                                Expr::cast(DataType::F32, Expr::var("zero_mask")),
                            ),
                        ),
                        Node::store("out", Expr::var("idx_bitor"), Expr::var("final_masked")),
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

        let size = 1_000_000;
        let mut a_bytes = vec![0u8; size * 4];
        let mut b_bytes = vec![0u8; size * 4];
        for i in 0..size {
            let a_val = (i % 257) as f32;
            let b_val = (i % 131) as f32;
            a_bytes[i * 4..i * 4 + 4].copy_from_slice(&a_val.to_le_bytes());
            b_bytes[i * 4..i * 4 + 4].copy_from_slice(&b_val.to_le_bytes());
        }

        let inputs = vec![a_bytes, b_bytes];

        let timed_unopt = ctx
            .dispatch_timed(prog, &inputs, &ctx.dispatch_config)
            .map_err(|e| BenchError::BackendFailed(e.to_string()))?;
        let elapsed_unopt = timed_unopt.wall_ns;
        let unopt_dispatch_ns = timed_unopt.device_ns;
        let unopt_outputs = timed_unopt.outputs;

        let optimized_prog = vyre_foundation::optimizer::optimize(prog.clone())
            .map_err(|e| BenchError::BackendFailed(e.to_string()))?;
        let optimizer_input_nodes = prog.stats().node_count as u64;
        let optimizer_output_nodes = optimized_prog.stats().node_count as u64;
        let optimizer_nodes_eliminated =
            optimizer_input_nodes.saturating_sub(optimizer_output_nodes);

        let timed_opt = ctx
            .dispatch_timed(&optimized_prog, &inputs, &ctx.dispatch_config)
            .map_err(|e| BenchError::BackendFailed(e.to_string()))?;
        let elapsed_opt = timed_opt.wall_ns;
        let opt_dispatch_ns = timed_opt.device_ns;
        let outputs = timed_opt.outputs;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(elapsed_opt),
                dispatch_ns: opt_dispatch_ns,
                input_bytes: Some(inputs.iter().map(Vec::len).sum::<usize>() as u64),
                output_bytes: Some(outputs.iter().map(Vec::len).sum::<usize>() as u64),
                custom: vec![
                    MetricPoint {
                        name: "optimizer_input_nodes".to_string(),
                        value: optimizer_input_nodes,
                    },
                    MetricPoint {
                        name: "optimizer_output_nodes".to_string(),
                        value: optimizer_output_nodes,
                    },
                    MetricPoint {
                        name: "optimizer_nodes_eliminated".to_string(),
                        value: optimizer_nodes_eliminated,
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(elapsed_unopt),
                dispatch_ns: unopt_dispatch_ns,
                input_bytes: Some(inputs.iter().map(Vec::len).sum::<usize>() as u64),
                output_bytes: Some(unopt_outputs.iter().map(Vec::len).sum::<usize>() as u64),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(unopt_outputs),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

inventory::submit! {
    &OptimizerImpact as &'static dyn BenchCase
}
