use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub struct Transpose;

impl BenchCase for Transpose {
    fn id(&self) -> BenchId {
        BenchId("foundation.transpose.512".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Transpose 512x512".to_string(),
            description: "Dense f32 matrix transpose with coalesced reads and strided writes"
                .to_string(),
            tags: vec!["memory-bound".to_string(), "layout".to_string()],
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

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let rows = 512u32;
        let cols = 512u32;
        let prog = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::F32)
                    .with_count(rows * cols),
                BufferDecl::output("out", 1, DataType::F32).with_count(rows * cols),
            ],
            [16, 16, 1],
            vec![
                Node::let_bind("row", Expr::gid_x()),
                Node::let_bind("col", Expr::gid_y()),
                Node::if_then(
                    Expr::and(
                        Expr::lt(Expr::var("row"), Expr::u32(rows)),
                        Expr::lt(Expr::var("col"), Expr::u32(cols)),
                    ),
                    vec![Node::store(
                        "out",
                        Expr::add(
                            Expr::mul(Expr::var("col"), Expr::u32(rows)),
                            Expr::var("row"),
                        ),
                        Expr::load(
                            "input",
                            Expr::add(
                                Expr::mul(Expr::var("row"), Expr::u32(cols)),
                                Expr::var("col"),
                            ),
                        ),
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
        let rows = 512usize;
        let cols = 512usize;
        let mut input = vec![0u8; rows * cols * 4];
        for i in 0..rows * cols {
            input[i * 4..i * 4 + 4].copy_from_slice(&((i % 251) as f32).to_le_bytes());
        }
        let inputs = vec![input];
        let move_count = rows.saturating_mul(cols) as u64;

        crate::cases::gpu_case::run_gpu_with_cpu_baseline(
            ctx,
            prepared,
            inputs,
            move_count,
            |inputs| {
                vec![crate::cases::cpu_baselines::transpose_f32_bytes(
                    &inputs[0], rows, cols,
                )]
            },
        )
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

inventory::submit! {
    &Transpose as &'static dyn BenchCase
}
